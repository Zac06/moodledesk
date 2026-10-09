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
struct Session {
    site: String,
    token: String,
    #[serde(default)]
    private_token: Option<String>,
}

#[derive(Default)]
struct AppState {
    pending: Mutex<Option<(String, String)>>, // (site, passport)
    session: Mutex<Option<Session>>,
    last_autologin: Mutex<Option<std::time::Instant>>,
}

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new("moodledesk", "session").map_err(|e| e.to_string())
}

fn normalize_site(input: &str) -> String {
    let mut s = input.trim().to_string();
    if !s.starts_with("http://") && !s.starts_with("https://") {
        s = format!("https://{s}");
    }
    let start = s.find("://").map(|i| i + 3).unwrap_or(0);
    // cut known Moodle sub-paths, but only on whole path segments (so "/mycampus" is preserved)
    let mut cut_at = s.len();
    for cut in [
        "/login",
        "/my",
        "/course",
        "/index.php",
        "/webservice",
        "/admin",
    ] {
        for (i, _) in s[start..].match_indices(cut) {
            let rest = &s[start + i + cut.len()..];
            if rest.is_empty() || rest.starts_with(['/', '?', '#']) {
                cut_at = cut_at.min(start + i);
                break;
            }
        }
    }
    s.truncate(cut_at);
    s.trim_end_matches('/').to_string()
}

/// Same scheme + host + port as the site. A plain `starts_with` would accept "https://moodle.uni.edu.evil.com".
fn same_origin(site: &str, url: &str) -> bool {
    match (reqwest::Url::parse(site), reqwest::Url::parse(url)) {
        (Ok(a), Ok(b)) => a.origin() == b.origin(),
        _ => false,
    }
}

fn store(app: &AppHandle, session: Session) {
    if let Ok(json) = serde_json::to_string(&session) {
        if let Ok(e) = keyring_entry() {
            let _ = e.set_password(&json);
        }
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
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(format!("{site}/lib/ajax/service-nologin.php"))
        .query(&[("info", "tool_mobile_get_public_config")])
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Can't reach {site}: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|_| {
        format!(
            "{site} didn't return Moodle JSON (HTTP {status}). Is the URL the Moodle base address?"
        )
    })?;
    let first = v.get(0).ok_or("Empty response from site")?;
    if first["error"].as_bool().unwrap_or(false) {
        let msg = first["exception"]["message"]
            .as_str()
            .unwrap_or("unknown error");
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
    launch(&app, &url)
}

fn handle_url(app: &AppHandle, url: &str) {
    let Some(b64) = url.strip_prefix(&format!("{SCHEME}://token=")) else {
        return;
    };
    let b64 = b64.trim_end_matches('/');
    let b64 = b64
        .replace("%3D", "=")
        .replace("%2B", "+")
        .replace("%2F", "/");
    let decoded = STANDARD
        .decode(b64)
        .ok()
        .and_then(|b| String::from_utf8(b).ok());
    let state = app.state::<AppState>();
    let pending = state.pending.lock().unwrap().take();
    let (Some(decoded), Some((site, passport))) = (decoded, pending) else {
        let _ = app.emit("sso-error", "Unexpected login callback");
        return;
    };
    let parts: Vec<&str> = decoded.split(":::").collect();
    let expected = format!("{:x}", md5::compute(format!("{site}{passport}")));
    if parts.len() < 2 || parts[0] != expected {
        let _ = app.emit("sso-error", "Login callback failed verification");
        return;
    }
    store(
        app,
        Session {
            site,
            token: parts[1].to_string(),
            private_token: parts.get(2).map(|p| p.to_string()),
        },
    );
    // bring the app back to the front now that the browser login is done
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    let _ = app.emit("sso-complete", ());
}

