mod client;
mod engine;
mod protocol;

#[cfg(windows)]
mod pipe;
#[cfg(windows)]
mod windows_svc;

pub use client::ServiceClient;
pub use protocol::{Op, Request, Response};

#[cfg(windows)]
pub fn run_service_mode() -> Result<(), String> {
    windows_svc::run_service()
}

#[cfg(not(windows))]
pub fn run_service_mode() -> Result<(), String> {
    Err("服务模式仅支持 Windows".into())
}
