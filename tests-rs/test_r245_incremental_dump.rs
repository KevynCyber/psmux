// Requirement: with several busy panes repainting, the client polls
// dump-state up to ~100x/sec while typing, and today's dump walks every
// row x col cell of every leaf pane on every call (src/layout.rs
// dump_layout_json_fast), even for panes whose content has not changed
// since the client last saw them. That O(panes x rows x cols) cost, run
// synchronously on the single server-loop thread that also services
// SendKey, is what makes fast keypresses appear to lag/vanish while
// several TUI panes repaint. The fix pinned here: an incremental dump
// that, given the client's last-seen (revision, rows, cols) per pane,
// emits a cheap "unchanged" marker instead of re-serialising a pane's
// cell grid when nothing has changed, while staying wire-backward-safe
// for a client that sends no revision map at all (full dump for every
// pane, exactly like today's dump_layout_json_fast).
//
// No spec-cache/FEAT-ID exists yet for this area (.claude/spec-cache is
// absent from this repo) -- no `// Covers:` tag until one is assigned.
//
// Dummy PTY (no real spawn), so this stays hermetic and portable, mirroring
// the harness in test_issue361_fastdump_hyperlink.rs.
//
// Pins the not-yet-existing entry point `dump_layout_json_fast_incremental`,
// which does not exist yet -- this file does not compile until it is added,
// which is itself part of the RED evidence.

use super::*;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
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

/// One window, one leaf pane with `bytes` already fed through vt100, wrapped
/// in a ready-to-dump `AppState`. Returns the app plus a handle to the pane's
/// revision counter, so a test can bump it between successive dump calls the
/// same way the real PTY reader thread would (src/pane.rs).
fn single_pane_app(bytes: &[u8]) -> (AppState, Arc<AtomicU64>) {
    let mut app = AppState::new("r245incr".to_string());
    app.window_base_index = 0;
    app.pane_base_index = 0;
    app.copy_command = String::new();
    app.set_clipboard = "off".to_string();
    let pane = make_pane(0, ROWS, COLS);
    let rev = pane.data_version.clone();
    pane.term.lock().expect("parser lock").process(bytes);
    let win = make_window(Node::Leaf(pane), vec![]);
    app.windows.push(win);
    app.active_idx = 0;
    app.last_window_area = psmux_tui::layout::Rect::new(0, 0, COLS, ROWS);
    (app, rev)
}

/// Two-leaf split window, so a test can advance one pane's revision and not
/// the other's within a single dump call.
fn two_pane_app() -> (AppState, Arc<AtomicU64>, Arc<AtomicU64>) {
    let mut app = AppState::new("r245incr2".to_string());
    app.window_base_index = 0;
    app.pane_base_index = 0;
    app.copy_command = String::new();
    app.set_clipboard = "off".to_string();
    let pane_a = make_pane(0, ROWS, COLS);
    let pane_b = make_pane(1, ROWS, COLS);
    let rev_a = pane_a.data_version.clone();
    let rev_b = pane_b.data_version.clone();
    pane_a.term.lock().expect("parser lock").process(b"pane a");
    pane_b.term.lock().expect("parser lock").process(b"pane b");
    let root = Node::Split {
        kind: LayoutKind::Horizontal,
        sizes: vec![50, 50],
        children: vec![Node::Leaf(pane_a), Node::Leaf(pane_b)],
    };
    let win = make_window(root, vec![0]);
    app.windows.push(win);
    app.active_idx = 0;
    app.last_window_area = psmux_tui::layout::Rect::new(0, 0, COLS, ROWS);
    (app, rev_a, rev_b)
}

