//! GUI-side client for RoommateNetworkService.

use crate::config;
use crate::service::protocol::{Op, Request, Response};
use crate::tailscale::NetworkStatus;

#[derive(Clone, Default)]
pub struct ServiceClient;

impl ServiceClient {
    pub fn new() -> Self {
        Self
    }

    #[cfg(windows)]
    fn call(&self, req: Request) -> Result<Response, String> {
        crate::service::pipe::client_transact(&req)
    }

    #[cfg(not(windows))]
    fn call(&self, _req: Request) -> Result<Response, String> {
        Err("网络服务仅支持 Windows".into())
    }

    pub fn health(&self) -> Result<Response, String> {
        let resp = self.call(Request {
            v: config::PROTOCOL_VERSION,
            op: Op::Health,
            hostname: None,
            ip: None,
        })?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "健康检查失败".into()));
        }
        if resp.protocol != Some(config::PROTOCOL_VERSION) {
            return Err(format!(
                "协议版本不匹配（服务 {:?}, 客户端 {}）。请重装 Roommate。",
                resp.protocol,
                config::PROTOCOL_VERSION
            ));
        }
        Ok(resp)
    }

    pub fn ready(&self) -> bool {
        self.health().map(|r| r.ready.unwrap_or(false)).unwrap_or(false)
    }

    pub fn connect(&self, hostname: &str) -> Result<String, String> {
        let _ = self.health()?;
        let resp = self.call(Request {
            v: config::PROTOCOL_VERSION,
            op: Op::Connect,
            hostname: Some(hostname.to_string()),
            ip: None,
        })?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "连接失败".into()));
        }
        Ok(resp.message.unwrap_or_else(|| "已连接".into()))
    }

    pub fn disconnect(&self) -> Result<String, String> {
        let resp = self.call(Request {
            v: config::PROTOCOL_VERSION,
            op: Op::Disconnect,
            hostname: None,
            ip: None,
        })?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "断开失败".into()));
        }
        Ok(resp.message.unwrap_or_else(|| "已断开连接".into()))
    }

    pub fn status(&self) -> Result<NetworkStatus, String> {
        let resp = self.call(Request {
            v: config::PROTOCOL_VERSION,
            op: Op::Status,
            hostname: None,
            ip: None,
        })?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "无法读取状态".into()));
        }
        resp.status.ok_or_else(|| "状态为空".into())
    }

    pub fn ping(&self, ip: &str) -> Result<u32, String> {
        let resp = self.call(Request {
            v: config::PROTOCOL_VERSION,
            op: Op::Ping,
            hostname: None,
            ip: Some(ip.to_string()),
        })?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "ping 失败".into()));
        }
        resp.latency_ms.ok_or_else(|| "无延迟数据".into())
    }

    pub fn version(&self) -> Result<String, String> {
        let resp = self.call(Request {
            v: config::PROTOCOL_VERSION,
            op: Op::Version,
            hostname: None,
            ip: None,
        })?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "版本查询失败".into()));
        }
        resp.version.ok_or_else(|| "无版本信息".into())
    }

    pub fn heartbeat(&self) -> Result<(), String> {
        let resp = self.call(Request {
            v: config::PROTOCOL_VERSION,
            op: Op::Heartbeat,
            hostname: None,
            ip: None,
        })?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "心跳失败".into()));
        }
        Ok(())
    }
}
