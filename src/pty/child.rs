//! ZDEP-023: port of portable-pty-psmux 0.9.7 (MIT) `src/win/mod.rs`'s
//! `WinChild`/`WinChildKiller`. Includes the issue #446 poison-tolerant
//! mutex fix and its regression tests, preserved verbatim.

use super::ffi;
use super::handle::OwnedHandle;
use super::{Child, ChildKiller, ExitStatus};
use std::io::{Error as IoError, Result as IoResult};
use std::sync::Mutex;

#[derive(Debug)]
pub struct WinChild {
    pub(crate) proc: Mutex<OwnedHandle>,
}

impl WinChild {
    fn is_complete(&mut self) -> IoResult<Option<ExitStatus>> {
        let mut status: u32 = 0;
        let proc = self
            .proc
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_clone()
            .unwrap();
        let res = unsafe { ffi::GetExitCodeProcess(proc.as_raw(), &mut status) };
        if res != 0 {
            if status == ffi::STILL_ACTIVE {
                Ok(None)
            } else {
                Ok(Some(ExitStatus::with_exit_code(status)))
            }
        } else {
            Ok(None)
        }
    }

    fn do_kill(&mut self) -> IoResult<()> {
        let proc = self
            .proc
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_clone()
            .unwrap();
        let res = unsafe { ffi::TerminateProcess(proc.as_raw() as isize, 1) };
        let err = IoError::last_os_error();
        if res == 0 {
            Err(err)
        } else {
            Ok(())
        }
    }
}

impl ChildKiller for WinChild {
    fn kill(&mut self) -> IoResult<()> {
        self.do_kill().ok();
        Ok(())
    }

    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        let proc = self
            .proc
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_clone()
            .unwrap();
        Box::new(WinChildKiller { proc })
    }
}

#[derive(Debug)]
pub struct WinChildKiller {
    proc: OwnedHandle,
}

impl ChildKiller for WinChildKiller {
    fn kill(&mut self) -> IoResult<()> {
        let res = unsafe { ffi::TerminateProcess(self.proc.as_raw() as isize, 1) };
        let err = IoError::last_os_error();
        if res == 0 {
            Err(err)
        } else {
            Ok(())
        }
    }

    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        let proc = self.proc.try_clone().unwrap();
        Box::new(WinChildKiller { proc })
    }
}

impl Child for WinChild {
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        self.is_complete()
    }

    fn wait(&mut self) -> IoResult<ExitStatus> {
        if let Ok(Some(status)) = self.try_wait() {
            return Ok(status);
        }
        let proc = self
            .proc
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_clone()
            .unwrap();
        unsafe {
            ffi::WaitForSingleObject(proc.as_raw(), ffi::INFINITE);
        }
        let mut status: u32 = 0;
        let res = unsafe { ffi::GetExitCodeProcess(proc.as_raw(), &mut status) };
        if res != 0 {
            Ok(ExitStatus::with_exit_code(status))
        } else {
            Err(IoError::last_os_error())
        }
    }

    fn process_id(&self) -> Option<u32> {
        let res = unsafe {
            ffi::GetProcessId(self.proc.lock().unwrap_or_else(|e| e.into_inner()).as_raw() as isize)
        };
        if res == 0 {
            None
        } else {
            Some(res)
        }
    }

    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        let proc = self.proc.lock().unwrap_or_else(|e| e.into_inner());
        Some(proc.as_raw() as std::os::windows::io::RawHandle)
    }
}

#[cfg(test)]
mod tests_issue446 {
    // Issue #446: a panic while holding WinChild's `proc` mutex poisons it, and
    // every method then does `self.proc.lock().unwrap_or_else(|e| e.into_inner())` which panics on a
    // poisoned mutex. Result: the pane's child can no longer be queried, waited
    // on, or killed, so it leaks and becomes permanently un-killable.
    //
    // The poisoning path is reachable from this very file: e.g.
    // `self.proc.lock().unwrap_or_else(|e| e.into_inner()).try_clone().unwrap()` keeps the guard alive
    // across the `try_clone().unwrap()`, so if `try_clone` fails (handle
    // exhaustion) the panic unwinds while the lock is held and poisons it.
    //
    // These tests build a WinChild over a REAL, live OS process, poison the
    // exact mutex the methods lock, then prove the child stays queryable and
    // killable. They PANIC (fail) before the fix and pass after it.
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::process::{Command, Stdio};

