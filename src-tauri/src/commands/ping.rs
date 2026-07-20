use tauri::AppHandle;

use crate::tailscale::TailscaleCli;

#[tauri::command]
pub async fn ping_peer(app: AppHandle, ip: String) -> Result<u32, String> {
    let cli = TailscaleCli::new(&app)?;
    cli.ping(&ip)
}
