//! Fixed-command IPC protocol between GUI and RoommateNetworkService.

use serde::{Deserialize, Serialize};

use crate::config;
use crate::tailscale::NetworkStatus;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Op {
    Health,
    Connect,
    Disconnect,
    Status,
    Ping,
    Version,
    Heartbeat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub v: u32,
    pub op: Op,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub ip: Option<String>,
    #[serde(default)]
    pub login_server: Option<String>,
    #[serde(default)]
    pub auth_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub ok: bool,
    pub op: Op,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<NetworkStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl Response {
    pub fn err(op: Op, error: impl Into<String>) -> Self {
        Self {
            ok: false,
            op,
            error: Some(error.into()),
            message: None,
            protocol: None,
            ready: None,
            connected: None,
            status: None,
            latency_ms: None,
            version: None,
        }
    }

    pub fn health(ready: bool, connected: bool) -> Self {
        Self {
            ok: true,
            op: Op::Health,
            error: None,
            message: None,
            protocol: Some(config::PROTOCOL_VERSION),
            ready: Some(ready),
            connected: Some(connected),
            status: None,
            latency_ms: None,
            version: None,
        }
    }

    pub fn ok_message(op: Op, message: impl Into<String>) -> Self {
        Self {
            ok: true,
            op,
            error: None,
            message: Some(message.into()),
            protocol: None,
            ready: None,
            connected: None,
            status: None,
            latency_ms: None,
            version: None,
        }
    }
}

pub fn encode_message(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let body = serde_json::to_vec(value).map_err(|e| format!("JSON 编码失败: {e}"))?;
    if body.len() > 8 * 1024 * 1024 {
        return Err("消息过大".into());
    }
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn decode_body<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|e| format!("JSON 解析失败: {e}"))
}

pub fn validate_request(req: &Request) -> Result<(), String> {
    if req.v != config::PROTOCOL_VERSION {
        return Err(format!(
            "协议版本不匹配（客户端 {}, 服务 {}）。请重装 Roommate。",
            req.v,
            config::PROTOCOL_VERSION
        ));
    }
    match req.op {
        Op::Ping => {
            let ip = req.ip.as_deref().unwrap_or("").trim();
            if ip.is_empty() {
                return Err("IP 为空".into());
            }
            if !ip
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == '.' || c == ':')
                || ip.len() > 45
            {
                return Err("非法 IP".into());
            }
        }
        Op::Connect => {
            if let Some(h) = &req.hostname {
                if h.len() > 48
                    || !h
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    return Err("非法 hostname".into());
                }
            }
            if let Some(key) = &req.auth_key {
                if key.len() > 512 || key.chars().any(|c| c.is_control()) {
                    return Err("非法 AuthKey".into());
                }
            }
            if let Some(server) = &req.login_server {
                let s = server.trim();
                if s.len() > 256
                    || !(s.starts_with("https://") || s.starts_with("http://"))
                    || s.chars().any(|c| c.is_whitespace() || c.is_control())
                {
                    return Err("非法 login server".into());
                }
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_frame() {
        let req = Request {
            v: 1,
            op: Op::Health,
            hostname: None,
            ip: None,
            login_server: None,
            auth_key: None,
        };
        let bytes = encode_message(&req).unwrap();
        assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize, bytes.len() - 4);
        let decoded: Request = decode_body(&bytes[4..]).unwrap();
        assert_eq!(decoded.op, Op::Health);
    }

    #[test]
    fn rejects_bad_protocol() {
        let req = Request {
            v: 99,
            op: Op::Health,
            hostname: None,
            ip: None,
            login_server: None,
            auth_key: None,
        };
        assert!(validate_request(&req).is_err());
    }

    #[test]
    fn rejects_injection_ip() {
        let req = Request {
            v: 1,
            op: Op::Ping,
            hostname: None,
            ip: Some("1.1.1.1 && calc".into()),
            login_server: None,
            auth_key: None,
        };
        assert!(validate_request(&req).is_err());
    }
}