    // Spawn a genuinely long-lived process and wrap a duplicated handle in a
    // WinChild, exactly the kind of full-access handle spawn_command produces.
    fn spawn_live_winchild() -> (WinChild, std::process::Child) {
        let child = Command::new("ping")
            .args(["-n", "30", "127.0.0.1"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn ping test child");
        let raw = std::os::windows::io::AsRawHandle::as_raw_handle(&child);
        let proc = OwnedHandle::dup_raw(raw as ffi::HANDLE).expect("duplicate process handle");
        (
            WinChild {
                proc: Mutex::new(proc),
            },
            child,
        )
    }

    // Poison `proc` the way an unrelated teardown panic would: hold the guard
    // and unwind through it.
    fn poison_proc(win: &WinChild) {
        let res = catch_unwind(AssertUnwindSafe(|| {
            let _guard = win.proc.lock().unwrap();
            panic!("simulated panic while holding proc lock (try_clone().unwrap() failure)");
        }));
        assert!(res.is_err(), "the panic must unwind so the lock is poisoned");
        assert!(win.proc.is_poisoned(), "proc mutex must be poisoned now");
    }

    #[test]
    fn poisoned_mutex_still_kills_and_waits() {
        let (mut win, mut sys_child) = spawn_live_winchild();

        // Baseline: process is alive and identifiable.
        let pid_before = win.process_id();
        assert!(
            pid_before.is_some(),
            "process_id should resolve for a live child"
        );

        poison_proc(&win);

        // Each of these panics on master (BUG). After the fix they must not.
        let pid = catch_unwind(AssertUnwindSafe(|| win.process_id()))
            .expect("BUG #446: process_id() panicked on a poisoned mutex");
        assert_eq!(
            pid, pid_before,
            "process_id must still resolve after poisoning"
        );

        let killed = catch_unwind(AssertUnwindSafe(|| win.kill()))
            .expect("BUG #446: kill() panicked on a poisoned mutex -> pane un-killable");
        assert!(killed.is_ok(), "kill() should report success");

        let status = catch_unwind(AssertUnwindSafe(|| win.wait()))
            .expect("BUG #446: wait() panicked on a poisoned mutex");
        assert!(
            status.is_ok(),
            "wait() should return an exit status after kill"
        );

        // Prove the real OS process is actually gone (not merely that we did
        // not crash): reaping the system handle must succeed.
        let reaped = sys_child.wait().expect("reap the underlying OS process");
        // A TerminateProcess(1) exit is a non-zero code; either way the process
        // has ended, which is the whole point.
        assert!(
            reaped.code().is_some(),
            "the underlying process must have terminated"
        );
    }

    #[test]
    fn poisoned_mutex_try_wait_raw_handle_and_clone_killer_survive() {
        let (mut win, mut sys_child) = spawn_live_winchild();
        poison_proc(&win);

        let tw = catch_unwind(AssertUnwindSafe(|| win.try_wait()))
            .expect("BUG #446: try_wait() panicked on a poisoned mutex");
        assert!(tw.is_ok(), "try_wait should return Ok(...)");

        let raw = catch_unwind(AssertUnwindSafe(|| win.as_raw_handle()))
            .expect("BUG #446: as_raw_handle() panicked on a poisoned mutex");
        assert!(raw.is_some(), "as_raw_handle should return the handle");

        // clone_killer is how the pane hands a killer to another thread; it must
        // survive poisoning too, and the cloned killer must actually kill.
        let mut killer = catch_unwind(AssertUnwindSafe(|| win.clone_killer()))
            .expect("BUG #446: clone_killer() panicked on a poisoned mutex");
        killer.kill().ok();
        let _ = win.wait();
        let reaped = sys_child.wait().expect("reap the underlying OS process");
        assert!(
            reaped.code().is_some(),
            "cloned killer must have terminated the process"
        );
    }
}
