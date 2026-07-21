// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Service / Session-0 path: never initialize Tauri/WebView2 under LocalSystem.
    if roommate_lib::config::wants_service_mode() {
        if let Err(e) = roommate_lib::service::run_service_mode() {
            // Avoid MessageBox / WebView; write to ProgramData if possible.
            let _ = std::fs::create_dir_all(r"C:\ProgramData\Roommate-LAN\logs");
            let _ = std::fs::write(
                r"C:\ProgramData\Roommate-LAN\logs\service-start-error.txt",
                format!("{e}\n"),
            );
            std::process::exit(1);
        }
        return;
    }

    roommate_lib::run()
}
