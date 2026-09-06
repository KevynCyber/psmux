//! INPUT_RECORD -> Event decoding: the stateless VK-code helpers
//! (`vk_to_keycode` / `vk_modifiers`, moved from the ssh_input module rather
//! than duplicated) and the stateful `InputDecoder` (surrogate-pair
//! buffering, previous-mouse-button tracking).

use super::{
    FOCUS_EVENT, FOCUS_EVENT_RECORD, INPUT_RECORD, KEY_EVENT, KEY_EVENT_RECORD, MOUSE_EVENT,
    MOUSE_EVENT_RECORD, WINDOW_BUFFER_SIZE_EVENT, WINDOW_BUFFER_SIZE_RECORD,
};
use super::super::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

#[link(name = "user32")]
extern "system" {
    /// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getforegroundwindow
    fn GetForegroundWindow() -> isize;
    /// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid
    fn GetWindowThreadProcessId(hWnd: isize, lpdwProcessId: *mut u32) -> u32;
    /// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getkeyboardlayout
    fn GetKeyboardLayout(idThread: u32) -> isize;
    /// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-tounicodeex
    fn ToUnicodeEx(
        wVirtKey: u32,
        wScanCode: u32,
        lpKeyState: *const u8,
        pwszBuff: *mut u16,
        cchBuff: i32,
        wFlags: u32,
        dwhkl: isize,
    ) -> i32;
}

const SHIFT_PRESSED: u32 = 0x0010;
const LEFT_ALT_PRESSED: u32 = 0x0002;
const RIGHT_ALT_PRESSED: u32 = 0x0001;
const LEFT_CTRL_PRESSED: u32 = 0x0004;
const RIGHT_CTRL_PRESSED: u32 = 0x0008;
const CAPSLOCK_ON: u32 = 0x0080;
const NUMLOCK_ON: u32 = 0x0020;

const FROM_LEFT_1ST_BUTTON_PRESSED: u32 = 0x0001;
const RIGHTMOST_BUTTON_PRESSED: u32 = 0x0002;
const FROM_LEFT_2ND_BUTTON_PRESSED: u32 = 0x0004;
const MOUSE_MOVED: u32 = 0x0001;
const MOUSE_WHEELED: u32 = 0x0004;
const MOUSE_HWHEELED: u32 = 0x0008;

/// Map a Windows virtual-key code to an `Event` `KeyCode`. Returns `None`
/// for modifier-only keys (Ctrl, Shift, Alt) and keys not in this table.
pub fn vk_to_keycode(vk: u16) -> Option<KeyCode> {
    match vk {
        0x08 => Some(KeyCode::Backspace), // VK_BACK
        0x09 => Some(KeyCode::Tab),       // VK_TAB
        0x0D => Some(KeyCode::Enter),     // VK_RETURN
        0x1B => Some(KeyCode::Esc),       // VK_ESCAPE
        0x20 => Some(KeyCode::Char(' ')), // VK_SPACE
        0x21 => Some(KeyCode::PageUp),    // VK_PRIOR
        0x22 => Some(KeyCode::PageDown),  // VK_NEXT
        0x23 => Some(KeyCode::End),       // VK_END
        0x24 => Some(KeyCode::Home),      // VK_HOME
        0x25 => Some(KeyCode::Left),      // VK_LEFT
        0x26 => Some(KeyCode::Up),        // VK_UP
        0x27 => Some(KeyCode::Right),     // VK_RIGHT
        0x28 => Some(KeyCode::Down),      // VK_DOWN
        0x2D => Some(KeyCode::Insert),    // VK_INSERT
        0x2E => Some(KeyCode::Delete),    // VK_DELETE
        0x70..=0x87 => Some(KeyCode::F((vk - 111) as u8)), // VK_F1..VK_F24
        _ => None,
    }
}

/// Extract `KeyModifiers` from Win32 `dwControlKeyState`.
pub fn vk_modifiers(state: u32) -> KeyModifiers {
    let mut m = KeyModifiers::empty();
    if state & SHIFT_PRESSED != 0 { m |= KeyModifiers::SHIFT; }
    if state & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED) != 0 { m |= KeyModifiers::ALT; }
    if state & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED) != 0 { m |= KeyModifiers::CONTROL; }
    m
}

fn key_event_state(state: u32) -> KeyEventState {
    let mut s = KeyEventState::empty();
    if state & CAPSLOCK_ON != 0 { s.insert(KeyEventState::CAPS_LOCK); }
    if state & NUMLOCK_ON != 0 { s.insert(KeyEventState::NUM_LOCK); }
    s
}

