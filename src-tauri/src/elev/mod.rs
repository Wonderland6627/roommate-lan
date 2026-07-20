#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{ensure_elevated, is_elevated};

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    true
}

#[cfg(not(windows))]
pub fn ensure_elevated() -> Result<(), String> {
    Ok(())
}
