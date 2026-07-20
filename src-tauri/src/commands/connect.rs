use tauri::{AppHandle, Emitter, Manager, State};

use crate::elev;
use crate::tailscale::{DaemonState, TailscaleCli};

#[tauri::command]
pub fn is_admin() -> bool {
    elev::is_elevated()
}

pub async fn connect_inner(app: &AppHandle, daemon: &DaemonState) -> Result<String, String> {
    match elev::ensure_elevated() {
        Ok(()) => {}
        Err(e) if e == "ELEVATION_RELAUNCH" => {
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(400));
                std::process::exit(0);
            });
            return Err("正在请求管理员权限，请在 UAC 对话框中确认…".into());
        }
        Err(e) => return Err(e),
    }

    let state_dir = crate::config::state_dir();
    daemon.start(app, &state_dir)?;

    let cli = TailscaleCli::new(app)?;
    cli.up()
}

#[tauri::command]
pub async fn connect(app: AppHandle, daemon: State<'_, DaemonState>) -> Result<String, String> {
    connect_inner(&app, daemon.inner()).await
}

#[tauri::command]
pub async fn disconnect(app: AppHandle, daemon: State<'_, DaemonState>) -> Result<String, String> {
    if let Ok(cli) = TailscaleCli::new(&app) {
        let _ = cli.down();
    }
    daemon.stop()?;
    Ok("已断开连接".into())
}

pub fn spawn_auto_connect_if_needed(app: &AppHandle) {
    if !crate::config::should_auto_connect() || !elev::is_elevated() {
        return;
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let daemon = handle.state::<DaemonState>();
        let result = tauri::async_runtime::block_on(connect_inner(&handle, daemon.inner()));
        let _ = handle.emit(
            "roommate-auto-connect",
            match result {
                Ok(msg) => serde_json::json!({ "ok": true, "message": msg }),
                Err(err) => serde_json::json!({ "ok": false, "message": err }),
            },
        );
    });
}
