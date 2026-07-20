use std::mem::size_of;
use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::config::CONNECT_FLAG;

pub fn is_elevated() -> bool {
    unsafe {
        let mut token: HANDLE = INVALID_HANDLE_VALUE;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        );
        CloseHandle(token);

        ok != 0 && elevation.TokenIsElevated != 0
    }
}

/// If not elevated, re-launch self via UAC (`runas`) and signal caller to exit.
pub fn ensure_elevated() -> Result<(), String> {
    if is_elevated() {
        return Ok(());
    }

    let exe = std::env::current_exe().map_err(|e| format!("无法定位程序路径: {e}"))?;
    let exe_wide = to_wide(exe.to_string_lossy().as_ref());
    let op = to_wide("runas");

    // Preserve args and ask the elevated process to resume connect.
    let mut parts: Vec<String> = std::env::args().skip(1).collect();
    if !parts.iter().any(|a| a == CONNECT_FLAG) {
        parts.push(CONNECT_FLAG.to_string());
    }
    let joined = parts.join(" ");
    let args = to_wide(&joined);

    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            op.as_ptr(),
            exe_wide.as_ptr(),
            if args.len() > 1 {
                args.as_ptr()
            } else {
                ptr::null()
            },
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    // ShellExecute returns > 32 on success
    if result as isize <= 32 {
        return Err("用户取消了管理员提权，或系统拒绝启动".into());
    }

    Err("ELEVATION_RELAUNCH".into())
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
