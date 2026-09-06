//! Native Win32 console I/O (ZDEP-026): raw mode, size, poll/read, and the
//! Win32 record types. Replaces crossterm's Windows backend
//! (crossterm_winapi + winapi) with direct FFI. The INPUT_RECORD -> Event
//! decoder lives in `decode` (kept in a separate file for the 500-line
//! file-structure gate) and is re-exported here.

mod decode;
pub use decode::{convert_native_mouse, vk_modifiers, vk_to_keycode, InputDecoder};

use super::event::Event;
use std::ffi::c_void;
use std::io;
use std::sync::OnceLock;
use std::time::Duration;

// ─── Win32 constants ──────────────────────────────────────────────────────

const STD_INPUT_HANDLE: u32 = (-10i32) as u32;
const STD_OUTPUT_HANDLE: u32 = (-11i32) as u32;

const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
const ENABLE_LINE_INPUT: u32 = 0x0002;
const ENABLE_ECHO_INPUT: u32 = 0x0004;
const ENABLE_WINDOW_INPUT: u32 = 0x0008;
const ENABLE_MOUSE_INPUT: u32 = 0x0010;
const ENABLE_QUICK_EDIT_MODE: u32 = 0x0040;
const ENABLE_EXTENDED_FLAGS: u32 = 0x0080;

/// Bits that must be clear for raw mode (crossterm's `NOT_RAW_MODE_MASK`).
const NOT_RAW_MODE_MASK: u32 = ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT;

const WAIT_OBJECT_0: u32 = 0x0000_0000;
const WAIT_TIMEOUT: u32 = 0x0000_0102;

pub const KEY_EVENT: u16 = 0x0001;
pub const MOUSE_EVENT: u16 = 0x0002;
pub const WINDOW_BUFFER_SIZE_EVENT: u16 = 0x0004;
pub const FOCUS_EVENT: u16 = 0x0010;

