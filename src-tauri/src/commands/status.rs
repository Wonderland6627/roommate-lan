use crate::service::ServiceClient;
use crate::tailscale::NetworkStatus;

#[tauri::command]
pub async fn get_status() -> Result<NetworkStatus, String> {
    let client = ServiceClient::new();
    // Status polling also refreshes the service-side lease.
    let _ = client.heartbeat();
    client.status()
}

#[tauri::command]
pub async fn sidecar_version() -> Result<String, String> {
    ServiceClient::new().version()
}
