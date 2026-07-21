//! Embedded / runtime configuration for Headscale login.

use std::path::PathBuf;
use std::sync::Once;

static LOAD_ENV: Once = Once::new();

pub const SERVICE_NAME: &str = "RoommateNetworkService";
pub const SERVICE_DISPLAY_NAME: &str = "Roommate Network Service";
pub const SERVICE_FLAG: &str = "--roommate-service";
pub const PROTOCOL_VERSION: u32 = 1;
pub const SERVICE_PIPE_NAME: &str = r"\\.\pipe\Roommate\NetworkService";
/// Lease expires if the GUI stops talking for this long.
pub const LEASE_TIMEOUT_SECS: u64 = 30;

/// Load repo-root / next-to-exe `.env` into process env (does not override existing vars).
pub fn load_dotenv() {
    LOAD_ENV.call_once(|| {
        for path in candidate_env_paths() {
            if path.is_file() {
                let _ = apply_env_file(&path);
                break;
            }
        }
    });
}

fn candidate_env_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join(".env"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join(".env"));
            // tauri dev: target/debug/roommate.exe → repo root is ../../..
            paths.push(dir.join("../../../.env"));
            // release service next to install dir / target/release
            paths.push(dir.join("../../.env"));
        }
    }
    paths.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.env"));
    paths
}

fn apply_env_file(path: &std::path::Path) -> std::io::Result<()> {
    let content = std::fs::read_to_string(path)?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if key.is_empty() {
            continue;
        }
        if std::env::var_os(key).is_none() {
            std::env::set_var(key, value);
        }
    }
    Ok(())
}

pub fn login_server() -> String {
    load_dotenv();
    if let Ok(v) = std::env::var("ROOMMATE_LOGIN_SERVER") {
        if !v.is_empty() {
            return v;
        }
    }
    option_env!("ROOMMATE_LOGIN_SERVER")
        .unwrap_or("https://hs.example.com")
        .to_string()
}

pub fn auth_key() -> String {
    load_dotenv();
    if let Ok(v) = std::env::var("ROOMMATE_AUTH_KEY") {
        if !v.is_empty() {
            return v;
        }
    }
    option_env!("ROOMMATE_AUTH_KEY")
        .unwrap_or("tskey-auth-replace-me")
        .to_string()
}

/// Service-owned state under ProgramData (LocalSystem-safe).
pub fn state_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
            .join("Roommate-LAN")
            .join("tailscale")
    }
    #[cfg(not(windows))]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Roommate-LAN")
            .join("tailscale")
    }
}

pub fn log_dir() -> PathBuf {
    state_dir()
        .parent()
        .map(|p| p.join("logs"))
        .unwrap_or_else(|| PathBuf::from("logs"))
}

/// LocalAPI socket / named pipe shared by Roommate's private sidecar.
pub fn tailscaled_socket() -> String {
    #[cfg(windows)]
    {
        r"\\.\pipe\ProtectedPrefix\Administrators\Roommate\tailscaled".into()
    }
    #[cfg(not(windows))]
    {
        state_dir()
            .join("tailscaled.sock")
            .to_string_lossy()
            .into_owned()
    }
}

/// Wintun adapter name — distinct from the official "Tailscale" adapter.
pub fn tun_name() -> &'static str {
    "Roommate"
}

pub fn hostname() -> String {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "player".into());
    sanitize_hostname_part(&user)
}

pub fn sanitize_hostname_part(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(24)
        .collect();
    let base = if sanitized.is_empty() {
        "player".into()
    } else {
        sanitized
    };
    format!("roommate-{base}")
}

pub fn wants_service_mode() -> bool {
    if std::env::args().any(|a| a == SERVICE_FLAG) {
        return true;
    }
    // SCM / Session 0 must never open the Tauri/WebView UI (LocalSystem has no usable
    // Edge profile and would pop "无法创建数据目录").
    #[cfg(windows)]
    {
        return is_session_zero();
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// True when this process runs in Windows Session 0 (services).
#[cfg(windows)]
pub fn is_session_zero() -> bool {
    use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;

    let mut session = u32::MAX;
    let ok = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) };
    ok != 0 && session == 0
}

