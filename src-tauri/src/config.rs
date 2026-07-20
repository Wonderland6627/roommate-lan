//! Embedded / runtime configuration for Headscale login.

use std::path::PathBuf;
use std::sync::Once;

static LOAD_ENV: Once = Once::new();

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

pub fn state_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Roommate-LAN")
        .join("tailscale")
}

pub fn hostname() -> String {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "player".into());
    let sanitized: String = user
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
    format!("roommate-{sanitized}")
}

pub const CONNECT_FLAG: &str = "--roommate-connect";

pub fn should_auto_connect() -> bool {
    std::env::args().any(|a| a == CONNECT_FLAG)
}
