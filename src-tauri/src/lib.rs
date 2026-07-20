mod commands;
mod config;
mod elev;
mod tailscale;

use tauri::{Manager, RunEvent};

use commands::{
    connect, disconnect, get_status, is_admin, ping_peer, sidecar_version,
    spawn_auto_connect_if_needed,
};
use tailscale::DaemonState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    config::load_dotenv();

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(DaemonState::new())
        .invoke_handler(tauri::generate_handler![
            connect,
            disconnect,
            get_status,
            ping_peer,
            is_admin,
            sidecar_version,
        ])
        .setup(|app| {
            spawn_auto_connect_if_needed(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let RunEvent::Exit = event {
                let daemon = app_handle.state::<DaemonState>();
                let _ = daemon.stop();
            }
        });
}
