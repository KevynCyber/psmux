//! ZDEP-008: `extern "system"` declarations replacing the `windows-sys`
//! items this crate used (clipboard + drive-type checks). Edition 2021
//! extern blocks carry no `unsafe` token, so the unsafe-inventory allowlist
//! is unaffected.

use std::ffi::c_void;

#[link(name = "kernel32")]
extern "system" {
    pub fn GlobalAlloc(uflags: u32, dwbytes: usize) -> *mut c_void;
    pub fn GlobalLock(hmem: *mut c_void) -> *mut c_void;
    pub fn GlobalUnlock(hmem: *mut c_void) -> i32;
    pub fn GlobalSize(hmem: *mut c_void) -> usize;
    pub fn GlobalFree(hmem: *mut c_void) -> *mut c_void;
    pub fn GetDriveTypeW(lprootpathname: *const u16) -> u32;
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