/// Best-effort char lookup for control-range `u_char` values (0x00..=0x1f),
/// accounting for the user's keyboard layout -- ports crossterm's
/// `get_char_for_key` (crossterm-0.29.0 event/sys/windows/parse.rs:143-202)
/// verbatim: `ToUnicodeEx` with an all-zero key-state array, then Shift/
/// CapsLock case correction applied manually afterward.
fn get_char_for_key(vk: u16, scan: u16, control_key_state: u32) -> Option<char> {
    let key_state = [0u8; 256];
    let mut utf16_buf = [0u16; 16];
    const DONT_CHANGE_KERNEL_KEYBOARD_STATE: u32 = 0x4;

    let layout = unsafe {
        let hwnd = GetForegroundWindow();
        let tid = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
        GetKeyboardLayout(tid)
    };

    let ret = unsafe {
        ToUnicodeEx(
            vk as u32,
            scan as u32,
            key_state.as_ptr(),
            utf16_buf.as_mut_ptr(),
            utf16_buf.len() as i32,
            DONT_CHANGE_KERNEL_KEYBOARD_STATE,
            layout,
        )
    };
    if ret < 1 { return None; }

    let mut it = char::decode_utf16(utf16_buf.into_iter().take(ret as usize));
    let mut ch = it.next()?.ok()?;
    if it.next().is_some() { return None; } // doesn't map to a single char

    let shift = control_key_state & SHIFT_PRESSED != 0;
    let caps = control_key_state & CAPSLOCK_ON != 0;
    if shift ^ caps {
        if ch.is_lowercase() {
            let mut up = ch.to_uppercase();
            if let (Some(u), None) = (up.next(), up.next()) { ch = u; }
        }
    } else if ch.is_uppercase() {
        let mut lo = ch.to_lowercase();
        if let (Some(l), None) = (lo.next(), lo.next()) { ch = l; }
    }
    Some(ch)
}

/// Stateless MOUSE_EVENT_RECORD -> Event conversion for the SSH/pipe input
/// path (ssh_input's own INPUT_RECORD reading loop), moved here verbatim.
/// Unlike `InputDecoder::decode`, this has no previous-button-state
/// tracking (matches the pre-fold behavior it replaces).
pub fn convert_native_mouse(rec: &MOUSE_EVENT_RECORD) -> Option<Event> {
    const FROM_LEFT_1ST: u32 = 0x0001;
    const RIGHTMOST: u32 = 0x0002;
    const FROM_LEFT_2ND: u32 = 0x0004;
    const ME_MOVED: u32 = 0x0001;
    const ME_WHEELED: u32 = 0x0004;

    let col = rec.mouse_x.max(0) as u16;
    let row = rec.mouse_y.max(0) as u16;
    let mods = vk_modifiers(rec.control_key_state);

    if rec.event_flags & ME_WHEELED != 0 {
        let delta = (rec.button_state >> 16) as i16;
        let kind = if delta > 0 { MouseEventKind::ScrollUp } else { MouseEventKind::ScrollDown };
        return Some(Event::Mouse(MouseEvent { kind, column: col, row, modifiers: mods }));
    }

    if rec.event_flags & ME_MOVED != 0 {
        if rec.button_state & FROM_LEFT_1ST != 0 {
            return Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Drag(MouseButton::Left), column: col, row, modifiers: mods }));
        }
        if rec.button_state & RIGHTMOST != 0 {
            return Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Drag(MouseButton::Right), column: col, row, modifiers: mods }));
        }
        return Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Moved, column: col, row, modifiers: mods }));
    }

    if rec.button_state & FROM_LEFT_1ST != 0 {
        return Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: col, row, modifiers: mods }));
    }
    if rec.button_state & RIGHTMOST != 0 {
        return Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Right), column: col, row, modifiers: mods }));
    }
    if rec.button_state & FROM_LEFT_2ND != 0 {
        return Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Middle), column: col, row, modifiers: mods }));
    }

    // button_state == 0 -> all buttons released
    if rec.button_state == 0 && rec.event_flags == 0 {
        return Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Up(MouseButton::Left), column: col, row, modifiers: mods }));
    }

    None
}

#[derive(Default)]
struct MouseButtonsPressed {
    left: bool,
    right: bool,
    middle: bool,
}

/// Decodes raw `INPUT_RECORD`s into `Event`s. Stateful because Up-vs-Down
/// mouse detection needs the previous button state, and a surrogate-pair
/// character key arrives as two separate KEY_EVENT records.
pub struct InputDecoder {
    surrogate: Option<u16>,
    mouse: MouseButtonsPressed,
}

impl InputDecoder {
    pub fn new() -> Self {
        Self { surrogate: None, mouse: MouseButtonsPressed::default() }
    }

