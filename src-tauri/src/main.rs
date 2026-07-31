// Windows: no console window behind the app in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Before anything else — before Tauri, before the webview exists. WebView2
    // otherwise writes its profile to %LOCALAPPDATA%\<bundle-id>\, which breaks
    // "nothing outside the app directory" silently. The env var is read by the
    // WebView2 loader itself; `WebviewWindowBuilder::data_directory` in
    // `lib.rs` sets the same path explicitly. Both, deliberately.
    gallery_lib::redirect_webview_data_dir();

    gallery_lib::run();
}
