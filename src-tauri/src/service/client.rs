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

    fn req(op: Op) -> Request {
        Request {
            v: config::PROTOCOL_VERSION,
            op,
            hostname: None,
            ip: None,
            login_server: None,
            auth_key: None,
        }
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
        let resp = self.call(Self::req(Op::Health))?;
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

    pub fn connect(
        &self,
        hostname: &str,
        login_server: Option<&str>,
        auth_key: Option<&str>,
    ) -> Result<String, String> {
        let _ = self.health()?;
        let mut req = Self::req(Op::Connect);
        req.hostname = Some(hostname.to_string());
        req.login_server = login_server
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        req.auth_key = auth_key
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let resp = self.call(req)?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "连接失败".into()));
        }
        Ok(resp.message.unwrap_or_else(|| "已连接".into()))
    }

    pub fn disconnect(&self) -> Result<String, String> {
        let resp = self.call(Self::req(Op::Disconnect))?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "断开失败".into()));
        }
        Ok(resp.message.unwrap_or_else(|| "已断开连接".into()))
    }

    pub fn reset_engine(&self) -> Result<String, String> {
        let resp = self.call(Self::req(Op::ResetEngine))?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "重置网络引擎失败".into()));
        }
        Ok(resp.message.unwrap_or_else(|| "网络引擎已重置".into()))
    }

    pub fn status(&self) -> Result<NetworkStatus, String> {
        let resp = self.call(Self::req(Op::Status))?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "无法读取状态".into()));
        }
        resp.status.ok_or_else(|| "状态为空".into())
    }

    pub fn ping(&self, ip: &str) -> Result<u32, String> {
        let mut req = Self::req(Op::Ping);
        req.ip = Some(ip.to_string());
        let resp = self.call(req)?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "ping 失败".into()));
        }
        resp.latency_ms.ok_or_else(|| "无延迟数据".into())
    }

    pub fn version(&self) -> Result<String, String> {
        let resp = self.call(Self::req(Op::Version))?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "版本查询失败".into()));
        }
        resp.version.ok_or_else(|| "无版本信息".into())
    }

    pub fn heartbeat(&self) -> Result<(), String> {
        let resp = self.call(Self::req(Op::Heartbeat))?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "心跳失败".into()));
        }
        Ok(())
    }
}