/// Extracts the `{"type":"leaf","id":<id>,...}` object's raw text out of the
/// dump JSON, including nested braces (e.g. a full-dump leaf's `rows_v2`
/// entries). `start` points at the needle `"type":"leaf","id":<id>,`, which
/// begins one character past the leaf object's own opening `{`; `depth`
/// starts at 1 to account for that un-counted enclosing brace, so the loop
/// still breaks at the object's true closing `}` and matches the
/// string-assertion style already used by test_issue361_fastdump_hyperlink.rs
/// against this same serialiser.
fn leaf_json_for(json: &str, id: usize) -> String {
    let needle = format!("\"type\":\"leaf\",\"id\":{},", id);
    let start = json.find(&needle).unwrap_or_else(|| panic!("no leaf id={id} in {json}"));
    let mut depth = 1i32;
    let mut end = start;
    for (i, ch) in json[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    json[start..end].to_string()
}

#[test]
fn incremental_dump_is_full_when_client_sends_no_known_revisions() {
    // Design constraint: wire format change must be backward-safe -- a
    // client sending no revision map still gets a full dump for every pane.
    let (mut app, _rev) = single_pane_app(b"hello");
    let known: HashMap<usize, (u64, u16, u16)> = HashMap::new();
    let json = crate::layout::dump_layout_json_fast_incremental(&mut app, &known)
        .expect("incremental dump");
    let leaf = leaf_json_for(&json, 0);
    assert!(leaf.contains("\"rows_v2\":["), "empty known map must yield a full cell grid: {leaf}");
    assert!(!leaf.contains("\"unchanged\""), "full dump must not carry an unchanged marker: {leaf}");
}

#[test]
fn incremental_dump_marks_an_unchanged_pane_without_a_cell_grid() {
    let (mut app, rev) = single_pane_app(b"hello");
    let cur_rev = rev.load(Ordering::Acquire);
    let mut known = HashMap::new();
    known.insert(0usize, (cur_rev, ROWS, COLS));
    let json = crate::layout::dump_layout_json_fast_incremental(&mut app, &known)
        .expect("incremental dump");
    let leaf = leaf_json_for(&json, 0);
    assert!(leaf.contains("\"unchanged\":true"), "unchanged pane must carry the marker: {leaf}");
    assert!(!leaf.contains("\"rows_v2\":["), "unchanged pane must not re-serialise its cell grid: {leaf}");
}

#[test]
fn incremental_dump_is_full_when_the_revision_advanced() {
    let (mut app, rev) = single_pane_app(b"hello");
    let stale_rev = rev.load(Ordering::Acquire);
    // Simulate the PTY reader thread processing new output and bumping the
    // revision counter (src/pane.rs), exactly as a real content change would.
    {
        let win = &mut app.windows[0];
        if let Node::Leaf(p) = &mut win.root {
            p.term.lock().expect("parser lock").process(b" world");
        }
    }
    rev.fetch_add(1, Ordering::AcqRel);
    let mut known = HashMap::new();
    known.insert(0usize, (stale_rev, ROWS, COLS));
    let json = crate::layout::dump_layout_json_fast_incremental(&mut app, &known)
        .expect("incremental dump");
    let leaf = leaf_json_for(&json, 0);
    assert!(leaf.contains("\"rows_v2\":["), "advanced revision must yield a full cell grid: {leaf}");
    assert!(!leaf.contains("\"unchanged\""), "advanced revision must not be marked unchanged: {leaf}");
}

#[test]
fn incremental_dump_mixed_window_one_changed_one_unchanged() {
    let (mut app, rev_a, _rev_b) = two_pane_app();
    let stale_a = rev_a.load(Ordering::Acquire);
    let cur_b = {
        let win = &app.windows[0];
        match &win.root {
            Node::Split { children, .. } => match &children[1] {
                Node::Leaf(p) => p.data_version.load(Ordering::Acquire),
                _ => panic!("expected leaf"),
            },
            _ => panic!("expected split"),
        }
    };
    // Pane a's content changes; pane b stays exactly as it was.
    {
        let win = &mut app.windows[0];
        if let Node::Split { children, .. } = &mut win.root {
            if let Node::Leaf(p) = &mut children[0] {
                p.term.lock().expect("parser lock").process(b" changed");
            }
        }
    }
    rev_a.fetch_add(1, Ordering::AcqRel);

    let mut known = HashMap::new();
    known.insert(0usize, (stale_a, ROWS, COLS));
    known.insert(1usize, (cur_b, ROWS, COLS));
    let json = crate::layout::dump_layout_json_fast_incremental(&mut app, &known)
        .expect("incremental dump");

    let leaf_a = leaf_json_for(&json, 0);
    let leaf_b = leaf_json_for(&json, 1);
    assert!(leaf_a.contains("\"rows_v2\":["), "changed pane a must be a full dump: {leaf_a}");
    assert!(!leaf_a.contains("\"unchanged\""), "changed pane a must not carry unchanged marker: {leaf_a}");
    assert!(leaf_b.contains("\"unchanged\":true"), "untouched pane b must be marked unchanged: {leaf_b}");
    assert!(!leaf_b.contains("\"rows_v2\":["), "untouched pane b must not re-serialise its cell grid: {leaf_b}");
}

#[test]
fn incremental_dump_is_full_for_a_pane_the_client_has_never_seen() {
    // The pane's own revision counter starts at 0 (freshly spawned, never
    // written to), but the client has NO entry for it at all -- distinct
    // from "known revision happens to be 0": this is a first-ever dump.
    let (mut app, _rev) = single_pane_app(b"hello");
    let known: HashMap<usize, (u64, u16, u16)> = HashMap::new();
    let json = crate::layout::dump_layout_json_fast_incremental(&mut app, &known)
        .expect("incremental dump");
    let leaf = leaf_json_for(&json, 0);
    assert!(leaf.contains("\"rows_v2\":["), "a pane absent from the client's revision map must be a full dump: {leaf}");
}

#[test]
fn incremental_dump_treats_a_resize_as_changed_even_with_same_revision() {
    // Revision unchanged, but the client's last-seen geometry no longer
    // matches the pane's current rows/cols -- must still be a full dump so
    // the client's cached grid (sized for the OLD geometry) is never reused.
    let (mut app, rev) = single_pane_app(b"hello");
    let cur_rev = rev.load(Ordering::Acquire);
    let mut known = HashMap::new();
    known.insert(0usize, (cur_rev, ROWS, COLS + 10));
    let json = crate::layout::dump_layout_json_fast_incremental(&mut app, &known)
        .expect("incremental dump");
    let leaf = leaf_json_for(&json, 0);
    assert!(leaf.contains("\"rows_v2\":["), "geometry mismatch must force a full dump even at the same revision: {leaf}");
    assert!(!leaf.contains("\"unchanged\""), "geometry mismatch must not be marked unchanged: {leaf}");
}

#[test]
fn incremental_dump_unchanged_marker_is_driven_by_the_real_counter_not_a_constant() {
    // Guards against a stub that always answers "unchanged" (or always
    // "changed") regardless of the actual revision comparison: replay the
    // same known-revision map across two dumps, bumping the counter only
    // between them, and require the two dumps to disagree.
    let (mut app, rev) = single_pane_app(b"hello");
    let stale_rev = rev.load(Ordering::Acquire);
    let mut known = HashMap::new();
    known.insert(0usize, (stale_rev, ROWS, COLS));

    let json_before = crate::layout::dump_layout_json_fast_incremental(&mut app, &known)
        .expect("incremental dump (before write)");
    let leaf_before = leaf_json_for(&json_before, 0);
    assert!(leaf_before.contains("\"unchanged\":true"), "no write yet: must be unchanged: {leaf_before}");

    {
        let win = &mut app.windows[0];
        if let Node::Leaf(p) = &mut win.root {
            p.term.lock().expect("parser lock").process(b" more");
        }
    }
    rev.fetch_add(1, Ordering::AcqRel);

    let json_after = crate::layout::dump_layout_json_fast_incremental(&mut app, &known)
        .expect("incremental dump (after write)");
    let leaf_after = leaf_json_for(&json_after, 0);
    assert!(!leaf_after.contains("\"unchanged\""), "same known map, but the counter moved: must now be a full dump: {leaf_after}");
    assert!(leaf_after.contains("\"rows_v2\":["), "post-write dump must carry the real cell grid: {leaf_after}");
}
