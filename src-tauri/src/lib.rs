use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_opener::OpenerExt;

const SERVICE: &str = "moodle_mobile_app";
const SCHEME: &str = "moodledesk";

#[derive(Clone, Serialize, Deserialize)]
struct Session { site: String, token: String }

#[derive(Default)]
struct AppState {
    pending: Mutex<Option<(String, String)>>, // (site, passport)
    session: Mutex<Option<Session>>,
}

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new("moodledesk", "session").map_err(|e| e.to_string())
}

fn normalize_site(input: &str) -> String {
    let mut s = input.trim().to_string();
    if !s.starts_with("http://") && !s.starts_with("https://") { s = format!("https://{s}"); }
        let start = s.find("://").map(|i| i + 3).unwrap_or(0);
        // cut known Moodle sub-paths, but only on whole path segments (so "/mycampus" is preserved)
        let mut cut_at = s.len();
        for cut in ["/login", "/my", "/course", "/index.php", "/webservice", "/admin"] {
            for (i, _) in s[start..].match_indices(cut) {
                let rest = &s[start + i + cut.len()..];
                if rest.is_empty() || rest.starts_with(['/', '?', '#']) { cut_at = cut_at.min(start + i); break; }
            }
        }
        s.truncate(cut_at);
        s.trim_end_matches('/').to_string()
}

fn store(app: &AppHandle, session: Session) {
    if let Ok(json) = serde_json::to_string(&session) {
        if let Ok(e) = keyring_entry() { let _ = e.set_password(&json); }
    }
    *app.state::<AppState>().session.lock().unwrap() = Some(session);
}

