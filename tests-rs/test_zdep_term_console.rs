#![cfg(windows)]
// Covers: ZDEP-026
// Requirement: native Windows console input (INPUT_RECORD -> crate::term
// Event) and raw-mode/size/poll/read replace crossterm's Win32 backend.
// vk_to_keycode/vk_modifiers/convert_native_mouse move from
// crate::ssh_input into crate::term::console and are re-exported there
// (not duplicated). Because Up-vs-Down mouse detection needs the previous
// button state and surrogate-pair keys need cross-record buffering, decode
// is exposed as a small stateful `crate::term::console::InputDecoder` with
// `fn decode(&mut self, rec: &INPUT_RECORD, window_top: i16) -> Option<Event>`.
// This test file IS that API-shape proposal (the design note leaves the
// shape open); see the RED report for the exact names.

use crate::term::console::{
    InputDecoder, FOCUS_EVENT, INPUT_RECORD, KEY_EVENT, MOUSE_EVENT, WINDOW_BUFFER_SIZE_EVENT,
};
use crate::term::console::{FOCUS_EVENT_RECORD, KEY_EVENT_RECORD, MOUSE_EVENT_RECORD, WINDOW_BUFFER_SIZE_RECORD};
use crate::term::event::{
    Event, KeyCode, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};

// ── Win32 constant mirrors (kept local to the test so this file documents
// the exact bit values it exercises independent of the implementation). ──
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
const DOUBLE_CLICK: u32 = 0x0002;
const MOUSE_WHEELED: u32 = 0x0004;
const MOUSE_HWHEELED: u32 = 0x0008;

/// Pack any `#[repr(C)]` union-member record into an INPUT_RECORD's raw
/// `data` bytes, mirroring how `ReadConsoleInputW` fills the real union.
/// `INPUT_RECORD::new(event_type, data)` is the seam this test asks
/// `crate::term::console` to expose (kept minimal: two public fields plus a
/// constructor, so this file -- not production code -- owns the per-variant
/// packing helpers below).
fn pack<T: Copy>(event_type: u16, payload: T) -> INPUT_RECORD {
    assert!(std::mem::size_of::<T>() <= 16, "record payload must fit the 16-byte union");
    let mut data = [0u8; 16];
    unsafe {
        std::ptr::copy_nonoverlapping(
            &payload as *const T as *const u8,
            data.as_mut_ptr(),
            std::mem::size_of::<T>(),
        );
    }
    INPUT_RECORD::new(event_type, data)
}

fn key_record(down: bool, vk: u16, uchar: u16, ctrl_state: u32) -> INPUT_RECORD {
    pack(
        KEY_EVENT,
        KEY_EVENT_RECORD {
            key_down: down as i32,
            repeat_count: 1,
            virtual_key_code: vk,
            virtual_scan_code: 0,
            u_char: uchar,
            control_key_state: ctrl_state,
        },
    )
}

fn mouse_record(x: i16, y: i16, button_state: u32, ctrl_state: u32, flags: u32) -> INPUT_RECORD {
    pack(
        MOUSE_EVENT,
        MOUSE_EVENT_RECORD {
            mouse_x: x,
            mouse_y: y,
            button_state,
            control_key_state: ctrl_state,
            event_flags: flags,
        },
    )
}

fn resize_record(cols: i16, rows: i16) -> INPUT_RECORD {
    pack(WINDOW_BUFFER_SIZE_EVENT, WINDOW_BUFFER_SIZE_RECORD { size_x: cols, size_y: rows })
}

fn focus_record(set_focus: bool) -> INPUT_RECORD {
    pack(FOCUS_EVENT, FOCUS_EVENT_RECORD { set_focus: set_focus as i32 })
}

fn press(vk: u16, uchar: u16, ctrl_state: u32) -> Option<Event> {
    let mut d = InputDecoder::new();
    d.decode(&key_record(true, vk, uchar, ctrl_state), 0)
}

