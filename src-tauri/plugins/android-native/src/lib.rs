//! Android only. The Kotlin side is in `android/`.
#![cfg(target_os = "android")]

use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Runtime};

pub struct AndroidNative<R: Runtime>(PluginHandle<R>);

#[derive(Serialize)]
struct PathArg<'a> {
    path: &'a str,
}
#[derive(Serialize)]
struct ValueArg<'a> {
    value: &'a str,
}
#[derive(Deserialize)]
struct Secret {
    value: Option<String>,
}

impl<R: Runtime> AndroidNative<R> {
    /// Opens the file in the app the user picks (the system's "open with" list).
    pub fn open_file(&self, path: &str) -> Result<(), String> {
        self.0.run_mobile_plugin("openFile", PathArg { path }).map_err(|e| e.to_string())
    }
    /// Encrypts with a key that lives in the Android Keystore (it never leaves the secure hardware) and stores the result.
    pub fn secret_set(&self, value: &str) -> Result<(), String> {
        self.0.run_mobile_plugin("secretSet", ValueArg { value }).map_err(|e| e.to_string())
    }
    pub fn secret_get(&self) -> Option<String> {
        self.0.run_mobile_plugin::<Secret>("secretGet", ()).ok()?.value
    }
    pub fn secret_delete(&self) {
        let _ = self.0.run_mobile_plugin::<()>("secretDelete", ());
    }
}

pub trait AndroidNativeExt<R: Runtime> {
    fn android_native(&self) -> &AndroidNative<R>;
}
impl<R: Runtime, T: Manager<R>> AndroidNativeExt<R> for T {
    fn android_native(&self) -> &AndroidNative<R> {
        self.state::<AndroidNative<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("android-native")
        .setup(|app, api| {
            let handle = api.register_android_plugin("app.moodledesk.androidnative", "AndroidNativePlugin")?;
            app.manage(AndroidNative(handle));
            Ok(())
        })
        .build()
}