/// Returns { site, config } where config.typeoflogin: 1=app, 2=browser SSO, 3=embedded
#[tauri::command]
async fn resolve_site(input: String) -> Result<Value, String> {
    let site = normalize_site(&input);
    let body = serde_json::json!([{ "index": 0, "methodname": "tool_mobile_get_public_config", "args": {} }]);
    let client = reqwest::Client::builder()
    .user_agent("MoodleDesk/0.1 (MoodleMobile)")
    .build().map_err(|e| e.to_string())?;
    let resp = client
    .post(format!("{site}/lib/ajax/service-nologin.php"))
    .query(&[("info", "tool_mobile_get_public_config")])
    .json(&body).send().await
    .map_err(|e| format!("Can't reach {site}: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text)
    .map_err(|_| format!("{site} didn't return Moodle JSON (HTTP {status}). Is the URL the Moodle base address?"))?;
    let first = v.get(0).ok_or("Empty response from site")?;
    if first["error"].as_bool().unwrap_or(false) {
        let msg = first["exception"]["message"].as_str().unwrap_or("unknown error");
        return Err(format!("Site rejected the request: {msg}"));
    }
    Ok(serde_json::json!({ "site": site, "config": first["data"] }))
}

#[tauri::command]
fn start_sso(app: AppHandle, state: State<AppState>, site: String) -> Result<(), String> {
    let passport = uuid::Uuid::new_v4().simple().to_string();
    *state.pending.lock().unwrap() = Some((site.clone(), passport.clone()));
    let url = format!(
        "{site}/admin/tool/mobile/launch.php?service={SERVICE}&passport={passport}&urlscheme={SCHEME}"
    );
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

fn handle_url(app: &AppHandle, url: &str) {
    let Some(b64) = url.strip_prefix(&format!("{SCHEME}://token=")) else { return };
    let b64 = b64.trim_end_matches('/');
    let b64 = b64.replace("%3D", "=").replace("%2B", "+").replace("%2F", "/");
    let decoded = STANDARD.decode(b64).ok().and_then(|b| String::from_utf8(b).ok());
    let state = app.state::<AppState>();
    let pending = state.pending.lock().unwrap().take();
    let (Some(decoded), Some((site, passport))) = (decoded, pending) else {
        let _ = app.emit("sso-error", "Unexpected login callback"); return;
    };
    let parts: Vec<&str> = decoded.split(":::").collect();
    let expected = format!("{:x}", md5::compute(format!("{site}{passport}")));
    if parts.len() < 2 || parts[0] != expected {
        let _ = app.emit("sso-error", "Login callback failed verification"); return;
    }
    store(app, Session { site, token: parts[1].to_string() });
    let _ = app.emit("sso-complete", ());
}

#[tauri::command]
async fn login_password(app: AppHandle, site: String, username: String, password: String) -> Result<(), String> {
    let res: Value = reqwest::Client::new()
    .post(format!("{site}/login/token.php"))
    .form(&[("username", username), ("password", password), ("service", SERVICE.to_string())])
    .send().await.map_err(|e| e.to_string())?
    .json().await.map_err(|e| e.to_string())?;
    match res.get("token").and_then(|t| t.as_str()) {
        Some(t) => { store(&app, Session { site, token: t.into() }); Ok(()) }
        None => Err(res.get("error").and_then(|e| e.as_str()).unwrap_or("Login failed").into()),
    }
}

/// Generic Moodle web-service call. Nested params use Moodle's flat syntax, e.g. "courseids[0]".
#[tauri::command]
async fn ws_call(state: State<'_, AppState>, function: String, params: HashMap<String, String>) -> Result<Value, String> {
    let s = state.session.lock().unwrap().clone().ok_or("Not logged in")?;
    let mut form = params;
    form.insert("wstoken".into(), s.token);
    form.insert("wsfunction".into(), function);
    form.insert("moodlewsrestformat".into(), "json".into());
    let v: Value = reqwest::Client::new()
    .post(format!("{}/webservice/rest/server.php", s.site))
    .form(&form).send().await.map_err(|e| e.to_string())?
    .json().await.map_err(|e| e.to_string())?;
    if let Some(msg) = v.get("message").and_then(|m| m.as_str()) { if v.get("exception").is_some() { return Err(msg.into()); } }
    Ok(v)
}

/// Downloads a Moodle file into <dir>/<subdir>/<filename>, then opens it with the OS default app.
#[tauri::command]
async fn download_file(app: AppHandle, state: State<'_, AppState>, url: String, dir: String, subdir: String, filename: String) -> Result<String, String> {
    let s = state.session.lock().unwrap().clone().ok_or("Not logged in")?;
    if !url.starts_with(&s.site) { return Err("Refusing to download from a different host".into()); }
    let clean = |x: &str| x.chars().map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c }).collect::<String>().trim().to_string();
    let mut path = std::path::PathBuf::from(dir);
    path.push(clean(&subdir));
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    path.push(clean(&filename));
    let sep = if url.contains('?') { '&' } else { '?' };
    let bytes = reqwest::get(format!("{url}{sep}token={}", s.token)).await.map_err(|e| e.to_string())?
    .error_for_status().map_err(|e| e.to_string())?
    .bytes().await.map_err(|e| e.to_string())?;
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    let p = path.to_string_lossy().to_string();
    app.opener().open_path(p.clone(), None::<&str>).map_err(|e| e.to_string())?;
    Ok(p)
}

/// Restores the session from the OS keychain. Returns { site, token } or null.
#[tauri::command]
fn get_session(state: State<AppState>) -> Option<Session> {
    let mut g = state.session.lock().unwrap();
    if g.is_none() {
        *g = keyring_entry().ok().and_then(|e| e.get_password().ok()).and_then(|j| serde_json::from_str(&j).ok());
    }
    g.clone()
}

#[tauri::command]
fn logout(state: State<AppState>) {
    *state.session.lock().unwrap() = None;
    if let Ok(e) = keyring_entry() { let _ = e.delete_credential(); }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
    .plugin(tauri_plugin_single_instance::init(|_, _, _| {})) // must be first
    .plugin(tauri_plugin_deep_link::init())
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_dialog::init())
    .manage(AppState::default())
    .setup(|app| {
        #[cfg(any(windows, target_os = "linux"))]
        app.deep_link().register_all()?; // registers moodledesk:// at runtime (dev + portable builds)
    let handle = app.handle().clone();
    app.deep_link().on_open_url(move |ev| {
        for u in ev.urls() { handle_url(&handle, u.as_str()); }
    });
    Ok(())
    })
    .invoke_handler(tauri::generate_handler![
        resolve_site, start_sso, login_password, ws_call, get_session, logout, download_file
    ])
    .run(tauri::generate_context!())
    .expect("error while running MoodleDesk");
}
