pub mod commands;
pub mod config;
pub mod service;
pub mod tailscale;

use tauri::{Manager, RunEvent};

use commands::{
    bootstrap_url, connect, disconnect, get_status, is_admin, network_service_ready, ping_peer,
    sidecar_version,
};
use service::ServiceClient;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    config::load_dotenv();

    tauri::Builder::default()
        // Must be first: second launch focuses the existing window and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            connect,
            disconnect,
            get_status,
            ping_peer,
            is_admin,
            network_service_ready,
            sidecar_version,
            bootstrap_url,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let RunEvent::Exit = event {
                // Ask service to tear down the tunnel; keep the Windows service itself running.
                let _ = ServiceClient::new().disconnect();
            }
        });
}
