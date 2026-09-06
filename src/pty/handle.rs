//! ZDEP-023: in-tree HANDLE wrapper, replacing the `filedescriptor` crate's
//! `OwnedHandle`/`FileDescriptor`. Ported from portable-pty-psmux 0.9.7 (MIT)
//! win/ code semantics: owns a Win32 HANDLE, closes it on drop, and supports
//! `Read`/`Write` via ReadFile/WriteFile plus `try_clone` via DuplicateHandle.
//! Used both for pipe endpoints (ConPTY stdin/stdout pipes) and process/thread
//! handles.

use super::ffi;
use std::io::{self, Read, Write};

#[derive(Debug)]
pub struct OwnedHandle(ffi::HANDLE);

// A Win32 HANDLE is safe to move between threads and to share behind a
// reference; nothing here is thread-affine (unlike e.g. a GUI window
// handle), and every access goes through a Win32 call that is itself
// thread-safe.
unsafe impl Send for OwnedHandle {}
unsafe impl Sync for OwnedHandle {}

impl OwnedHandle {
    /// Takes ownership of a raw handle. The caller must not use `handle`
    /// after this call except through the returned `OwnedHandle`.
    pub unsafe fn from_raw(handle: ffi::HANDLE) -> Self {
        Self(handle)
    }

    pub fn as_raw(&self) -> ffi::HANDLE {
        self.0
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        Self::dup_raw(self.0)
    }

    /// Duplicates a raw handle this `OwnedHandle` does not own (e.g. one
    /// borrowed from a live `std::process::Child`), without closing the
    /// source. Contrast with `from_raw`, which takes ownership and WILL
    /// close `handle` on drop -- calling `from_raw` on a handle someone else
    /// still owns double-closes it out from under them.
    pub fn dup_raw(handle: ffi::HANDLE) -> io::Result<Self> {
        let mut cloned: ffi::HANDLE = std::ptr::null_mut();
        let ok = unsafe {
            ffi::DuplicateHandle(
                ffi::GetCurrentProcess(),
                handle,
                ffi::GetCurrentProcess(),
                &mut cloned,
                0,
                1,
                ffi::DUPLICATE_SAME_ACCESS,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(cloned))
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != ffi::INVALID_HANDLE_VALUE {
            unsafe { ffi::CloseHandle(self.0 as isize) };
        }
    }
}

impl Read for OwnedHandle {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut bytes_read: u32 = 0;
        let ok = unsafe {
            ffi::ReadFile(
                self.0,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut bytes_read,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            let err = io::Error::last_os_error();
            // The write end of the pipe closed: treat as EOF, not an error,
            // matching the semantics readers of pair.master already rely on.
            return match err.raw_os_error() {
                Some(ffi::ERROR_BROKEN_PIPE) | Some(ffi::ERROR_HANDLE_EOF) => Ok(0),
                _ => Err(err),
            };
        }
        Ok(bytes_read as usize)
    }
}

impl Write for OwnedHandle {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut bytes_written: u32 = 0;
        let ok = unsafe {
            ffi::WriteFile(
                self.0,
                buf.as_ptr(),
                buf.len() as u32,
                &mut bytes_written,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(bytes_written as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
