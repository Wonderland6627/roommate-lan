use crate::service::ServiceClient;
use crate::tailscale::NetworkStatus;

#[tauri::command]
pub async fn get_status() -> Result<NetworkStatus, String> {
    // Lease keepalive is owned by the Rust-side heartbeat thread after connect.
    ServiceClient::new().status()
}

#[tauri::command]
pub async fn sidecar_version() -> Result<String, String> {
    ServiceClient::new().version()
}
