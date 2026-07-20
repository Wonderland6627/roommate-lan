use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};

/// Holds the spawned `tailscaled` child process for the app lifetime.
pub struct DaemonState {
    child: Mutex<Option<Child>>,
    /// When true, we drive the official Windows Tailscale service (no sidecar daemon).
    pub use_system_service: Mutex<bool>,
}

impl DaemonState {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
            use_system_service: Mutex::new(false),
        }
    }

    pub fn is_running(&self) -> bool {
        if *self.use_system_service.lock().unwrap_or_else(|e| e.into_inner()) {
            return system_tailscale_running();
        }
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

    pub fn start(&self, app: &AppHandle, state_dir: &Path) -> Result<(), String> {
        // Windows: prefer official Tailscale service (Wintun). Sidecar CLI otherwise
        // talks to the wrong daemon and stays NeedsLogin forever.
        #[cfg(windows)]
        {
            if let Some(cli) = system_tailscale_cli() {
                let _ = cli;
                *self
                    .use_system_service
                    .lock()
                    .map_err(|e| e.to_string())? = true;
                if !system_tailscale_running() {
                    let _ = Command::new("sc")
                        .args(["start", "Tailscale"])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                    std::thread::sleep(Duration::from_millis(1200));
                }
                if !system_tailscale_running() {
                    return Err(
                        "已检测到官方 Tailscale，但服务未运行。请从开始菜单打开 Tailscale 后重试"
                            .into(),
                    );
                }
                return Ok(());
            }
        }

        *self
            .use_system_service
            .lock()
            .map_err(|e| e.to_string())? = false;

        if self.is_running() {
            return Ok(());
        }

        std::fs::create_dir_all(state_dir)
            .map_err(|e| format!("无法创建状态目录 {}: {e}", state_dir.display()))?;

        let bin = resolve_sidecar(app, "tailscaled")?;
        let state = state_dir.join("tailscaled.state");

        let mut cmd = Command::new(&bin);
        cmd.arg("--statedir")
            .arg(state_dir)
            .arg("--state")
            .arg(&state)
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("启动 tailscaled 失败 ({}): {e}", bin.display()))?;

        let mut guard = self.child.lock().map_err(|e| e.to_string())?;
        *guard = Some(child);

        std::thread::sleep(Duration::from_millis(800));
        if !self.is_running() {
            return Err(
                "tailscaled 启动后立即退出。Windows 请先安装官方 Tailscale，或检查杀软是否拦截"
                    .into(),
            );
        }
        Ok(())
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn system_tailscale_cli() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(r"C:\Program Files\Tailscale\tailscale.exe"),
        PathBuf::from(r"C:\Program Files (x86)\Tailscale\tailscale.exe"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn system_tailscale_running() -> bool {
    Command::new("sc")
        .args(["query", "Tailscale"])
        .output()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.contains("RUNNING")
        })
        .unwrap_or(false)
}

pub fn resolve_cli(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(sys) = system_tailscale_cli() {
        return Ok(sys);
    }
    resolve_sidecar(app, "tailscale")
}

pub fn resolve_sidecar(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidates = [
                dir.join(format!("{name}.exe")),
                dir.join(name),
                dir.join("binaries").join(format!("{name}.exe")),
                dir.join("binaries").join(name),
            ];
            for c in candidates {
                if c.is_file() {
                    return Ok(c);
                }
            }
        }
    }

    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("resource_dir: {e}"))?;

    let triple = host_triple();
    let candidates = [
        resource_dir
            .join("binaries")
            .join(format!("{name}-{triple}.exe")),
        resource_dir.join("binaries").join(format!("{name}.exe")),
        resource_dir.join(format!("{name}-{triple}.exe")),
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
        "找不到 Tailscale。请安装官方 Tailscale，或运行 npm run fetch-bins 拉取 sidecar"
    ))
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
