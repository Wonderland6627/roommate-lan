use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::config;
use crate::tailscale::process::resolve_cli;
use crate::tailscale::status_parse::{parse_status_json, NetworkStatus};

pub struct TailscaleCli {
    bin: PathBuf,
    /// When using sidecar daemon; unused for system Windows service.
    state_dir: PathBuf,
    use_system_service: bool,
}

impl TailscaleCli {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let use_system_service = crate::tailscale::process::system_tailscale_cli().is_some();
        Ok(Self {
            bin: resolve_cli(app)?,
            state_dir: config::state_dir(),
            use_system_service,
        })
    }

    fn base_cmd(&self) -> Command {
        let mut cmd = Command::new(&self.bin);
        if !self.use_system_service {
            // Best-effort hint for unix-style sockets; Windows system service ignores this.
            cmd.env("TS_DEBUG_SOCKET_PATH", self.state_dir.join("tailscaled.sock"));
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd
    }

    pub fn version(&self) -> Result<String, String> {
        self.run_args(&["version"], Duration::from_secs(15))
    }

    pub fn up(&self) -> Result<String, String> {
        let login = config::login_server();
        let key = config::auth_key();
        if key.contains("replace-me") || key.is_empty() {
            return Err("未配置 AuthKey。请在 .env 设置 ROOMMATE_AUTH_KEY".into());
        }

        let hostname = config::hostname();
        let login_ref = login.as_str();
        let key_ref = key.as_str();
        let host_ref = hostname.as_str();
        let args = [
            "up",
            "--login-server",
            login_ref,
            "--authkey",
            key_ref,
            "--hostname",
            host_ref,
            "--accept-dns=false",
            "--accept-routes=false",
            "--reset",
        ];

        let out = self.run_args(&args, Duration::from_secs(45))?;

        // Confirm we actually logged in (up can return while still NeedsLogin).
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(500));
            if let Ok(st) = self.status() {
                let state = st.backend_state.to_lowercase();
                if state == "running" && !st.self_ips.is_empty() {
                    return Ok(format!(
                        "已连接 {} ({})",
                        st.self_ips.join(", "),
                        st.self_hostname
                    ));
                }
            }
        }

        let st = self.status().ok();
        let detail = st
            .as_ref()
            .map(|s| format!("BackendState={}, ips={:?}", s.backend_state, s.self_ips))
            .unwrap_or_else(|| "无法读取 status".into());

        Err(format!(
            "登录未完成（{detail}）。请确认 AuthKey 有效、login-server 为 {login}。输出: {out}"
        ))
    }

    pub fn down(&self) -> Result<String, String> {
        self.run_args(&["down"], Duration::from_secs(30))
    }

    pub fn status(&self) -> Result<NetworkStatus, String> {
        let out = self.run_args(&["status", "--json"], Duration::from_secs(15))?;
        parse_status_json(&out)
    }

    pub fn ping(&self, ip: &str) -> Result<u32, String> {
        if ip.is_empty() {
            return Err("IP 为空".into());
        }
        let json_try =
            self.run_args(&["ping", "--json", "-c", "1", ip], Duration::from_secs(15));
        if let Ok(out) = json_try {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&out) {
                if let Some(ms) = v
                    .get("latency")
                    .and_then(|x| x.as_str())
                    .and_then(parse_duration_ms)
                    .or_else(|| {
                        v.get("LatencySeconds")
                            .and_then(|x| x.as_f64())
                            .map(|s| (s * 1000.0) as u32)
                    })
                {
                    return Ok(ms);
                }
            }
        }

        let out = self.run_args(&["ping", "-c", "1", ip], Duration::from_secs(15))?;
        parse_ping_text(&out).ok_or_else(|| format!("无法解析 ping 输出: {out}"))
    }

    fn run_args(&self, args: &[&str], timeout: Duration) -> Result<String, String> {
        let mut cmd = self.base_cmd();
        cmd.args(args);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("执行 tailscale {} 失败: {e}", args.join(" ")))?;

        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let mut stdout = String::new();
                    let mut stderr = String::new();
                    if let Some(mut out) = child.stdout.take() {
                        let _ = out.read_to_string(&mut stdout);
                    }
                    if let Some(mut err) = child.stderr.take() {
                        let _ = err.read_to_string(&mut stderr);
                    }
                    let stdout = stdout.trim().to_string();
                    let stderr = stderr.trim().to_string();

                    if !status.success() {
                        let msg = if !stderr.is_empty() {
                            stderr
                        } else if !stdout.is_empty() {
                            stdout
                        } else {
                            format!("exit {status}")
                        };
                        return Err(msg);
                    }
                    return Ok(if !stdout.is_empty() { stdout } else { stderr });
                }
                Ok(None) => {
                    if started.elapsed() > timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(format!(
                            "命令超时 ({}s): tailscale {}",
                            timeout.as_secs(),
                            args.join(" ")
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => return Err(format!("等待命令结束失败: {e}")),
            }
        }
    }
}

fn parse_duration_ms(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(rest) = s.strip_suffix("ms") {
        return rest.parse::<f64>().ok().map(|v| v.round() as u32);
    }
    if let Some(rest) = s.strip_suffix('s') {
        return rest.parse::<f64>().ok().map(|v| (v * 1000.0).round() as u32);
    }
    None
}

fn parse_ping_text(out: &str) -> Option<u32> {
    for part in out.split_whitespace() {
        if let Some(ms) = parse_duration_ms(part.trim_end_matches(',')) {
            return Some(ms);
        }
        if part.ends_with("ms") {
            if let Some(ms) = parse_duration_ms(part) {
                return Some(ms);
            }
        }
    }
    if let Some(idx) = out.find(" in ") {
        let rest = &out[idx + 4..];
        let token = rest.split_whitespace().next()?;
        return parse_duration_ms(token);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ping_samples() {
        assert_eq!(
            parse_ping_text("pong from roommate-bob (100.64.0.2) via DERP(txy) in 45.2ms"),
            Some(45)
        );
        assert_eq!(parse_duration_ms("12.6ms"), Some(13));
    }
}