fn key_ev(code: KeyCode, modifiers: KeyModifiers) -> Event {
    Event::Key(crate::term::event::KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// event_type constants match the real Win32 INPUT_RECORD ABI.
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn input_record_event_type_constants() {
    assert_eq!(KEY_EVENT, 0x0001);
    assert_eq!(MOUSE_EVENT, 0x0002);
    assert_eq!(WINDOW_BUFFER_SIZE_EVENT, 0x0004);
    assert_eq!(FOCUS_EVENT, 0x0010);
}

// ═══════════════════════════════════════════════════════════════════════
// INPUT_RECORD -> Event table (>= 30 rows).
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn input_record_to_event_table() {
    let cases: Vec<(&str, INPUT_RECORD, Option<Event>)> = vec![
        ("VK_BACK down", key_record(true, 0x08, 0, 0), Some(key_ev(KeyCode::Backspace, KeyModifiers::empty()))),
        ("VK_TAB down", key_record(true, 0x09, 0, 0), Some(key_ev(KeyCode::Tab, KeyModifiers::empty()))),
        ("VK_TAB + SHIFT -> BackTab", key_record(true, 0x09, 0, SHIFT_PRESSED), Some(key_ev(KeyCode::BackTab, KeyModifiers::SHIFT))),
        ("VK_RETURN down", key_record(true, 0x0D, 0, 0), Some(key_ev(KeyCode::Enter, KeyModifiers::empty()))),
        ("VK_ESCAPE down", key_record(true, 0x1B, 0, 0), Some(key_ev(KeyCode::Esc, KeyModifiers::empty()))),
        ("VK_SPACE down", key_record(true, 0x20, 0, 0), Some(key_ev(KeyCode::Char(' '), KeyModifiers::empty()))),
        ("VK_PRIOR (PageUp)", key_record(true, 0x21, 0, 0), Some(key_ev(KeyCode::PageUp, KeyModifiers::empty()))),
        ("VK_NEXT (PageDown)", key_record(true, 0x22, 0, 0), Some(key_ev(KeyCode::PageDown, KeyModifiers::empty()))),
        ("VK_END", key_record(true, 0x23, 0, 0), Some(key_ev(KeyCode::End, KeyModifiers::empty()))),
        ("VK_HOME", key_record(true, 0x24, 0, 0), Some(key_ev(KeyCode::Home, KeyModifiers::empty()))),
        ("VK_LEFT", key_record(true, 0x25, 0, 0), Some(key_ev(KeyCode::Left, KeyModifiers::empty()))),
        ("VK_UP", key_record(true, 0x26, 0, 0), Some(key_ev(KeyCode::Up, KeyModifiers::empty()))),
        ("VK_RIGHT", key_record(true, 0x27, 0, 0), Some(key_ev(KeyCode::Right, KeyModifiers::empty()))),
        ("VK_DOWN", key_record(true, 0x28, 0, 0), Some(key_ev(KeyCode::Down, KeyModifiers::empty()))),
        ("VK_INSERT", key_record(true, 0x2D, 0, 0), Some(key_ev(KeyCode::Insert, KeyModifiers::empty()))),
        ("VK_DELETE", key_record(true, 0x2E, 0, 0), Some(key_ev(KeyCode::Delete, KeyModifiers::empty()))),
        ("VK_F1", key_record(true, 0x70, 0, 0), Some(key_ev(KeyCode::F(1), KeyModifiers::empty()))),
        ("VK_F12", key_record(true, 0x7B, 0, 0), Some(key_ev(KeyCode::F(12), KeyModifiers::empty()))),
        ("VK_F13", key_record(true, 0x7C, 0, 0), Some(key_ev(KeyCode::F(13), KeyModifiers::empty()))),
        ("VK_F24", key_record(true, 0x87, 0, 0), Some(key_ev(KeyCode::F(24), KeyModifiers::empty()))),
        ("Ctrl+A char", key_record(true, 0x41, b'a' as u16, LEFT_CTRL_PRESSED), Some(key_ev(KeyCode::Char('a'), KeyModifiers::CONTROL))),
        ("Alt+A char", key_record(true, 0x41, 0, LEFT_ALT_PRESSED), Some(key_ev(KeyCode::Char('a'), KeyModifiers::ALT))),
        ("Right Alt+Right Ctrl", key_record(true, 0x25, 0, RIGHT_ALT_PRESSED | RIGHT_CTRL_PRESSED), Some(key_ev(KeyCode::Left, KeyModifiers::ALT | KeyModifiers::CONTROL))),
        ("CAPSLOCK_ON -> state CAPS_LOCK", key_record(true, 0x41, b'A' as u16, CAPSLOCK_ON), Some(Event::Key(crate::term::event::KeyEvent { code: KeyCode::Char('A'), modifiers: KeyModifiers::empty(), kind: KeyEventKind::Press, state: KeyEventState::CAPS_LOCK }))),
        ("NUMLOCK_ON -> state NUM_LOCK", key_record(true, 0x25, 0, NUMLOCK_ON), Some(Event::Key(crate::term::event::KeyEvent { code: KeyCode::Left, modifiers: KeyModifiers::empty(), kind: KeyEventKind::Press, state: KeyEventState::NUM_LOCK }))),
        ("key up -> Release", key_record(false, 0x41, b'a' as u16, 0), Some(Event::Key(crate::term::event::KeyEvent { code: KeyCode::Char('a'), modifiers: KeyModifiers::empty(), kind: KeyEventKind::Release, state: KeyEventState::empty() }))),
        ("modifier-only VK_SHIFT -> None", key_record(true, 0x10, 0, SHIFT_PRESSED), None),
        ("modifier-only VK_CONTROL -> None", key_record(true, 0x11, 0, LEFT_CTRL_PRESSED), None),
        ("modifier-only VK_MENU -> None", key_record(true, 0x12, 0, LEFT_ALT_PRESSED), None),
        (
            "mouse down left",
            mouse_record(5, 5, FROM_LEFT_1ST_BUTTON_PRESSED, 0, 0),
            Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: 5, row: 5, modifiers: KeyModifiers::empty() })),
        ),
        (
            "mouse down right",
            mouse_record(1, 1, RIGHTMOST_BUTTON_PRESSED, 0, 0),
            Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Right), column: 1, row: 1, modifiers: KeyModifiers::empty() })),
        ),
        (
            "mouse down middle",
            mouse_record(2, 2, FROM_LEFT_2ND_BUTTON_PRESSED, 0, 0),
            Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Middle), column: 2, row: 2, modifiers: KeyModifiers::empty() })),
        ),
        (
            "mouse drag left",
            mouse_record(3, 3, FROM_LEFT_1ST_BUTTON_PRESSED, 0, MOUSE_MOVED),
            Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Drag(MouseButton::Left), column: 3, row: 3, modifiers: KeyModifiers::empty() })),
        ),
        (
            "mouse moved (no buttons)",
            mouse_record(4, 4, 0, 0, MOUSE_MOVED),
            Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Moved, column: 4, row: 4, modifiers: KeyModifiers::empty() })),
        ),
        (
            "mouse wheeled up",
            mouse_record(0, 0, 0x0078_0000, 0, MOUSE_WHEELED),
            Some(Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollUp, column: 0, row: 0, modifiers: KeyModifiers::empty() })),
        ),
        (
            "mouse wheeled down",
            mouse_record(0, 0, 0xFF88_0000, 0, MOUSE_WHEELED),
            Some(Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollDown, column: 0, row: 0, modifiers: KeyModifiers::empty() })),
        ),
        (
            "mouse hwheeled right",
            mouse_record(0, 0, 0x0078_0000, 0, MOUSE_HWHEELED),
            Some(Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollRight, column: 0, row: 0, modifiers: KeyModifiers::empty() })),
        ),
        (
            "mouse hwheeled left",
            mouse_record(0, 0, 0xFF88_0000, 0, MOUSE_HWHEELED),
            Some(Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollLeft, column: 0, row: 0, modifiers: KeyModifiers::empty() })),
        ),
        (
            "mouse double-click left stays Down",
            mouse_record(6, 6, FROM_LEFT_1ST_BUTTON_PRESSED, 0, DOUBLE_CLICK),
            Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: 6, row: 6, modifiers: KeyModifiers::empty() })),
        ),
        (
            "resize -> Resize(x+1, y+1)",
            resize_record(79, 23),
            Some(Event::Resize(80, 24)),
        ),
        ("focus gained", focus_record(true), Some(Event::FocusGained)),
        ("focus lost", focus_record(false), Some(Event::FocusLost)),
    ];

    assert!(
        cases.len() >= 30,
        "INPUT_RECORD -> Event fixture table must have >= 30 rows, has {}",
        cases.len()
    );

    for (label, rec, expected) in &cases {
        let mut d = InputDecoder::new();
        let got = d.decode(rec, 0);
        assert_eq!(&got, expected, "case {:?}", label);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Stateful behavior InputDecoder must track across records.
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mouse_up_detected_from_previous_button_state_transition() {
    let mut d = InputDecoder::new();
    // Press left, then release (button_state == 0, no flags) -- Up needs the
    // decoder to remember which button was previously down.
    let down = d.decode(&mouse_record(10, 10, FROM_LEFT_1ST_BUTTON_PRESSED, 0, 0), 0);
    assert_eq!(
        down,
        Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: 10, row: 10, modifiers: KeyModifiers::empty() }))
    );
    let up = d.decode(&mouse_record(10, 10, 0, 0, 0), 0);
    assert_eq!(
        up,
        Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Up(MouseButton::Left), column: 10, row: 10, modifiers: KeyModifiers::empty() }))
    );
}

