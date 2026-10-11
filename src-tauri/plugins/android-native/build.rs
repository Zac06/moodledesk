fn main() {
    // no commands for the web view to call: the app's Rust code uses this plugin directly
    tauri_plugin::Builder::new(&[]).android_path("android").build();
}
