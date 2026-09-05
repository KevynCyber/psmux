//! ZDEP-008: `extern "system"` declarations replacing the `windows-sys`
//! items this crate used (clipboard + drive-type checks). Edition 2021
//! extern blocks need no marker keyword before `fn`, so the token-inventory
//! allowlist in tests-rs/test_unsafe_inventory.rs is unaffected.

use std::ffi::c_void;

/// ZDEP-010/ZDEP-011: mirrors Win32 `SYSTEMTIME` (field order matches the
/// real struct so it can be passed by pointer to `GetLocalTime` /
/// `FileTimeToSystemTime`).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct SYSTEMTIME {
    pub wyear: u16,
    pub wmonth: u16,
    pub wdayofweek: u16,
    pub wday: u16,
    pub whour: u16,
    pub wminute: u16,
    pub wsecond: u16,
    pub wmilliseconds: u16,
}

/// Mirrors Win32 `FILETIME`: 100-nanosecond intervals since 1601-01-01 UTC.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct FILETIME {
    pub dwlowdatetime: u32,
    pub dwhighdatetime: u32,
}

#[link(name = "kernel32")]
extern "system" {
    pub fn GlobalAlloc(uflags: u32, dwbytes: usize) -> *mut c_void;
    pub fn GlobalLock(hmem: *mut c_void) -> *mut c_void;
    pub fn GlobalUnlock(hmem: *mut c_void) -> i32;
    pub fn GlobalSize(hmem: *mut c_void) -> usize;
    pub fn GlobalFree(hmem: *mut c_void) -> *mut c_void;
    pub fn GetDriveTypeW(lprootpathname: *const u16) -> u32;
    pub fn GetLocalTime(lpsystemtime: *mut SYSTEMTIME);
    pub fn FileTimeToLocalFileTime(lpfiletime: *const FILETIME, lplocalfiletime: *mut FILETIME) -> i32;
    pub fn FileTimeToSystemTime(lpfiletime: *const FILETIME, lpsystemtime: *mut SYSTEMTIME) -> i32;
}

#[link(name = "user32")]
extern "system" {
    pub fn OpenClipboard(hwndnewowner: *mut c_void) -> i32;
    pub fn CloseClipboard() -> i32;
    pub fn EmptyClipboard() -> i32;
    pub fn GetClipboardData(uformat: u32) -> *mut c_void;
    pub fn SetClipboardData(uformat: u32, hmem: *mut c_void) -> *mut c_void;
}

pub const GMEM_MOVEABLE: u32 = 0x0002;
pub const DRIVE_REMOTE: u32 = 4;
