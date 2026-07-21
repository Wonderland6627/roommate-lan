//! Network engine owned exclusively by the Windows service.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config;
use crate::service::protocol::{Op, Request, Response};
use crate::tailscale::{DaemonState, EnginePaths, TailscaleCli};

pub struct NetworkEngine {
    paths: EnginePaths,
    daemon: DaemonState,
    connected: Mutex<bool>,
    last_touch: Mutex<Instant>,
}

impl NetworkEngine {
    pub fn new() -> Result<Self, String> {
        config::load_dotenv();
        Ok(Self {
            paths: EnginePaths::resolve()?,
            daemon: DaemonState::new(),
            connected: Mutex::new(false),
            last_touch: Mutex::new(Instant::now()),
        })
    }

    fn touch(&self) {
        if let Ok(mut g) = self.last_touch.lock() {
            *g = Instant::now();
        }
    }

    pub fn lease_expired(&self) -> bool {
        let connected = *self.connected.lock().unwrap_or_else(|e| e.into_inner());
        if !connected {
            return false;
        }
        let last = *self.last_touch.lock().unwrap_or_else(|e| e.into_inner());
        last.elapsed() > Duration::from_secs(config::LEASE_TIMEOUT_SECS)
    }

    pub fn enforce_lease(&self) {
        if !self.lease_expired() {
            return;
        }
        let _ = self.disconnect_inner();
    }

    pub fn handle(&self, req: Request) -> Response {
        if let Err(e) = crate::service::protocol::validate_request(&req) {
            return Response::err(req.op.clone(), e);
        }
        self.touch();
        match req.op {
            Op::Health => self.health(),
            Op::Connect => self.connect(req.hostname.as_deref().unwrap_or("")),
            Op::Disconnect => self.disconnect(),
            Op::Status => self.status(),
            Op::Ping => self.ping(req.ip.as_deref().unwrap_or("")),
            Op::Version => self.version(),
            Op::Heartbeat => {
                let connected = *self.connected.lock().unwrap_or_else(|e| e.into_inner());
                Response::ok_message(Op::Heartbeat, if connected { "ok" } else { "idle" })
            }
        }
    }

    fn health(&self) -> Response {
        let connected = *self.connected.lock().unwrap_or_else(|e| e.into_inner());
        Response::health(true, connected)
    }

    fn connect(&self, hostname: &str) -> Response {
        match self.connect_inner(hostname) {
            Ok(msg) => {
                *self.connected.lock().unwrap_or_else(|e| e.into_inner()) = true;
                self.touch();
                Response::ok_message(Op::Connect, msg)
            }
            Err(e) => Response::err(Op::Connect, e),
        }
    }

    fn connect_inner(&self, hostname: &str) -> Result<String, String> {
        let state_dir = config::state_dir();
        self.daemon.start(&self.paths, &state_dir)?;
        let cli = TailscaleCli::from_paths(&self.paths);
        cli.up(hostname)
    }

    fn disconnect(&self) -> Response {
        match self.disconnect_inner() {
            Ok(()) => Response::ok_message(Op::Disconnect, "已断开连接"),
            Err(e) => Response::err(Op::Disconnect, e),
        }
    }

    fn disconnect_inner(&self) -> Result<(), String> {
        let cli = TailscaleCli::from_paths(&self.paths);
        let _ = cli.down();
        self.daemon.stop()?;
        self.daemon.cleanup(&self.paths, &config::state_dir());
        *self.connected.lock().unwrap_or_else(|e| e.into_inner()) = false;
        Ok(())
    }

    fn status(&self) -> Response {
        let connected = *self.connected.lock().unwrap_or_else(|e| e.into_inner());
        if !connected && !self.daemon.is_running() {
            return Response::err(Op::Status, "尚未连接");
        }
        match TailscaleCli::from_paths(&self.paths).status() {
            Ok(status) => Response {
                ok: true,
                op: Op::Status,
                error: None,
                message: None,
                protocol: None,
                ready: None,
                connected: Some(connected),
                status: Some(status),
                latency_ms: None,
                version: None,
            },
            Err(e) => Response::err(Op::Status, e),
        }
    }

    fn ping(&self, ip: &str) -> Response {
        match TailscaleCli::from_paths(&self.paths).ping(ip) {
            Ok(ms) => Response {
                ok: true,
                op: Op::Ping,
                error: None,
                message: None,
                protocol: None,
                ready: None,
                connected: None,
                status: None,
                latency_ms: Some(ms),
                version: None,
            },
            Err(e) => Response::err(Op::Ping, e),
        }
    }

    fn version(&self) -> Response {
        match TailscaleCli::from_paths(&self.paths).version() {
            Ok(v) => Response {
                ok: true,
                op: Op::Version,
                error: None,
                message: None,
                protocol: None,
                ready: None,
                connected: None,
                status: None,
                latency_ms: None,
                version: Some(v),
            },
            Err(e) => Response::err(Op::Version, e),
        }
    }

    pub fn shutdown(&self) {
        let _ = self.disconnect_inner();
    }
}
