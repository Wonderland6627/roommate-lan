use tauri::AppHandle;

use crate::tailscale::{NetworkStatus, TailscaleCli};

#[tauri::command]
pub async fn get_status(app: AppHandle) -> Result<NetworkStatus, String> {
    let cli = TailscaleCli::new(&app)?;
    cli.status()
}

#[tauri::command]
pub async fn sidecar_version(app: AppHandle) -> Result<String, String> {
    let cli = TailscaleCli::new(&app)?;
    cli.version()
}