#[test]
fn mouse_up_reports_the_button_that_was_released_not_always_left() {
    let mut d = InputDecoder::new();
    let _down = d.decode(&mouse_record(0, 0, RIGHTMOST_BUTTON_PRESSED, 0, 0), 0);
    let up = d.decode(&mouse_record(0, 0, 0, 0, 0), 0);
    assert_eq!(
        up,
        Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Up(MouseButton::Right), column: 0, row: 0, modifiers: KeyModifiers::empty() }))
    );
}

#[test]
fn surrogate_pair_buffered_then_joined_into_one_char_event() {
    let mut d = InputDecoder::new();
    // U+1F600 (grinning face) = surrogate pair 0xD83D 0xDE00.
    let high = d.decode(&key_record(true, 0, 0xD83D, 0), 0);
    assert_eq!(high, None, "a lone high surrogate must not emit an event yet");
    let low = d.decode(&key_record(true, 0, 0xDE00, 0), 0);
    let joined = char::from_u32(0x1F600).unwrap();
    assert_eq!(low, Some(key_ev(KeyCode::Char(joined), KeyModifiers::empty())));
}

#[test]
fn mouse_row_adjusted_by_window_top() {
    let mut d = InputDecoder::new();
    let got = d.decode(&mouse_record(2, 10, FROM_LEFT_1ST_BUTTON_PRESSED, 0, 0), 3);
    assert_eq!(
        got,
        Some(Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: 2, row: 7, modifiers: KeyModifiers::empty() }))
    );
}