#[tauri::command]
async fn login_password(
    app: AppHandle,
    site: String,
    username: String,
    password: String,
) -> Result<(), String> {
    let res: Value = reqwest::Client::new()
        .post(format!("{site}/login/token.php"))
        .form(&[
            ("username", username),
            ("password", password),
            ("service", SERVICE.to_string()),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    match res.get("token").and_then(|t| t.as_str()) {
        Some(t) => {
            store(
                &app,
                Session {
                    site,
                    token: t.into(),
                    private_token: res
                        .get("privatetoken")
                        .and_then(|p| p.as_str())
                        .map(String::from),
                },
            );
            Ok(())
        }
        None => Err(res
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("Login failed")
            .into()),
    }
}

/// Generic Moodle web-service call. Nested params use Moodle's flat syntax, e.g. "courseids[0]".
#[tauri::command]
async fn ws_call(
    state: State<'_, AppState>,
    function: String,
    params: HashMap<String, String>,
) -> Result<Value, String> {
    let s = state
        .session
        .lock()
        .unwrap()
        .clone()
        .ok_or("Not logged in")?;
    let mut form = params;
    form.insert("wstoken".into(), s.token);
    form.insert("wsfunction".into(), function);
    form.insert("moodlewsrestformat".into(), "json".into());
    form.insert("moodlewssettingfilter".into(), "true".into()); // resolves multilingual names/summaries
    let v: Value = reqwest::Client::new()
        .post(format!("{}/webservice/rest/server.php", s.site))
        .form(&form)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
        if v.get("exception").is_some() {
            let code = v.get("errorcode").and_then(|c| c.as_str()).unwrap_or("");
            return Err(if code.is_empty() {
                msg.to_string()
            } else {
                format!("{msg} [{code}]")
            });
        }
    }
    Ok(v)
}

#[derive(Serialize, Deserialize, Clone)]
struct DlEntry {
    path: String,
    course: String,
    name: String,
    size: u64,
    at: u64,
}

fn manifest_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    let d = app.path().app_data_dir().ok()?;
    std::fs::create_dir_all(&d).ok()?;
    Some(d.join("downloads.json"))
}
fn load_manifest(app: &AppHandle) -> Vec<DlEntry> {
    manifest_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default()
}
fn save_manifest(app: &AppHandle, m: &[DlEntry]) {
    if let Some(p) = manifest_path(app) {
        if let Ok(j) = serde_json::to_string(m) {
            let _ = std::fs::write(p, j);
        }
    }
}
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Opens a file path or web URL with the OS default handler.
/// Inside an AppImage, AppRun puts the bundle's own libraries and data dirs into the environment. A viewer or
/// browser started with that environment can fail on incompatible libraries (silently, since the launch itself
/// succeeds), so on Linux we start xdg-open ourselves with the bundle's entries stripped out.
fn launch(app: &AppHandle, target: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use std::process::{Command, Stdio};
        let mut cmd = Command::new("xdg-open");
        cmd.arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(dir) = std::env::var("APPDIR").ok().filter(|d| !d.is_empty()) {
            for (k, v) in std::env::vars() {
                if matches!(k.as_str(), "APPDIR" | "APPIMAGE" | "ARGV0" | "OWD") {
                    cmd.env_remove(&k);
                    continue;
                }
                if v.contains(&dir) {
                    let kept: Vec<&str> = v
                        .split(':')
                        .filter(|p| !p.starts_with(dir.as_str()))
                        .collect();
                    if kept.is_empty() {
                        cmd.env_remove(&k);
                    } else {
                        cmd.env(&k, kept.join(":"));
                    }
                }
            }
        }
        if let Ok(mut child) = cmd.spawn() {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            return Ok(());
        }
    }
    if target.starts_with("http://") || target.starts_with("https://") {
        app.opener()
            .open_url(target.to_string(), None::<&str>)
            .map_err(|e| e.to_string())
    } else {
        app.opener()
            .open_path(target.to_string(), None::<&str>)
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn open_external(app: AppHandle, url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only web links can be opened".into());
    }
    launch(&app, &url)
}

