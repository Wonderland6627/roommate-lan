use crate::service::ServiceClient;

#[tauri::command]
pub async fn ping_peer(ip: String) -> Result<u32, String> {
    ServiceClient::new().ping(&ip)
}
