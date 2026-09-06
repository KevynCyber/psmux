//! ZDEP-023: port of portable-pty-psmux 0.9.7 (MIT) `src/win/psuedocon.rs`.
//! `OnceLock` replaces `lazy_static` + `shared_library` (A7); the module
//! table always resolves from "kernel32.dll" -- do NOT sideload a foreign
//! conpty.dll. Terminal emulators like WezTerm bundle their own
//! conpty.dll + OpenConsole.exe, and the DLL search order can pick those up
//! when psmux runs inside such a terminal; a foreign conpty.dll can cause
//! blank panes and broken I/O because the bundled OpenConsole.exe may not be
//! compatible with our ConPTY flags (PASSTHROUGH_MODE, WIN32_INPUT_MODE).

use super::cmdbuilder::CommandBuilder;
use super::child::WinChild;
use super::ffi;
use super::handle::OwnedHandle;
use super::procthreadattr::ProcThreadAttributeList;
use std::io::{self, Error as IoError};
use std::sync::{Mutex, OnceLock};

pub type HPCON = ffi::HANDLE;

pub const PSEUDOCONSOLE_RESIZE_QUIRK: u32 = 0x2;
pub const PSEUDOCONSOLE_WIN32_INPUT_MODE: u32 = 0x4;
pub const PSEUDOCONSOLE_PASSTHROUGH_MODE: u32 = 0x8;
/// Deliberately absent from `conpty_base_flags()` (see its doc comment):
/// only the regression test asserting that absence references it, hence
/// test-gated.
#[cfg(test)]
pub const PSEUDOCONSOLE_INHERIT_CURSOR: u32 = 0x1;

type CreatePseudoConsoleFn =
    unsafe extern "system" fn(ffi::COORD, ffi::HANDLE, ffi::HANDLE, u32, *mut HPCON) -> i32;
type ResizePseudoConsoleFn = unsafe extern "system" fn(HPCON, ffi::COORD) -> i32;
type ClosePseudoConsoleFn = unsafe extern "system" fn(HPCON);

struct ConPtyFuncs {
    create: CreatePseudoConsoleFn,
    resize: ResizePseudoConsoleFn,
    close: ClosePseudoConsoleFn,
}

fn to_wide_nul(s: &str) -> Vec<u16> {
    let mut wide: Vec<u16> = s.encode_utf16().collect();
    wide.push(0);
    wide
}

fn resolve_module(module_name: &str) -> Option<ffi::HANDLE> {
    let wide = to_wide_nul(module_name);
    let h = unsafe { ffi::GetModuleHandleW(wide.as_ptr()) };
    if !h.is_null() {
        return Some(h);
    }
    let h = unsafe { ffi::LoadLibraryW(wide.as_ptr()) };
    if h.is_null() {
        None
    } else {
        Some(h)
    }
}

fn resolve_symbol(module: ffi::HANDLE, name: &[u8]) -> Option<*mut std::ffi::c_void> {
    let p = unsafe { ffi::GetProcAddress(module, name.as_ptr() as *const i8) };
    if p.is_null() {
        None
    } else {
        Some(p)
    }
}

fn unsupported_conpty_err() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "this system does not support ConPTY. Windows 10 October 2018 or newer is required",
    )
}

fn resolve_funcs(module_name: &str) -> io::Result<ConPtyFuncs> {
    let module = resolve_module(module_name).ok_or_else(unsupported_conpty_err)?;
    let create = resolve_symbol(module, b"CreatePseudoConsole\0").ok_or_else(unsupported_conpty_err)?;
    let resize = resolve_symbol(module, b"ResizePseudoConsole\0").ok_or_else(unsupported_conpty_err)?;
    let close = resolve_symbol(module, b"ClosePseudoConsole\0").ok_or_else(unsupported_conpty_err)?;
    Ok(unsafe {
        ConPtyFuncs {
            create: std::mem::transmute::<*mut std::ffi::c_void, CreatePseudoConsoleFn>(create),
            resize: std::mem::transmute::<*mut std::ffi::c_void, ResizePseudoConsoleFn>(resize),
            close: std::mem::transmute::<*mut std::ffi::c_void, ClosePseudoConsoleFn>(close),
        }
    })
}

