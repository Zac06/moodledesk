//! iOS only. The Swift side is in `ios/`.
#![cfg(target_os = "ios")]

use serde::Serialize;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Runtime};

tauri::ios_plugin_binding!(init_plugin_ios_native);

pub struct IosNative<R: Runtime>(PluginHandle<R>);

#[derive(Serialize)]
struct PathArg<'a> {
    path: &'a str,
}

impl<R: Runtime> IosNative<R> {
    /// Shows the file in the system viewer (Quick Look), which has Share and "Open in…" buttons of its own.
    pub fn open_file(&self, path: &str) -> Result<(), String> {
        self.0.run_mobile_plugin("openFile", PathArg { path }).map_err(|e| e.to_string())
    }
}

pub trait IosNativeExt<R: Runtime> {
    fn ios_native(&self) -> &IosNative<R>;
}
impl<R: Runtime, T: Manager<R>> IosNativeExt<R> for T {
    fn ios_native(&self) -> &IosNative<R> {
        self.state::<IosNative<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("ios-native")
        .setup(|app, api| {
            let handle = api.register_ios_plugin(init_plugin_ios_native)?;
            app.manage(IosNative(handle));
            Ok(())
        })
        .build()
}