// ─── Win32 structs ────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone)]
pub struct KEY_EVENT_RECORD {
    pub key_down: i32,
    pub repeat_count: u16,
    pub virtual_key_code: u16,
    pub virtual_scan_code: u16,
    pub u_char: u16,
    pub control_key_state: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MOUSE_EVENT_RECORD {
    pub mouse_x: i16,
    pub mouse_y: i16,
    pub button_state: u32,
    pub control_key_state: u32,
    pub event_flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct WINDOW_BUFFER_SIZE_RECORD {
    pub size_x: i16,
    pub size_y: i16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FOCUS_EVENT_RECORD {
    pub set_focus: i32,
}

/// The 16-byte union view Win32's `ReadConsoleInputW` fills; `data` holds
/// whichever of KEY_EVENT_RECORD / MOUSE_EVENT_RECORD / ... is valid for
/// `event_type` (the largest variant is 16 bytes, so that's the buffer size).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct INPUT_RECORD {
    // pub(crate): the ssh_input module's own INPUT_RECORD reading loop
    // (SSH/pipe mode; a separate mechanism from this module's InputDecoder)
    // matches on event_type and casts data directly, same as the pre-fold
    // local struct it used to define.
    pub(crate) event_type: u16,
    _pad: u16,
    pub(crate) data: [u8; 16],
}

impl INPUT_RECORD {
    pub fn new(event_type: u16, data: [u8; 16]) -> Self {
        Self { event_type, _pad: 0, data }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct Coord {
    x: i16,
    y: i16,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct SmallRect {
    left: i16,
    top: i16,
    right: i16,
    bottom: i16,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct ConsoleScreenBufferInfo {
    size: Coord,
    cursor_position: Coord,
    attributes: u16,
    window: SmallRect,
    maximum_window_size: Coord,
}

// ─── Win32 imports ────────────────────────────────────────────────────────
//
// GetStdHandle/GetConsoleMode/SetConsoleMode/GetNumberOfConsoleInputEvents/
// ReadConsoleInputW/WaitForSingleObject are also declared in src/platform.rs
// and (until this fold) src/ssh_input.rs; every declaration in the crate
// uses the same parameter types (*mut c_void handles, u32 modes) to avoid
// clashing_extern_declarations. ReadConsoleInputW's buffer stays `*mut
// c_void` for the same reason: each module keeps its own view of the
// INPUT_RECORD union.
//
// Win32 signatures: https://learn.microsoft.com/en-us/windows/console/

#[link(name = "kernel32")]
extern "system" {
    /// https://learn.microsoft.com/en-us/windows/console/getstdhandle
    fn GetStdHandle(nStdHandle: u32) -> *mut c_void;
    /// https://learn.microsoft.com/en-us/windows/console/getconsolemode
    fn GetConsoleMode(hConsoleHandle: *mut c_void, lpMode: *mut u32) -> i32;
    /// https://learn.microsoft.com/en-us/windows/console/setconsolemode
    fn SetConsoleMode(hConsoleHandle: *mut c_void, dwMode: u32) -> i32;
    /// https://learn.microsoft.com/en-us/windows/console/getnumberofconsoleinputevents
    fn GetNumberOfConsoleInputEvents(hConsoleInput: *mut c_void, lpcNumberOfEvents: *mut u32) -> i32;
    /// https://learn.microsoft.com/en-us/windows/console/readconsoleinput
    fn ReadConsoleInputW(hConsoleInput: *mut c_void, lpBuffer: *mut c_void, nLength: u32, lpNumberOfEventsRead: *mut u32) -> i32;
    /// https://learn.microsoft.com/en-us/windows/console/getconsolescreenbufferinfo
    fn GetConsoleScreenBufferInfo(hConsoleOutput: *mut c_void, lpConsoleScreenBufferInfo: *mut ConsoleScreenBufferInfo) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    /// https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject
    fn WaitForSingleObject(hHandle: *mut c_void, dwMilliseconds: u32) -> u32;
}

fn check_handle(h: *mut c_void) -> io::Result<*mut c_void> {
    if h.is_null() || h == (-1isize) as *mut c_void {
        Err(io::Error::new(io::ErrorKind::Other, "invalid console handle (not attached to a console?)"))
    } else {
        Ok(h)
    }
}

fn last_err() -> io::Error {
    io::Error::last_os_error()
}

// ─── raw mode / size / poll / read ────────────────────────────────────────

pub fn enable_raw_mode() -> io::Result<()> {
    unsafe {
        let h = check_handle(GetStdHandle(STD_INPUT_HANDLE))?;
        let mut mode: u32 = 0;
        if GetConsoleMode(h, &mut mode) == 0 { return Err(last_err()); }
        if SetConsoleMode(h, mode & !NOT_RAW_MODE_MASK) == 0 { return Err(last_err()); }
    }
    Ok(())
}

/// crossterm ORs the not-raw-mode bits back onto the CURRENT mode rather
/// than restoring a saved value -- replicated here (terminal/sys/windows.rs
/// `disable_raw_mode`).
pub fn disable_raw_mode() -> io::Result<()> {
    unsafe {
        let h = check_handle(GetStdHandle(STD_INPUT_HANDLE))?;
        let mut mode: u32 = 0;
        if GetConsoleMode(h, &mut mode) == 0 { return Err(last_err()); }
        if SetConsoleMode(h, mode | NOT_RAW_MODE_MASK) == 0 { return Err(last_err()); }
    }
    Ok(())
}

pub fn size() -> io::Result<(u16, u16)> {
    unsafe {
        let h = check_handle(GetStdHandle(STD_OUTPUT_HANDLE))?;
        let mut info = ConsoleScreenBufferInfo::default();
        if GetConsoleScreenBufferInfo(h, &mut info) == 0 { return Err(last_err()); }
        let w = (info.window.right - info.window.left + 1) as u16;
        let h2 = (info.window.bottom - info.window.top + 1) as u16;
        Ok((w, h2))
    }
}

/// Cursor position relative to the visible window (subtracts `srWindow.Top`,
/// matching crossterm's `cursor::position()`). Returns an error when stdout
/// is not attached to a console (e.g. piped/SSH mode).
pub fn cursor_position() -> io::Result<(u16, u16)> {
    unsafe {
        let h = check_handle(GetStdHandle(STD_OUTPUT_HANDLE))?;
        let mut info = ConsoleScreenBufferInfo::default();
        if GetConsoleScreenBufferInfo(h, &mut info) == 0 { return Err(last_err()); }
        let col = info.cursor_position.x.max(0) as u16;
        let row = (info.cursor_position.y as i32 - info.window.top as i32).max(0) as u16;
        Ok((col, row))
    }
}

fn window_top() -> i16 {
    unsafe {
        let Ok(h) = check_handle(GetStdHandle(STD_OUTPUT_HANDLE)) else { return 0 };
        let mut info = ConsoleScreenBufferInfo::default();
        if GetConsoleScreenBufferInfo(h, &mut info) == 0 { return 0; }
        info.window.top
    }
}

pub fn poll(timeout: Duration) -> io::Result<bool> {
    unsafe {
        let h = check_handle(GetStdHandle(STD_INPUT_HANDLE))?;
        let ms = timeout.as_millis().min(u128::from(u32::MAX - 1)) as u32;
        match WaitForSingleObject(h, ms) {
            WAIT_OBJECT_0 => {
                let mut n: u32 = 0;
                if GetNumberOfConsoleInputEvents(h, &mut n) == 0 { return Err(last_err()); }
                Ok(n > 0)
            }
            WAIT_TIMEOUT => Ok(false),
            _ => Err(last_err()),
        }
    }
}

thread_local! {
    static DECODER: std::cell::RefCell<InputDecoder> = std::cell::RefCell::new(InputDecoder::new());
}

pub fn read() -> io::Result<Event> {
    unsafe {
        let h = check_handle(GetStdHandle(STD_INPUT_HANDLE))?;
        loop {
            let mut rec = INPUT_RECORD::new(0, [0u8; 16]);
            let mut count: u32 = 0;
            if ReadConsoleInputW(h, &mut rec as *mut INPUT_RECORD as *mut c_void, 1, &mut count) == 0 {
                return Err(last_err());
            }
            if count == 0 { continue; }
            let top = window_top();
            let evt = DECODER.with(|d| d.borrow_mut().decode(&rec, top));
            if let Some(evt) = evt { return Ok(evt); }
        }
    }
}

// ─── mouse capture native console-mode toggle ────────────────────────────

static ORIGINAL_MOUSE_MODE: OnceLock<u32> = OnceLock::new();

/// crossterm's Windows `enable_mouse_capture` replaces the whole console
/// mode with `ENABLE_MOUSE_INPUT | ENABLE_EXTENDED_FLAGS | ENABLE_WINDOW_INPUT`
/// (its `ENABLE_MOUSE_MODE = 0x10 | 0x80 | 0x08`). psmux's own
/// `send_mouse_keepalive` already writes DECSET explicitly and
/// `ssh_input.rs`'s SetConsoleMode has the final word in SSH mode, so this
/// ORs those bits onto the current mode instead of replacing it outright,
/// while still clearing ENABLE_QUICK_EDIT_MODE (0x40) so a click doesn't
/// fall into console text selection.
pub fn enable_mouse_capture_native() -> io::Result<()> {
    unsafe {
        let h = check_handle(GetStdHandle(STD_INPUT_HANDLE))?;
        let mut mode: u32 = 0;
        if GetConsoleMode(h, &mut mode) == 0 { return Err(last_err()); }
        let _ = ORIGINAL_MOUSE_MODE.set(mode);
        let new_mode = (mode & !ENABLE_QUICK_EDIT_MODE) | ENABLE_MOUSE_INPUT | ENABLE_EXTENDED_FLAGS | ENABLE_WINDOW_INPUT;
        if SetConsoleMode(h, new_mode) == 0 { return Err(last_err()); }
    }
    Ok(())
}

pub fn disable_mouse_capture_native() -> io::Result<()> {
    if let Some(&orig) = ORIGINAL_MOUSE_MODE.get() {
        unsafe {
            let h = check_handle(GetStdHandle(STD_INPUT_HANDLE))?;
            if SetConsoleMode(h, orig) == 0 { return Err(last_err()); }
        }
    }
    Ok(())
}