/// Resolves the three ConPTY entry points from the named module, without
/// caching. Used both by the global table (always "kernel32.dll") and by
/// tests probing arbitrary modules. Never panics: an unresolvable module or
/// missing symbols yield `ErrorKind::Unsupported`.
pub fn probe_conpty(module_name: &str) -> io::Result<()> {
    resolve_funcs(module_name).map(|_| ())
}

static CONPTY: OnceLock<ConPtyFuncs> = OnceLock::new();

fn conpty_funcs() -> &'static ConPtyFuncs {
    CONPTY.get_or_init(|| {
        resolve_funcs("kernel32.dll")
            .expect("this system does not support conpty. Windows 10 October 2018 or newer is required")
    })
}

/// Pure predicate (A7): an explicit "1" or case-insensitive "true" env
/// override always disables passthrough regardless of build number;
/// otherwise the build-number threshold (Windows 11 22H2 = 22621) governs.
pub fn passthrough_supported(build_number: u32, no_passthrough_env: Option<&str>) -> bool {
    if let Some(v) = no_passthrough_env {
        if v == "1" || v.eq_ignore_ascii_case("true") {
            return false;
        }
    }
    build_number >= 22621
}

fn query_build_number() -> u32 {
    let module = match resolve_module("ntdll.dll") {
        Some(m) => m,
        None => return 0,
    };
    let sym = match resolve_symbol(module, b"RtlGetVersion\0") {
        Some(s) => s,
        None => return 0,
    };
    type RtlGetVersionFn = unsafe extern "system" fn(*mut ffi::OSVERSIONINFOW) -> i32;
    let rtl_get_version: RtlGetVersionFn = unsafe { std::mem::transmute(sym) };
    let mut info = ffi::OSVERSIONINFOW {
        dw_os_version_info_size: std::mem::size_of::<ffi::OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    unsafe { rtl_get_version(&mut info) };
    info.dw_build_number
}

/// Respects `PSMUX_NO_PASSTHROUGH=1` (or case-insensitive `true`) to let
/// users force-disable passthrough mode on builds where it causes
/// CreateProcessW to fail with ERROR_INVALID_PARAMETER (87).
fn supports_passthrough_mode() -> bool {
    let env = std::env::var("PSMUX_NO_PASSTHROUGH").ok();
    passthrough_supported(query_build_number(), env.as_deref())
}

/// The flag set passed to every CreatePseudoConsole call (passthrough is
/// OR'ed on separately where supported).
///
/// PSEUDOCONSOLE_INHERIT_CURSOR is deliberately NOT set. With it, conhost
/// emits an ESC[6n cursor-position request at startup and will not service a
/// child's console connection until the host answers it. So if that reply is
/// sent later than the child's connect attempt, the child blocks in
/// ConsoleCreateConnectionObject during process initialization (a single
/// thread, before any user code runs) until the reply arrives: a temporary
/// stall if it is merely late, indefinite if it never comes. A multiplexer
/// pane always starts on a fresh screen, so inheriting the host cursor row
/// buys nothing here.
pub fn conpty_base_flags() -> u32 {
    PSEUDOCONSOLE_RESIZE_QUIRK | PSEUDOCONSOLE_WIN32_INPUT_MODE
}

pub struct PsuedoCon {
    con: HPCON,
    /// Whether this ConPTY was created with PSEUDOCONSOLE_PASSTHROUGH_MODE.
    /// Used by the retry logic in ConPtySlavePty::spawn_command to decide
    /// whether a fallback without passthrough is worth attempting.
    pub used_passthrough: bool,
}

unsafe impl Send for PsuedoCon {}
unsafe impl Sync for PsuedoCon {}

impl Drop for PsuedoCon {
    fn drop(&mut self) {
        unsafe { (conpty_funcs().close)(self.con) };
    }
}

impl PsuedoCon {
    pub fn new(size: ffi::COORD, input: OwnedHandle, output: OwnedHandle) -> io::Result<Self> {
        let mut con: HPCON = ffi::INVALID_HANDLE_VALUE;
        let base_flags = conpty_base_flags();

        if supports_passthrough_mode() {
            let result = unsafe {
                (conpty_funcs().create)(
                    size,
                    input.as_raw(),
                    output.as_raw(),
                    base_flags | PSEUDOCONSOLE_PASSTHROUGH_MODE,
                    &mut con,
                )
            };
            if result == ffi::S_OK {
                return Ok(Self { con, used_passthrough: true });
            }
            con = ffi::INVALID_HANDLE_VALUE;
        }

        let result = unsafe {
            (conpty_funcs().create)(size, input.as_raw(), output.as_raw(), base_flags, &mut con)
        };
        if result != ffi::S_OK {
            return Err(io::Error::other(format!(
                "failed to create psuedo console: HRESULT {}",
                result
            )));
        }
        Ok(Self { con, used_passthrough: false })
    }

    /// Create a ConPTY explicitly without passthrough mode, regardless of
    /// Windows build version. Used by the retry logic when CreateProcessW
    /// rejects the passthrough ConPTY handle.
    pub fn new_without_passthrough(size: ffi::COORD, input: OwnedHandle, output: OwnedHandle) -> io::Result<Self> {
        let mut con: HPCON = ffi::INVALID_HANDLE_VALUE;
        let base_flags = conpty_base_flags();

        let result = unsafe {
            (conpty_funcs().create)(size, input.as_raw(), output.as_raw(), base_flags, &mut con)
        };
        if result != ffi::S_OK {
            return Err(io::Error::other(format!(
                "failed to create psuedo console (no passthrough): HRESULT {}",
                result
            )));
        }
        Ok(Self { con, used_passthrough: false })
    }

    pub fn resize(&self, size: ffi::COORD) -> io::Result<()> {
        let result = unsafe { (conpty_funcs().resize)(self.con, size) };
        if result != ffi::S_OK {
            return Err(io::Error::other(format!(
                "failed to resize console to {}x{}: HRESULT: {}",
                size.x, size.y, result
            )));
        }
        Ok(())
    }

    pub fn spawn_command(&self, cmd: CommandBuilder) -> io::Result<WinChild> {
        let mut si = ffi::STARTUPINFOEXW::default();
        si.startup_info.cb = std::mem::size_of::<ffi::STARTUPINFOEXW>() as u32;
        // Note: we deliberately do NOT set STARTF_USESTDHANDLES with
        // INVALID_HANDLE_VALUE for stdio. MSDN explicitly requires
        // STARTF_USESTDHANDLES to be paired with bInheritHandles=TRUE, and we
        // use bInheritHandles=FALSE below. Most Windows builds tolerate the
        // combination silently (INVALID_HANDLE_VALUE is a sentinel rather
        // than a real handle), but newer/restricted configurations -- Win 11
        // 26200, Microsoft-account profiles with tighter token policies,
        // certain WDAC/AppLocker rule sets -- now enforce the contract
        // strictly and reject the call with ERROR_INVALID_PARAMETER (87).
        // See psmux issue #167.
        //
        // ConPTY routes stdio through the PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE
        // attribute on the attribute list, so the child gets correct stdio
        // regardless of dwFlags. bInheritHandles=FALSE prevents leaking any
        // other inheritable handles.

        let mut attrs = ProcThreadAttributeList::with_capacity(1)?;
        attrs.set_pty(self.con)?;
        si.lp_attribute_list = attrs.as_mut_ptr();

        let mut pi = ffi::PROCESS_INFORMATION::default();

        let (mut exe, mut cmdline) = cmd.cmdline()?;

        let cwd = cmd.current_directory();

        // The child's ProcessParameters std handles are stamped from this
        // process's std handle slots at CreateProcessW time. Hold the
        // console state lock so no FreeConsole/AttachConsole dance (Ctrl+C
        // delivery, mouse/VT injection) is mid-flight on another thread, and
        // park the slots on NULL for the duration of the call: after any
        // dance the slots of a headless server dangle on freed, recycled
        // handle values, and a child born from them dies at its first
        // console read (issue #450). A NULL-std parent is the GUI-parent
        // case, for which conhost always hands the child fresh handles to
        // its own console.
        let _console_guard = super::console_state_lock();
        let saved_std = unsafe {
            let saved = (
                ffi::GetStdHandle(ffi::STD_INPUT_HANDLE),
                ffi::GetStdHandle(ffi::STD_OUTPUT_HANDLE),
                ffi::GetStdHandle(ffi::STD_ERROR_HANDLE),
            );
            ffi::SetStdHandle(ffi::STD_INPUT_HANDLE, std::ptr::null_mut());
            ffi::SetStdHandle(ffi::STD_OUTPUT_HANDLE, std::ptr::null_mut());
            ffi::SetStdHandle(ffi::STD_ERROR_HANDLE, std::ptr::null_mut());
            saved
        };

        let env_block = cmd.environment_block();
        let res = unsafe {
            ffi::CreateProcessW(
                exe.as_mut_slice().as_mut_ptr(),
                cmdline.as_mut_slice().as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                ffi::EXTENDED_STARTUPINFO_PRESENT | ffi::CREATE_UNICODE_ENVIRONMENT,
                env_block.as_slice().as_ptr() as *const _,
                cwd.as_ref()
                    .map(|c| c.as_slice().as_ptr())
                    .unwrap_or(std::ptr::null()),
                &si.startup_info,
                &mut pi,
            )
        };
        // Capture the OS error before it can be clobbered by SetStdHandle
        // below; propagate it as-is (not stringified) so the caller can
        // check `raw_os_error() == Some(87)` for the passthrough-fallback
        // retry (see ConPtySlavePty::spawn_command).
        let create_err = IoError::last_os_error();
        unsafe {
            ffi::SetStdHandle(ffi::STD_INPUT_HANDLE, saved_std.0);
            ffi::SetStdHandle(ffi::STD_OUTPUT_HANDLE, saved_std.1);
            ffi::SetStdHandle(ffi::STD_ERROR_HANDLE, saved_std.2);
        }
        if res == 0 {
            return Err(create_err);
        }

        // Make sure we close out the thread handle so we don't leak it; we
        // do this simply by making it owned.
        let _main_thread = unsafe { OwnedHandle::from_raw(pi.h_thread) };
        let proc = unsafe { OwnedHandle::from_raw(pi.h_process) };

        Ok(WinChild {
            proc: Mutex::new(proc),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conpty_flags_do_not_include_inherit_cursor() {
        // PSEUDOCONSOLE_INHERIT_CURSOR makes the new conhost emit ESC[6n at
        // startup and block its own initialization until the host replies;
        // an unanswered query leaves conhost unable to service the child's
        // console connect, so the child hangs inside process initialization
        // (single thread parked in ConsoleCreateConnectionObject). A
        // multiplexer pane always starts on a fresh screen, so inheriting
        // the host cursor row has no value here.
        assert_eq!(
            conpty_base_flags() & PSEUDOCONSOLE_INHERIT_CURSOR,
            0,
            "INHERIT_CURSOR must not be set: it makes conhost block startup waiting for an ESC[6n reply"
        );
        // The other flags are load-bearing and must stay.
        assert_ne!(conpty_base_flags() & PSEUDOCONSOLE_RESIZE_QUIRK, 0);
        assert_ne!(conpty_base_flags() & PSEUDOCONSOLE_WIN32_INPUT_MODE, 0);
    }
}