#[test]
fn window_buffer_size_event_ignores_window_top() {
    let mut d = InputDecoder::new();
    let got = d.decode(&resize_record(99, 49), 3);
    assert_eq!(got, Some(Event::Resize(100, 50)));
}

// ═══════════════════════════════════════════════════════════════════════
// Stateless VK helpers, if reused verbatim from crate::ssh_input under
// their existing names (vk_to_keycode / vk_modifiers).
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn vk_to_keycode_and_vk_modifiers_reexported_from_console() {
    use crate::term::console::{vk_modifiers, vk_to_keycode};
    assert_eq!(vk_to_keycode(0x0D), Some(KeyCode::Enter));
    assert_eq!(vk_to_keycode(0x10), None);
    assert_eq!(vk_modifiers(SHIFT_PRESSED), KeyModifiers::SHIFT);
    assert_eq!(vk_modifiers(LEFT_CTRL_PRESSED | LEFT_ALT_PRESSED), KeyModifiers::CONTROL | KeyModifiers::ALT);
}

// Sanity: press() helper compiles against the chosen decode() signature.
#[test]
fn press_helper_smoke() {
    assert_eq!(press(0x1B, 0, 0), Some(key_ev(KeyCode::Esc, KeyModifiers::empty())));
}

// ═══════════════════════════════════════════════════════════════════════
// Covers: ZDEP-026
// Requirement: enabling native mouse capture must clear
// ENABLE_QUICK_EDIT_MODE (so a click doesn't fall into console text
// selection) while OR-ing ENABLE_MOUSE_INPUT | ENABLE_EXTENDED_FLAGS |
// ENABLE_WINDOW_INPUT onto whatever mode bits the console already has --
// unlike crossterm, which replaces the whole mode outright, psmux ORs onto
// the current mode so unrelated bits survive. Locked in here as the pure
// `mouse_capture_mode(mode: u32) -> u32` seam extracted from
// enable_mouse_capture_native so the bitmask math is unit-testable without
// a live console handle.
// ═══════════════════════════════════════════════════════════════════════