    pub fn decode(&mut self, rec: &INPUT_RECORD, window_top: i16) -> Option<Event> {
        match rec.event_type {
            KEY_EVENT => {
                let r = unsafe { &*(rec.data.as_ptr() as *const KEY_EVENT_RECORD) };
                self.decode_key(r)
            }
            MOUSE_EVENT => {
                let r = unsafe { &*(rec.data.as_ptr() as *const MOUSE_EVENT_RECORD) };
                Some(self.decode_mouse(r, window_top))
            }
            WINDOW_BUFFER_SIZE_EVENT => {
                let r = unsafe { &*(rec.data.as_ptr() as *const WINDOW_BUFFER_SIZE_RECORD) };
                Some(Event::Resize((r.size_x + 1) as u16, (r.size_y + 1) as u16))
            }
            FOCUS_EVENT => {
                let r = unsafe { &*(rec.data.as_ptr() as *const FOCUS_EVENT_RECORD) };
                Some(if r.set_focus != 0 { Event::FocusGained } else { Event::FocusLost })
            }
            _ => None,
        }
    }

    fn handle_surrogate(&mut self, new_surrogate: u16) -> Option<char> {
        match self.surrogate.take() {
            Some(buffered) => char::decode_utf16([buffered, new_surrogate]).next().and_then(|r| r.ok()),
            None => {
                self.surrogate = Some(new_surrogate);
                None
            }
        }
    }

    fn decode_key(&mut self, rec: &KEY_EVENT_RECORD) -> Option<Event> {
        let vk = rec.virtual_key_code;
        let modifiers = vk_modifiers(rec.control_key_state);

        if matches!(vk, 0x10 | 0x11 | 0x12) {
            return None; // VK_SHIFT | VK_CONTROL | VK_MENU
        }

        let code = if let Some(c) = vk_to_keycode(vk) {
            Some(c)
        } else {
            match rec.u_char {
                surrogate @ 0xD800..=0xDFFF => {
                    return self
                        .handle_surrogate(surrogate)
                        .map(|ch| Event::Key(KeyEvent::new(KeyCode::Char(ch), modifiers)));
                }
                0x00..=0x1f => get_char_for_key(vk, rec.virtual_scan_code, rec.control_key_state).map(KeyCode::Char),
                unicode => char::from_u32(unicode as u32).map(KeyCode::Char),
            }
        };

        let mut code = code?;
        if code == KeyCode::Tab && modifiers.contains(KeyModifiers::SHIFT) {
            code = KeyCode::BackTab;
        }
        self.surrogate = None;

        let kind = if rec.key_down != 0 { KeyEventKind::Press } else { KeyEventKind::Release };
        let state = key_event_state(rec.control_key_state);
        Some(Event::Key(KeyEvent { code, modifiers, kind, state }))
    }

    fn decode_mouse(&mut self, rec: &MOUSE_EVENT_RECORD, window_top: i16) -> Event {
        let modifiers = vk_modifiers(rec.control_key_state);
        let column = rec.mouse_x.max(0) as u16;
        let row = (rec.mouse_y as i32 - window_top as i32).max(0) as u16;

        let left_now = rec.button_state & FROM_LEFT_1ST_BUTTON_PRESSED != 0;
        let right_now = rec.button_state & RIGHTMOST_BUTTON_PRESSED != 0;
        let middle_now = rec.button_state & FROM_LEFT_2ND_BUTTON_PRESSED != 0;

        let kind = if rec.event_flags & MOUSE_MOVED != 0 {
            if !left_now && !right_now && !middle_now {
                MouseEventKind::Moved
            } else {
                let button = if left_now { MouseButton::Left } else if right_now { MouseButton::Right } else { MouseButton::Middle };
                MouseEventKind::Drag(button)
            }
        } else if rec.event_flags & MOUSE_WHEELED != 0 {
            let delta = (rec.button_state >> 16) as i16;
            if delta > 0 { MouseEventKind::ScrollUp } else { MouseEventKind::ScrollDown }
        } else if rec.event_flags & MOUSE_HWHEELED != 0 {
            let delta = (rec.button_state >> 16) as i16;
            if delta > 0 { MouseEventKind::ScrollRight } else { MouseEventKind::ScrollLeft }
        } else if left_now && !self.mouse.left {
            MouseEventKind::Down(MouseButton::Left)
        } else if !left_now && self.mouse.left {
            MouseEventKind::Up(MouseButton::Left)
        } else if right_now && !self.mouse.right {
            MouseEventKind::Down(MouseButton::Right)
        } else if !right_now && self.mouse.right {
            MouseEventKind::Up(MouseButton::Right)
        } else if middle_now && !self.mouse.middle {
            MouseEventKind::Down(MouseButton::Middle)
        } else if !middle_now && self.mouse.middle {
            MouseEventKind::Up(MouseButton::Middle)
        } else {
            MouseEventKind::Moved
        };

        self.mouse.left = left_now;
        self.mouse.right = right_now;
        self.mouse.middle = middle_now;

        Event::Mouse(MouseEvent { kind, column, row, modifiers })
    }
}
