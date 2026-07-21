use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use crate::config;

/// Resolved sidecar binaries next to the Roommate install / build output.
#[derive(Debug, Clone)]
pub struct EnginePaths {
    pub install_dir: PathBuf,
    pub tailscale: PathBuf,
    pub tailscaled: PathBuf,
}

impl EnginePaths {
    pub fn resolve() -> Result<Self, String> {
        let install_dir = std::env::current_exe()
            .map_err(|e| format!("无法定位程序路径: {e}"))?
            .parent()
            .ok_or_else(|| "无法解析安装目录".to_string())?
            .to_path_buf();

        let triple = host_triple();
        let tailscale = find_bin(&install_dir, "tailscale", triple)?;
        let tailscaled = find_bin(&install_dir, "tailscaled", triple)?;
        Ok(Self {
            install_dir,
            tailscale,
            tailscaled,
        })
    }

    pub fn ensure_wintun(&self) -> Result<(), String> {
        #[cfg(not(windows))]
        {
            return Ok(());
        }
        #[cfg(windows)]
        {
            let dest = self.install_dir.join("wintun.dll");
            if dest.is_file() {
                return Ok(());
            }
            let search = [
                self.install_dir.join("binaries").join("wintun.dll"),
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("binaries")
                    .join("wintun.dll"),
            ];
            for src in search {
                if !src.is_file() {
                    continue;
                }
                std::fs::copy(&src, &dest).map_err(|e| {
                    format!("无法将 wintun.dll 复制到 {}: {e}", dest.display())
                })?;
                return Ok(());
            }
            Err(format!(
                "缺少 wintun.dll（应与 {} 同目录）。请运行 npm run fetch-bins",
                self.tailscaled.display()
            ))
        }
    }
}

fn find_bin(install_dir: &Path, name: &str, triple: &str) -> Result<PathBuf, String> {
    let candidates = [
        install_dir.join(format!("{name}.exe")),
        install_dir.join(name),
        install_dir.join("binaries").join(format!("{name}.exe")),
        install_dir.join("binaries").join(name),
        install_dir.join(format!("{name}-{triple}.exe")),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(format!("{name}-{triple}.exe")),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(format!("{name}.exe")),
    ];
    for c in candidates {
        if c.is_file() {
            return Ok(c);
        }
    }
    Err(format!(
        "找不到内置 {name} sidecar。请运行 npm run fetch-bins 拉取二进制后再试"
    ))
}

/// Holds the spawned `tailscaled` child process for the service lifetime.
pub struct DaemonState {
    child: Mutex<Option<Child>>,
}

impl DaemonState {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }

    pub fn is_running(&self) -> bool {
        let mut guard = self.child.lock().expect("daemon mutex");
        match guard.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(None) => true,
                Ok(Some(_)) => {
                    *guard = None;
                    false
                }
                Err(_) => false,
            },
            None => false,
        }
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut guard = self.child.lock().map_err(|e| e.to_string())?;
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    pub fn cleanup(&self, paths: &EnginePaths, state_dir: &Path) {
        let mut cmd = Command::new(&paths.tailscaled);
        cmd.arg("--socket")
            .arg(config::tailscaled_socket())
            .arg("--statedir")
            .arg(state_dir)
            .arg("--cleanup")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let _ = cmd.status();
    }

    pub fn start(&self, paths: &EnginePaths, state_dir: &Path) -> Result<(), String> {
        #[cfg(windows)]
        {
            if official_tailscale_running() {
                return Err(
                    "检测到官方 Tailscale 正在运行。请先退出官方客户端（托盘退出或执行 net stop Tailscale），再连接 Roommate。"
                        .into(),
                );
            }
        }

        if self.is_running() {
            return Ok(());
        }

        std::fs::create_dir_all(state_dir)
            .map_err(|e| format!("无法创建状态目录 {}: {e}", state_dir.display()))?;
        let log_dir = config::log_dir();
        std::fs::create_dir_all(&log_dir)
            .map_err(|e| format!("无法创建日志目录 {}: {e}", log_dir.display()))?;

        paths.ensure_wintun()?;

        let state = state_dir.join("tailscaled.state");
        let log_path = log_dir.join("tailscaled.log");
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("无法打开日志 {}: {e}", log_path.display()))?;
        let log_err = log_file
            .try_clone()
            .map_err(|e| format!("无法克隆日志句柄: {e}"))?;

        let mut cmd = Command::new(&paths.tailscaled);
        cmd.arg("--statedir")
            .arg(state_dir)
            .arg("--state")
            .arg(&state)
            .arg("--socket")
            .arg(config::tailscaled_socket())
            .arg("--tun")
            .arg(config::tun_name())
            .arg("--no-logs-no-support")
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_err));

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let child = cmd.spawn().map_err(|e| {
            format!(
                "启动 tailscaled 失败 ({}): {e}",
                paths.tailscaled.display()
            )
        })?;

        let mut guard = self.child.lock().map_err(|e| e.to_string())?;
        *guard = Some(child);
        drop(guard);

        std::thread::sleep(Duration::from_millis(800));
        if !self.is_running() {
            let hint = read_log_tail(&log_path, 1200);
            return Err(format!(
                "tailscaled 启动后立即退出。请检查杀软是否拦截，并确认 wintun.dll 与 sidecar 同目录。日志: {}{}",
                log_path.display(),
                if hint.is_empty() {
                    String::new()
                } else {
                    format!("\n{hint}")
                }
            ));
        }
        Ok(())
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
pub fn official_tailscale_running() -> bool {
    Command::new("sc")
        .args(["query", "Tailscale"])
        .output()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.contains("RUNNING")
        })
        .unwrap_or(false)
}

#[cfg(not(windows))]
pub fn official_tailscale_running() -> bool {
    false
}

fn read_log_tail(path: &Path, max_bytes: usize) -> String {
    let Ok(data) = std::fs::read(path) else {
        return String::new();
    };
    if data.is_empty() {
        return String::new();
    }
    let start = data.len().saturating_sub(max_bytes);
    String::from_utf8_lossy(&data[start..]).trim().to_string()
}

fn host_triple() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_bin_prefers_manifest_binaries_in_dev() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
        if !dir.exists() {
            return;
        }
        // Smoke: resolve from CARGO_MANIFEST_DIR fallbacks when install_dir empty of bins.
        let triple = host_triple();
        let name = format!("tailscale-{triple}.exe");
        if dir.join(&name).is_file() || dir.join("tailscale.exe").is_file() {
            let r = find_bin(Path::new("."), "tailscale", triple);
            assert!(r.is_ok(), "{r:?}");
        }
    }
}