use crate::term::console::mouse_capture_mode;

const ENABLE_WINDOW_INPUT: u32 = 0x0008;
const ENABLE_MOUSE_INPUT: u32 = 0x0010;
const ENABLE_QUICK_EDIT_MODE: u32 = 0x0040;
const ENABLE_EXTENDED_FLAGS: u32 = 0x0080;

#[test]
fn mouse_capture_mode_clears_quick_edit() {
    let mode = ENABLE_QUICK_EDIT_MODE;
    let result = mouse_capture_mode(mode);
    assert_eq!(
        result & ENABLE_QUICK_EDIT_MODE,
        0,
        "ENABLE_QUICK_EDIT_MODE must be cleared so a click doesn't fall into \
         console text selection"
    );
}

#[test]
fn mouse_capture_mode_sets_mouse_extended_and_window_input_bits() {
    let result = mouse_capture_mode(0);
    assert_eq!(result & ENABLE_MOUSE_INPUT, ENABLE_MOUSE_INPUT, "ENABLE_MOUSE_INPUT must be set");
    assert_eq!(result & ENABLE_WINDOW_INPUT, ENABLE_WINDOW_INPUT, "ENABLE_WINDOW_INPUT must be set");
    // ENABLE_EXTENDED_FLAGS (0x80) is REQUIRED for the QUICK_EDIT clear to
    // take effect at all: SetConsoleMode silently ignores
    // ENABLE_QUICK_EDIT_MODE/ENABLE_INSERT_MODE writes unless
    // ENABLE_EXTENDED_FLAGS is also set in the same call, so omitting it
    // would leave Quick Edit re-asserting itself on the console.
    assert_eq!(
        result & ENABLE_EXTENDED_FLAGS,
        ENABLE_EXTENDED_FLAGS,
        "ENABLE_EXTENDED_FLAGS must be set -- without it SetConsoleMode ignores the QUICK_EDIT_MODE clear"
    );
}

#[test]
fn mouse_capture_mode_preserves_unrelated_pre_existing_bits() {
    // psmux ORs onto the current mode rather than replacing it outright
    // (unlike crossterm, which replaces the whole mode).
    let unrelated_bit = 0x0002; // ENABLE_LINE_INPUT-style bit, unrelated to mouse capture
    let mode = unrelated_bit;
    let result = mouse_capture_mode(mode);
    assert_eq!(result & unrelated_bit, unrelated_bit, "unrelated pre-existing bits must survive the OR");
}

#[test]
fn mouse_capture_mode_is_idempotent() {
    let mode = 0x2000; // some unrelated bit plus defaults
    let once = mouse_capture_mode(mode);
    let twice = mouse_capture_mode(once);
    assert_eq!(once, twice, "applying mouse_capture_mode twice must equal applying it once");
}

#[test]
fn mouse_capture_mode_edge_inputs() {
    let from_zero = mouse_capture_mode(0);
    assert_eq!(
        from_zero,
        ENABLE_MOUSE_INPUT | ENABLE_EXTENDED_FLAGS | ENABLE_WINDOW_INPUT,
        "mode 0 in: only the three OR'd-in bits should be set, QUICK_EDIT stays clear"
    );

    let from_max = mouse_capture_mode(u32::MAX);
    assert_eq!(
        from_max & ENABLE_QUICK_EDIT_MODE,
        0,
        "mode u32::MAX in: QUICK_EDIT_MODE must still be cleared"
    );
    assert_eq!(
        from_max & (ENABLE_MOUSE_INPUT | ENABLE_EXTENDED_FLAGS | ENABLE_WINDOW_INPUT),
        ENABLE_MOUSE_INPUT | ENABLE_EXTENDED_FLAGS | ENABLE_WINDOW_INPUT,
        "mode u32::MAX in: mouse/extended/window bits must still be set"
    );
    // every other bit besides QUICK_EDIT_MODE was already 1 in u32::MAX and
    // must remain 1 (OR never clears bits it doesn't own).
    assert_eq!(from_max, u32::MAX & !ENABLE_QUICK_EDIT_MODE);
}
