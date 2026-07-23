use crate::config;
use crate::service::ServiceClient;

#[tauri::command]
pub fn is_admin() -> bool {
    // Legacy name kept for UI compatibility: true when network service is reachable.
    ServiceClient::new().ready()
}

#[tauri::command]
pub fn network_service_ready() -> bool {
    ServiceClient::new().ready()
}

#[tauri::command]
pub fn bootstrap_url() -> String {
    config::bootstrap_url()
}

#[tauri::command]
pub async fn connect(
    login_server: Option<String>,
    auth_key: Option<String>,
) -> Result<String, String> {
    let client = ServiceClient::new();
    let health = client.health().map_err(|e| {
        if e.contains("未就绪") || e.contains("无法连接") {
            format!("{e}（安装 Roommate 时会注册网络服务，日常使用无需再弹 UAC）")
        } else {
            e
        }
    })?;
    if health.ready != Some(true) {
        return Err("网络服务未就绪，请修复安装或运行 scripts/dev-service.ps1".into());
    }
    client.connect(
        &config::hostname(),
        login_server.as_deref(),
        auth_key.as_deref(),
    )
}

#[tauri::command]
pub async fn disconnect() -> Result<String, String> {
    ServiceClient::new().disconnect()
}

#[tauri::command]
pub async fn reset_engine() -> Result<String, String> {
    ServiceClient::new().reset_engine()
}
