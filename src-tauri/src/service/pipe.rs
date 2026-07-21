//! Named-pipe IPC (Windows).

#![cfg(windows)]

use std::fs::File;
use std::io::{Read, Write};
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::ptr;

use windows_sys::Win32::Foundation::{FALSE, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FlushFileBuffers, ReadFile, WriteFile, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING,
    PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, WaitNamedPipeW, PIPE_READMODE_BYTE,
    PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use crate::config;
use crate::service::engine::NetworkEngine;
use crate::service::protocol::{decode_body, encode_message, Request, Response};

/// SYSTEM/Admins full; Interactive Users + Builtin Users read/write (local only).
const SDDL: &str = "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)(A;;GRGW;;;BU)";

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn make_security_attrs() -> Result<SECURITY_ATTRIBUTES, String> {
    let mut sd_ptr: PSECURITY_DESCRIPTOR = ptr::null_mut();
    let wide = to_wide(SDDL);
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(wide.as_ptr(), 1, &mut sd_ptr, ptr::null_mut())
    };
    if ok == 0 || sd_ptr.is_null() {
        return Err(format!(
            "无法创建管道 ACL: {}",
            std::io::Error::last_os_error()
        ));
    }
    // Intentionally leak SD for the lifetime of the process accept loop / pipe instance.
    Ok(SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd_ptr,
        bInheritHandle: FALSE,
    })
}

pub fn create_server_pipe() -> Result<OwnedHandle, String> {
    let name = to_wide(config::SERVICE_PIPE_NAME);
    let attrs = make_security_attrs()?;
    let handle = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            64 * 1024,
            64 * 1024,
            0,
            &attrs,
        )
    };
    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        return Err(format!(
            "创建命名管道失败: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(handle as _) })
}

pub fn wait_for_client(server: &OwnedHandle) -> Result<(), String> {
    use std::os::windows::io::AsRawHandle;
    let raw = server.as_raw_handle() as HANDLE;
    let ok = unsafe { ConnectNamedPipe(raw, ptr::null_mut()) };
    if ok != 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    let code = err.raw_os_error().unwrap_or(0);
    // ERROR_PIPE_CONNECTED = 535
    if code == 535 {
        return Ok(());
    }
    Err(format!("等待客户端连接失败: {err}"))
}

pub fn disconnect_client(server: &OwnedHandle) {
    use std::os::windows::io::AsRawHandle;
    let raw = server.as_raw_handle() as HANDLE;
    unsafe {
        let _ = FlushFileBuffers(raw);
        let _ = DisconnectNamedPipe(raw);
    }
}

fn read_exact(handle: HANDLE, buf: &mut [u8]) -> Result<(), String> {
    let mut off = 0;
    while off < buf.len() {
        let mut read = 0u32;
        let ok = unsafe {
            ReadFile(
                handle,
                buf[off..].as_mut_ptr() as _,
                (buf.len() - off) as u32,
                &mut read,
                ptr::null_mut(),
            )
        };
        if ok == 0 || read == 0 {
            return Err(format!("读取管道失败: {}", std::io::Error::last_os_error()));
        }
        off += read as usize;
    }
    Ok(())
}

fn write_all(handle: HANDLE, buf: &[u8]) -> Result<(), String> {
    let mut off = 0;
    while off < buf.len() {
        let mut written = 0u32;
        let ok = unsafe {
            WriteFile(
                handle,
                buf[off..].as_ptr() as _,
                (buf.len() - off) as u32,
                &mut written,
                ptr::null_mut(),
            )
        };
        if ok == 0 || written == 0 {
            return Err(format!("写入管道失败: {}", std::io::Error::last_os_error()));
        }
        off += written as usize;
    }
    Ok(())
}

pub fn serve_one(engine: &NetworkEngine) -> Result<(), String> {
    use std::os::windows::io::AsRawHandle;
    let pipe = create_server_pipe()?;
    wait_for_client(&pipe)?;
    let handle = pipe.as_raw_handle() as HANDLE;
    let result = (|| {
        let mut len_buf = [0u8; 4];
        read_exact(handle, &mut len_buf)?;
        let len = u32::from_le_bytes(len_buf) as usize;
        if len == 0 || len > 8 * 1024 * 1024 {
            return Err("非法帧长度".into());
        }
        let mut body = vec![0u8; len];
        read_exact(handle, &mut body)?;
        let req: Request = decode_body(&body)?;
        let resp = engine.handle(req);
        let out = encode_message(&resp)?;
        write_all(handle, &out)?;
        unsafe {
            let _ = FlushFileBuffers(handle);
        }
        Ok(())
    })();
    disconnect_client(&pipe);
    drop(pipe);
    result
}

pub fn client_transact(req: &Request) -> Result<Response, String> {
    let name = to_wide(config::SERVICE_PIPE_NAME);
    let handle = open_client_with_retry(&name)?;
    let owned = unsafe { OwnedHandle::from_raw_handle(handle as _) };
    let mut file = File::from(owned);
    let out = encode_message(req)?;
    file.write_all(&out)
        .map_err(|e| format!("写入服务失败: {e}"))?;
    file.flush().ok();

    let mut len_buf = [0u8; 4];
    file.read_exact(&mut len_buf)
        .map_err(|e| format!("读取服务响应失败: {e}"))?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > 8 * 1024 * 1024 {
        return Err("非法响应长度".into());
    }
    let mut body = vec![0u8; len];
    file.read_exact(&mut body)
        .map_err(|e| format!("读取服务响应失败: {e}"))?;
    decode_body(&body)
}

fn open_client_with_retry(name: &[u16]) -> Result<HANDLE, String> {
    // Server recreates the listening instance after each RPC; brief gaps cause
    // ERROR_FILE_NOT_FOUND (2) or ERROR_PIPE_BUSY (231). Wait + retry.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut last_err = String::new();
    while std::time::Instant::now() < deadline {
        unsafe {
            let _ = WaitNamedPipeW(name.as_ptr(), 1000);
        }
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                0x8000_0000 | 0x4000_0000, // GENERIC_READ | GENERIC_WRITE
                0,
                ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            )
        };
        if handle != INVALID_HANDLE_VALUE && !handle.is_null() {
            return Ok(handle);
        }
        last_err = map_connect_error();
        let code = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        // Only retry transient pipe-not-ready / busy.
        if code != 2 && code != 231 {
            return Err(last_err);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Err(last_err)
}

fn map_connect_error() -> String {
    let err = std::io::Error::last_os_error();
    match err.raw_os_error().unwrap_or(0) {
        2 => "网络服务未就绪。请确认已安装 Roommate，或管理员运行 scripts/dev-service.ps1。"
            .into(),
        231 => "网络服务正忙，请稍后重试。".into(),
        5 => "无权连接网络服务。".into(),
        _ => format!("无法连接网络服务: {err}"),
    }
}
