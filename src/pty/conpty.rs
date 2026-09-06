//! ZDEP-023: port of portable-pty-psmux 0.9.7 (MIT) `src/win/conpty.rs`.

use super::cmdbuilder::CommandBuilder;
use super::ffi;
use super::handle::OwnedHandle;
use super::psuedocon::PsuedoCon;
use super::{Child, Error, MasterPty, PtyPair, PtySize, PtySystem, SlavePty};
use std::io;
use std::sync::{Arc, Mutex};

/// Create a pipe pair with an explicit buffer size.
///
/// Windows Terminal uses 128 KB pipe buffers for ConPTY I/O. The default
/// `CreatePipe(..., 0)` typically gets 4 KB, which forces more frequent
/// kernel transitions during high-throughput output (e.g. `cat large_file`).
/// Using 64 KB matches Windows Terminal's approach and reduces syscall
/// overhead for both input (mouse/keyboard) and output.
fn create_pipe_with_buffer(size: u32) -> io::Result<(OwnedHandle, OwnedHandle)> {
    let sa = ffi::SECURITY_ATTRIBUTES {
        n_length: std::mem::size_of::<ffi::SECURITY_ATTRIBUTES>() as u32,
        lp_security_descriptor: std::ptr::null_mut(),
        b_inherit_handle: 1,
    };
    let mut read: ffi::HANDLE = ffi::INVALID_HANDLE_VALUE;
    let mut write: ffi::HANDLE = ffi::INVALID_HANDLE_VALUE;
    if unsafe { ffi::CreatePipe(&mut read, &mut write, &sa, size) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { (OwnedHandle::from_raw(read), OwnedHandle::from_raw(write)) })
}

#[derive(Default)]
pub struct ConPtySystem {}

impl PtySystem for ConPtySystem {
    fn openpty(&self, size: PtySize) -> Result<PtyPair, Error> {
        // Use 64KB pipe buffers (Windows Terminal uses 128KB).
        // Default CreatePipe(..., 0) = ~4KB, causing frequent kernel round-trips.
        const PIPE_BUF: u32 = 64 * 1024;
        let (stdin_read, stdin_write) = create_pipe_with_buffer(PIPE_BUF)?;
        let (stdout_read, stdout_write) = create_pipe_with_buffer(PIPE_BUF)?;

        let con = PsuedoCon::new(
            ffi::COORD { x: size.cols as i16, y: size.rows as i16 },
            stdin_read,
            stdout_write,
        )?;

        let master = ConPtyMasterPty {
            inner: Arc::new(Mutex::new(Inner {
                con,
                readable: stdout_read,
                writable: Some(stdin_write),
                size,
            })),
        };

        let slave = ConPtySlavePty {
            inner: master.inner.clone(),
        };

        Ok(PtyPair {
            master: Box::new(master),
            slave: Box::new(slave),
        })
    }
}

struct Inner {
    con: PsuedoCon,
    readable: OwnedHandle,
    writable: Option<OwnedHandle>,
    size: PtySize,
}

impl Inner {
    pub fn resize(
        &mut self,
        num_rows: u16,
        num_cols: u16,
        pixel_width: u16,
        pixel_height: u16,
    ) -> Result<(), Error> {
        self.con.resize(ffi::COORD { x: num_cols as i16, y: num_rows as i16 })?;
        self.size = PtySize { rows: num_rows, cols: num_cols, pixel_width, pixel_height };
        Ok(())
    }
}

#[derive(Clone)]
pub struct ConPtyMasterPty {
    inner: Arc<Mutex<Inner>>,
}

pub struct ConPtySlavePty {
    inner: Arc<Mutex<Inner>>,
}

impl MasterPty for ConPtyMasterPty {
    fn resize(&self, size: PtySize) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.resize(size.rows, size.cols, size.pixel_width, size.pixel_height)
    }

    fn get_size(&self) -> Result<PtySize, Error> {
        let inner = self.inner.lock().unwrap();
        Ok(inner.size)
    }

    fn try_clone_reader(&self) -> Result<Box<dyn std::io::Read + Send>, Error> {
        Ok(Box::new(self.inner.lock().unwrap().readable.try_clone()?))
    }

    fn take_writer(&self) -> Result<Box<dyn std::io::Write + Send>, Error> {
        Ok(Box::new(
            self.inner
                .lock()
                .unwrap()
                .writable
                .take()
                .ok_or_else(|| io::Error::other("writer already taken"))?,
        ))
    }

    fn conpty_passthrough_mode(&self) -> Option<bool> {
        Some(self.inner.lock().unwrap().con.used_passthrough)
    }
}

impl SlavePty for ConPtySlavePty {
    fn spawn_command(&self, cmd: CommandBuilder) -> Result<Box<dyn Child + Send + Sync>, Error> {
        let mut inner = self.inner.lock().unwrap();
        match inner.con.spawn_command(cmd.clone()) {
            Ok(child) => Ok(Box::new(child)),
            Err(e) if inner.con.used_passthrough && is_invalid_parameter(&e) => {
                // CreateProcessW rejected the ConPTY handle that was created
                // with PSEUDOCONSOLE_PASSTHROUGH_MODE. Some Windows 11 builds
                // (notably Insider/Canary builds like 26200) accept the flag
                // during CreatePseudoConsole but later fail in
                // CreateProcessW with ERROR_INVALID_PARAMETER (87).
                //
                // Recovery: recreate the ConPTY without passthrough mode and
                // create fresh pipe pairs for the new pseudo-console.
                const PIPE_BUF: u32 = 64 * 1024;
                let (stdin_read, stdin_write) = create_pipe_with_buffer(PIPE_BUF)?;
                let (stdout_read, stdout_write) = create_pipe_with_buffer(PIPE_BUF)?;

                let new_con = PsuedoCon::new_without_passthrough(
                    ffi::COORD { x: inner.size.cols as i16, y: inner.size.rows as i16 },
                    stdin_read,
                    stdout_write,
                )?;

                // Replace the ConPTY and pipe endpoints inside Inner. At this
                // point nobody has cloned the reader or taken the writer yet
                // (pane.rs acquires them after spawn_command), so the old
                // handles are dropped cleanly.
                inner.con = new_con;
                inner.readable = stdout_read;
                inner.writable = Some(stdin_write);

                let child = inner.con.spawn_command(cmd)?;
                Ok(Box::new(child))
            }
            Err(e) => Err(e),
        }
    }
}

/// Check whether an error is Windows ERROR_INVALID_PARAMETER (87). The OS
/// error number is locale-independent; the textual message varies (e.g.
/// "Falscher Parameter" in German), so this checks the raw code rather than
/// scanning the formatted message.
fn is_invalid_parameter(e: &io::Error) -> bool {
    e.raw_os_error() == Some(87)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHILD_ENV: &str = "PORTABLE_PTY_CONPTY_PASSTHROUGH_TEST_CHILD";

    #[test]
    fn reports_when_passthrough_is_disabled_by_environment() {
        if std::env::var_os(CHILD_ENV).is_some() {
            let pair = ConPtySystem::default().openpty(PtySize::default()).unwrap();

            assert_eq!(pair.master.conpty_passthrough_mode(), Some(false));
            return;
        }

        let exe = std::env::current_exe().unwrap();
        let output = std::process::Command::new(exe)
            .args([
                "--exact",
                "pty::conpty::tests::reports_when_passthrough_is_disabled_by_environment",
            ])
            .env("PSMUX_NO_PASSTHROUGH", "1")
            .env(CHILD_ENV, "1")
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "child test failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
