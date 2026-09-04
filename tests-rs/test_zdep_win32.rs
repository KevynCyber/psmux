#![cfg(windows)]
// Covers: ZDEP-008
// Requirement: `src/win32.rs` (`#[cfg(windows)]`) declares, in
// `extern "system"` blocks linked to `kernel32` and `user32`, exactly the
// items previously imported from `windows-sys`: `GlobalAlloc`, `GlobalLock`,
// `GlobalUnlock`, `GlobalSize`, `GlobalFree`, `GMEM_MOVEABLE` (= 2),
// `OpenClipboard`, `CloseClipboard`, `EmptyClipboard`, `GetClipboardData`,
// `SetClipboardData`, `GetDriveTypeW`, plus `DRIVE_REMOTE` (= 4).
// `OpenClipboard`/`CloseClipboard` touch user clipboard state and are NOT
// exercised here.

use crate::win32::*;

/// The two constants replacing the windows-sys ones must keep the same
/// numeric values.
#[test]
fn constants_match_windows_sys_values() {
    assert_eq!(GMEM_MOVEABLE, 2);
    assert_eq!(DRIVE_REMOTE, 4);
}

/// GetDriveTypeW on the SystemDrive root must return a real drive-type
/// value and must NOT be DRIVE_REMOTE (the dev machine's system drive is
/// local).
#[test]
fn get_drive_type_w_on_system_drive_root() {
    let sysdrive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    let root = format!("{}\\", sysdrive);
    let mut wide: Vec<u16> = root.encode_utf16().collect();
    wide.push(0); // wide null terminator

    let drive_type = unsafe { GetDriveTypeW(wide.as_ptr()) };
    assert!(
        (2..=6).contains(&drive_type),
        "GetDriveTypeW must return a documented drive type (2..=6), got {}",
        drive_type
    );
    assert_ne!(drive_type, DRIVE_REMOTE, "the local SystemDrive root must not report as a network drive");
}

/// GlobalAlloc/GlobalLock/write/GlobalUnlock/GlobalSize/GlobalFree round-trip:
/// allocate 64 bytes, write a marker, verify GlobalSize reports at least 64,
/// and free returns null on success.
#[test]
fn global_alloc_lock_write_unlock_size_free_roundtrip() {
    unsafe {
        let hmem = GlobalAlloc(GMEM_MOVEABLE, 64);
        assert!(!hmem.is_null(), "GlobalAlloc(GMEM_MOVEABLE, 64) must succeed");

        let ptr = GlobalLock(hmem) as *mut u8;
        assert!(!ptr.is_null(), "GlobalLock must return a non-null pointer");
        let marker = b"psmux-zdep-win32-roundtrip-marker";
        std::ptr::copy_nonoverlapping(marker.as_ptr(), ptr, marker.len());
        GlobalUnlock(hmem);

        let size = GlobalSize(hmem);
        assert!(size >= 64, "GlobalSize must report at least the requested 64 bytes, got {}", size);

        let freed = GlobalFree(hmem);
        assert!(freed.is_null(), "GlobalFree must return null on success");
    }
}