fn default_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .download_dir()
        .or_else(|_| app.path().home_dir())
        .map(|p| p.join("MoodleDesk"))
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn default_download_dir(app: AppHandle) -> Result<String, String> {
    default_dir(&app).map(|p| p.to_string_lossy().to_string())
}

/// Saves a Moodle file to <dir>/<subdir>/<filename> and opens it with the OS default app.
/// mode: "always" = download every time, "reuse" = open the saved copy if present,
/// "updated" = reuse unless Moodle's copy (timemodified) is newer than the saved file.
#[tauri::command]
async fn download_file(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    dir: String,
    subdir: String,
    filename: String,
    mode: String,
    modified: Option<u64>,
    force: Option<bool>,
) -> Result<Value, String> {
    let s = state
        .session
        .lock()
        .unwrap()
        .clone()
        .ok_or("Not logged in")?;
    if !same_origin(&s.site, &url) {
        return Err("Refusing to download from a different host".into());
    }
    let clean = |x: &str| {
        x.chars()
            .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
            .collect::<String>()
            .trim()
            .to_string()
    };
    // empty dir = the default folder (system Downloads/MoodleDesk); the folder is chosen in Settings only
    let mut path = if dir.trim().is_empty() {
        default_dir(&app)?
    } else {
        std::path::PathBuf::from(dir)
    };
    let sub = clean(&subdir);
    path.push(if sub.is_empty() {
        "Moodle".to_string()
    } else {
        sub
    });
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    path.push(clean(&filename));
    let p = path.to_string_lossy().to_string();

    if path.is_file() && mode != "always" && !force.unwrap_or(false) {
        let up_to_date = mode == "reuse"
            || modified.map_or(true, |m| {
                std::fs::metadata(&path)
                    .and_then(|md| md.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(false, |d| d.as_secs() >= m)
            });
        if up_to_date {
            // adopt a saved copy we don't track yet, so it can be re-downloaded or cleared later
            let mut m = load_manifest(&app);
            if !m.iter().any(|e| e.path == p) {
                let size = std::fs::metadata(&path).map(|md| md.len()).unwrap_or(0);
                m.push(DlEntry {
                    path: p.clone(),
                    course: subdir.clone(),
                    name: filename.clone(),
                    size,
                    at: now_secs(),
                });
                save_manifest(&app, &m);
            }
            launch(&app, &p)?;
            return Ok(serde_json::json!({ "path": p, "reused": true }));
        }
    }

    let sep = if url.contains('?') { '&' } else { '?' };
    // no total timeout (big files on slow links are fine); only a stalled connection or transfer gives up
    let resp = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .read_timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?
        .get(format!("{url}{sep}token={}", s.token))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let is_json = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map_or(false, |t| t.starts_with("application/json"));
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    // Moodle can answer 200 with a JSON error instead of the file; don't save that as the file
    if is_json && !filename.to_lowercase().ends_with(".json") {
        let msg = serde_json::from_slice::<Value>(&bytes)
            .ok()
            .and_then(|v| v["message"].as_str().or(v["error"].as_str()).map(String::from))
            .unwrap_or_else(|| "Moodle returned an error instead of the file".into());
        return Err(msg);
    }
    // write beside the target and rename, so an interrupted write never leaves a truncated "saved" file
    let mut part = path.clone().into_os_string();
    part.push(".part");
    let part = std::path::PathBuf::from(part);
    std::fs::write(&part, &bytes)
        .and_then(|_| std::fs::rename(&part, &path))
        .map_err(|e| {
            let _ = std::fs::remove_file(&part);
            e.to_string()
        })?;

    // remember what we saved, so "clear downloads" only ever touches our own files
    let mut m = load_manifest(&app);
    m.retain(|e| e.path != p);
    m.push(DlEntry {
        path: p.clone(),
        course: subdir,
        name: filename,
        size: bytes.len() as u64,
        at: now_secs(),
    });
    save_manifest(&app, &m);

    launch(&app, &p)?;
    Ok(serde_json::json!({ "path": p, "reused": false }))
}

#[tauri::command]
fn list_downloads(app: AppHandle) -> Vec<DlEntry> {
    let mut m = load_manifest(&app);
    m.retain(|e| std::path::Path::new(&e.path).is_file());
    for e in m.iter_mut() {
        if let Ok(md) = std::fs::metadata(&e.path) {
            e.size = md.len();
        }
    }
    save_manifest(&app, &m);
    m.sort_by(|a, b| a.course.cmp(&b.course).then(a.name.cmp(&b.name)));
    m
}

/// Deletes the given tracked files (or all tracked files when paths is null). Returns how many were removed.
#[tauri::command]
fn delete_downloads(app: AppHandle, paths: Option<Vec<String>>) -> u64 {
    let mut n = 0u64;
    let mut m = load_manifest(&app);
    m.retain(|e| {
        if paths.as_ref().map_or(false, |p| !p.contains(&e.path)) {
            return true;
        }
        let gone = std::fs::remove_file(&e.path).is_ok() || !std::path::Path::new(&e.path).exists();
        if gone {
            n += 1;
            if let Some(d) = std::path::Path::new(&e.path).parent() {
                let _ = std::fs::remove_dir(d);
            } // only succeeds if empty
        }
        !gone
    });
    save_manifest(&app, &m);
    n
}

#[tauri::command]
fn open_local(app: AppHandle, path: String) -> Result<(), String> {
    launch(&app, &path)
}

/// Opens a URL in the system browser. For same-site URLs it first asks Moodle for a one-time
/// auto-login key (like the official mobile app) so the browser is logged in. Falls back to the plain URL.
#[tauri::command]
async fn open_authed(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<Option<String>, String> {
    let s = state
        .session
        .lock()
        .unwrap()
        .clone()
        .ok_or("Not logged in")?;
    // Moodle throttles key requests (default 6 min); within that window the browser session from the last one is reused
    let recent = state.last_autologin.lock().unwrap().map_or(false, |t| {
        t.elapsed() < std::time::Duration::from_secs(6 * 60)
    });
    let mut target = url.clone();
    let mut note: Option<String> = None;
    if same_origin(&s.site, &url) && !recent {
        if let Some(pt) = s.private_token.clone() {
            let res: Result<String, String> = async {
                // Moodle only issues auto-login keys to requests that identify as its app (errorcode apprequired)
                let client = reqwest::Client::builder()
                    .user_agent("MoodleMobile MoodleDesk/0.1")
                    .build()
                    .map_err(|e| e.to_string())?;
                let endpoint = format!("{}/webservice/rest/server.php", s.site);
                let info: Value = client
                    .post(&endpoint)
                    .form(&[
                        ("wstoken", s.token.as_str()),
                        ("wsfunction", "core_webservice_get_site_info"),
                        ("moodlewsrestformat", "json"),
                    ])
                    .send()
                    .await
                    .map_err(|e| e.to_string())?
                    .json()
                    .await
                    .map_err(|e| e.to_string())?;
                let uid = info["userid"].as_u64().ok_or("no userid")?;
                let v: Value = client
                    .post(&endpoint)
                    .form(&[
                        ("wstoken", s.token.as_str()),
                        ("wsfunction", "tool_mobile_get_autologin_key"),
                        ("moodlewsrestformat", "json"),
                        ("privatetoken", pt.as_str()),
                    ])
                    .send()
                    .await
                    .map_err(|e| e.to_string())?
                    .json()
                    .await
                    .map_err(|e| e.to_string())?;
                let key = v["key"].as_str().ok_or_else(|| {
                    v["message"]
                        .as_str()
                        .or(v["error"].as_str())
                        .unwrap_or("no key returned")
                        .to_string()
                })?;
                let base = v["autologinurl"].as_str().ok_or("no autologin url")?;
                let mut u = reqwest::Url::parse(base).map_err(|e| e.to_string())?;
                u.query_pairs_mut()
                    .append_pair("userid", &uid.to_string())
                    .append_pair("key", key)
                    .append_pair("urltogo", &url);
                Ok(u.to_string())
            }
            .await;
            match res {
                Ok(u) => {
                    target = u;
                    *state.last_autologin.lock().unwrap() = Some(std::time::Instant::now());
                }
                Err(e) => {
                    note = Some(format!(
                        "Browser auto-login unavailable: {e}. Opened the normal page."
                    ))
                }
            }
        } else {
            note = Some(
                "Browser auto-login unavailable: no private token stored. Sign out and in again."
                    .into(),
            );
        }
    }
    launch(&app, &target)?;
    Ok(note)
}

#[derive(Serialize)]
struct UpdateInfo {
    version: String,
    url: String,
    notes: String,
}

fn parse_ver(v: &str) -> Vec<u64> {
    let mut n: Vec<u64> = v
        .trim_start_matches('v')
        .split(['.', '-', '+'])
        .map_while(|p| p.parse::<u64>().ok())
        .collect();
    n.resize(3, 0);
    n
}

/// Checks the latest published GitHub release; returns it only if newer than the running version.
#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let v: Value = reqwest::Client::builder()
        .user_agent("MoodleDesk-update-check")
        .build()
        .map_err(|e| e.to_string())?
        .get("https://api.github.com/repos/zac06/moodledesk/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().ok_or("no tag in release")?;
    let current = app.package_info().version.to_string();
    if parse_ver(tag) > parse_ver(&current) {
        Ok(Some(UpdateInfo {
            version: tag.trim_start_matches('v').to_string(),
            url: v["html_url"]
                .as_str()
                .unwrap_or("https://github.com/zac06/moodledesk/releases/latest")
                .to_string(),
            notes: v["body"].as_str().unwrap_or("").chars().take(400).collect(),
        }))
    } else {
        Ok(None)
    }
}

/// Restores the session from the OS keychain. Returns { site, token } or null.
#[tauri::command]
fn get_session(state: State<AppState>) -> Option<Session> {
    let mut g = state.session.lock().unwrap();
    if g.is_none() {
        *g = keyring_entry()
            .ok()
            .and_then(|e| e.get_password().ok())
            .and_then(|j| serde_json::from_str(&j).ok());
    }
    g.clone()
}

#[tauri::command]
fn logout(state: State<AppState>) {
    *state.session.lock().unwrap() = None;
    if let Ok(e) = keyring_entry() {
        let _ = e.delete_credential();
    }
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
                for u in ev.urls() {
                    handle_url(&handle, u.as_str());
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            resolve_site,
            start_sso,
            login_password,
            ws_call,
            get_session,
            logout,
            download_file,
            default_download_dir,
            open_external,
            open_authed,
            check_update,
            list_downloads,
            delete_downloads,
            open_local
        ])
        .run(tauri::generate_context!())
        .expect("error while running MoodleDesk");
}

#[cfg(test)]
mod tests {
    use super::same_origin;
    #[test]
    fn origin_check() {
        let site = "https://moodle.uni.edu/mycampus";
        assert!(same_origin(site, "https://moodle.uni.edu/webservice/pluginfile.php/1/a.pdf"));
        assert!(same_origin(site, "https://moodle.uni.edu:443/x"));
        assert!(!same_origin(site, "https://moodle.uni.edu.evil.com/x"));
        assert!(!same_origin(site, "https://moodle.uni.edu@evil.com/x"));
        assert!(!same_origin(site, "http://moodle.uni.edu/x"));
        assert!(!same_origin(site, "not a url"));
    }
}
