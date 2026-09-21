// Requirement: #{pane_in_mode} and #{pane_mode} must report the COPY-MODE
// state of the TARGETED pane (e.g. `display-message -t <pane> '#{pane_in_mode}'`,
// or a list-panes -F row for a non-active pane), not the server's single
// global `app.mode`. Per-pane copy state already exists on `Pane.copy_state`
// (src/types.rs:209), set by `copy_mode::enter_copy_mode` /
// `save_copy_state_to_pane` and cleared by `exit_copy_mode`
// (src/copy_mode.rs). Before this fix, `src/format.rs` matched only on
// `app.mode`, so with pane A in copy mode and pane B not, asking for pane
// B's `#{pane_in_mode}` incorrectly reported "1" (copy-mode) because it read
// the server-wide mode instead of `target_pane().copy_state`.
//
// No FEAT-ID fits: docs/features/*.md (this repo's spec home, see
// r245-psmux-slices.md) has no entry covering `pane_in_mode`/`pane_mode`
// format-variable semantics, so this file carries no `// Covers:` tag.
//
// Dummy PTY (no real spawn), mirroring the two_pane_app harness in
// tests-rs/test_r245_incremental_dump.rs, so this stays hermetic.

use super::*;

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::types::{AppState, LayoutKind, Node};

const ROWS: u16 = 4;
const COLS: u16 = 40;

#[derive(Debug)]
struct DummyChild;

#[derive(Debug)]
struct DummyWriter;

struct DummyMaster;

impl std::io::Write for DummyWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> { Ok(buf.len()) }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

impl crate::pty::ChildKiller for DummyChild {
    fn kill(&mut self) -> std::io::Result<()> { Ok(()) }
    fn clone_killer(&self) -> Box<dyn crate::pty::ChildKiller + Send + Sync> {
        Box::new(DummyChild)
    }
}

impl crate::pty::Child for DummyChild {
    fn try_wait(&mut self) -> std::io::Result<Option<crate::pty::ExitStatus>> {
        Ok(Some(crate::pty::ExitStatus::with_exit_code(0)))
    }
    fn wait(&mut self) -> std::io::Result<crate::pty::ExitStatus> {
        Ok(crate::pty::ExitStatus::with_exit_code(0))
    }
    fn process_id(&self) -> Option<u32> { None }
    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> { None }
}

impl crate::pty::MasterPty for DummyMaster {
    fn resize(&self, _size: crate::pty::PtySize) -> Result<(), crate::pty::Error> { Ok(()) }
    fn get_size(&self) -> Result<crate::pty::PtySize, crate::pty::Error> {
        Ok(crate::pty::PtySize { rows: ROWS, cols: COLS, pixel_width: 0, pixel_height: 0 })
    }
    fn try_clone_reader(&self) -> Result<Box<dyn std::io::Read + Send>, crate::pty::Error> {
        Ok(Box::new(std::io::empty()))
    }
    fn take_writer(&self) -> Result<Box<dyn std::io::Write + Send>, crate::pty::Error> {
        Ok(Box::new(DummyWriter))
    }
    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<i32> { None }
    #[cfg(unix)]
    fn as_raw_fd(&self) -> Option<std::os::unix::io::RawFd> { None }
    #[cfg(unix)]
    fn tty_name(&self) -> Option<std::path::PathBuf> { None }
}

fn make_pane(id: usize, rows: u16, cols: u16) -> crate::types::Pane {
    let term = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 0)));
    let epoch = Instant::now() - Duration::from_secs(2);
    crate::types::Pane {
        master: Box::new(DummyMaster),
        writer: Box::new(DummyWriter),
        child: Box::new(DummyChild),
        term,
        last_rows: rows,
        last_cols: cols,
        id,
        title: String::new(),
        title_locked: false,
        child_pid: None,
        data_version: Arc::new(AtomicU64::new(0)),
        last_title_check: epoch,
        last_infer_title: epoch,
        dead: false,
        last_text_input: None,
        last_special_key: None,
        vt_bridge_cache: None,
        vti_mode_cache: None,
        mouse_input_cache: None,
        scroll_fg_cache: None,
        cursor_shape: Arc::new(AtomicU8::new(0)),
        bell_pending: Arc::new(AtomicBool::new(false)),
        cpr_pending: Arc::new(AtomicBool::new(false)),
        color_query_pending: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        copy_state: None,
        pane_style: None, pane_options: Default::default(),
        squelch_until: None,
        output_ring: Arc::new(Mutex::new(std::collections::VecDeque::new())),
        spawned_at: None,
    }
}

fn make_window(root: Node, active_path: Vec<usize>) -> crate::types::Window {
    crate::types::Window {
        root,
        active_path,
        name: "w".to_string(),
        id: 0,
        area: psmux_tui::layout::Rect::new(0, 0, COLS, ROWS),
        window_size: None,
        activity_flag: false,
        bell_flag: false,
        silence_flag: false,
        last_output_time: Instant::now(),
        last_seen_version: 0,
        manual_rename: false,
        layout_index: 0,
        pane_mru: vec![],
        zoom_saved: None,
        linked_from: None,
        floating: Vec::new(),
        floating_focus: None,
    }
}

/// Two-leaf split window: pane 0 (active) will be put into copy mode, pane 1
/// stays in Passthrough. Pane ids double as the split position (0, 1).
fn two_pane_app() -> AppState {
    let mut app = AppState::new("r252panemode".to_string());
    app.window_base_index = 0;
    app.pane_base_index = 0;
    app.copy_command = String::new();
    app.set_clipboard = "off".to_string();
    let pane_a = make_pane(0, ROWS, COLS);
    let pane_b = make_pane(1, ROWS, COLS);
    let root = Node::Split {
        kind: LayoutKind::Horizontal,
        sizes: vec![50, 50],
        children: vec![Node::Leaf(pane_a), Node::Leaf(pane_b)],
    };
    // active_path [0] selects pane_a (the first child) as the active pane.
    let win = make_window(root, vec![0]);
    app.windows.push(win);
    app.active_idx = 0;
    app.last_window_area = psmux_tui::layout::Rect::new(0, 0, COLS, ROWS);
    app
}

#[test]
fn pane_in_mode_and_pane_mode_honor_target_pane_not_global_mode() {
    let mut app = two_pane_app();

    // Put the ACTIVE pane (id 0) into copy mode via the real entry point,
    // so Pane 0's per-pane copy_state is set exactly as production code
    // would set it.
    crate::copy_mode::enter_copy_mode(&mut app);

    // The pane actually in copy mode (id 0) must report "1" / "copy-mode".
    assert_eq!(
        expand_format_for_pane_by_id("#{pane_in_mode}", &app, 0),
        "1",
        "pane 0 is in copy mode and must report pane_in_mode=1"
    );
    assert_eq!(
        expand_format_for_pane_by_id("#{pane_mode}", &app, 0),
        "copy-mode",
        "pane 0 is in copy mode and must report pane_mode=copy-mode"
    );

    // The OTHER pane (id 1) never entered copy mode -- its copy_state is
    // still None -- so it must report "0" / "" even though the server's
    // global app.mode is currently CopyMode (set by pane 0's entry above).
    assert_eq!(
        expand_format_for_pane_by_id("#{pane_in_mode}", &app, 1),
        "0",
        "pane 1 was never put in copy mode; global app.mode must not leak into its pane_in_mode"
    );
    assert_eq!(
        expand_format_for_pane_by_id("#{pane_mode}", &app, 1),
        "",
        "pane 1 was never put in copy mode; global app.mode must not leak into its pane_mode"
    );
}
