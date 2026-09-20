// Requirement: psmux can save the shape of a session (windows, pane splits,
// each pane's cwd and start_command, sizes) to disk and reload it later, so a
// user can restart their machine or the psmux server without losing their
// working layout.
//
// Coverage IDs are tagged per section below (R245-003 .. R245-007), since
// this file locks five distinct requirements; see docs/features/
// r245-psmux-slices.md.
//
// Scope deliberately excluded from this slice (a later slice's job):
//   - floating panes and their geometry
//   - zoom-saved state
//   - per-window options, hooks, or key-tables
//   - actually spawning a real child process during restore (`restore_sessions`
//     here is a pure name-collision gate; no ConPTY is touched)
//   - Windows service registration, installer changes, or an autostart entry
//
// crate::persist has no third-party dep (psmux is zero-dep in Cargo.toml):
// the (de)serialiser here is hand-rolled, not serde-based.
//
// Filesystem tests route through PSMUX_DATA_DIR (see src/paths.rs) so they
// never touch %USERPROFILE%\.psmux; each test gets its own temp dir.

use super::*;
use std::io::Write as _;

/// Creates a fresh, empty temp directory under the OS temp root, unique per
/// call so parallel tests never collide.
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let mut dir = std::env::temp_dir();
    let unique = format!(
        "psmux-persist-test-{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    dir.push(unique);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn sample_pane(id: usize) -> PaneSnapshot {
    PaneSnapshot {
        id,
        cwd: format!("C:\\work\\proj{id}"),
        start_command: "cmd.exe".to_string(),
        rows: 40,
        cols: 120,
    }
}

fn sample_session(name: &str) -> SessionSnapshot {
    SessionSnapshot {
        name: name.to_string(),
        windows: vec![WindowSnapshot {
            id: 0,
            name: "main".to_string(),
            root: NodeSnapshot::Leaf(sample_pane(1)),
        }],
    }
}

/// Test-only helper: rewrites the schema version field inside a raw
/// hand-rolled state file to `new_version`, for the future-schema-version
/// test in the R245-004 section. Exact format is whatever `save_state`
/// emits; this just needs a version marker to exist and be rewritable.
fn bump_schema_version_in_raw(raw: &str, new_version: u32) -> String {
    let current = SCHEMA_VERSION.to_string();
    if let Some(pos) = raw.find(&current) {
        let mut out = String::with_capacity(raw.len() + 8);
        out.push_str(&raw[..pos]);
        out.push_str(&new_version.to_string());
        out.push_str(&raw[pos + current.len()..]);
        out
    } else {
        // No literal schema-version marker found (format not yet
        // implemented) -- fall back to a byte we can still corrupt so the
        // test at minimum exercises "not the exact bytes save_state wrote".
        let mut buf = raw.as_bytes().to_vec();
        if let Some(first) = buf.first_mut() {
            *first = first.wrapping_add(1);
        }
        String::from_utf8_lossy(&buf).into_owned()
    }
}

// =====================================================================
// Covers: R245-003
// Requirement: a session layout snapshot (windows, nested splits with
// direction/order/sizes, and each pane's cwd and start_command) survives
// a save_state/load_state round trip unchanged, so a user's working
// layout can be reloaded after a restart.
// =====================================================================

// Lock 1: a single-pane session round-trips through save_state/load_state
// byte-for-byte as data (identity on the parsed structures).
#[test]
fn single_pane_round_trip_is_identity() {
    let dir = temp_dir("single-pane");
    let sessions = vec![sample_session("work")];

    save_state(&dir, None, &sessions).expect("save_state should succeed");
    let loaded = load_state(&dir, None).expect("load_state should succeed");

    assert_eq!(loaded, sessions);
}

// Lock 2: a nested split (horizontal split whose second child is itself a
// vertical split) round-trips preserving direction, child order, and sizes.
#[test]
fn nested_split_round_trip_preserves_direction_order_and_sizes() {
    let dir = temp_dir("nested-split");
    let inner = NodeSnapshot::Split {
        kind: SplitKind::Vertical,
        sizes: vec![30, 70],
        children: vec![
            NodeSnapshot::Leaf(sample_pane(2)),
            NodeSnapshot::Leaf(sample_pane(3)),
        ],
    };
    let root = NodeSnapshot::Split {
        kind: SplitKind::Horizontal,
        sizes: vec![50, 50],
        children: vec![NodeSnapshot::Leaf(sample_pane(1)), inner],
    };
    let sessions = vec![SessionSnapshot {
        name: "split-sess".to_string(),
        windows: vec![WindowSnapshot {
            id: 0,
            name: "main".to_string(),
            root,
        }],
    }];

    save_state(&dir, None, &sessions).expect("save_state should succeed");
    let loaded = load_state(&dir, None).expect("load_state should succeed");

    assert_eq!(loaded, sessions, "split direction/order/sizes must survive round trip");
}

// Lock 3: pane cwd and start_command survive the round trip verbatim,
// including a start_command with embedded spaces and quotes.
#[test]
fn pane_cwd_and_start_command_round_trip() {
    let dir = temp_dir("cwd-cmd");
    let pane = PaneSnapshot {
        id: 7,
        cwd: "C:\\Users\\Kev\\projects\\psmux-rust".to_string(),
        start_command: "powershell.exe -NoExit -Command \"git status\"".to_string(),
        rows: 24,
        cols: 80,
    };
    let sessions = vec![SessionSnapshot {
        name: "cmdsess".to_string(),
        windows: vec![WindowSnapshot {
            id: 0,
            name: "main".to_string(),
            root: NodeSnapshot::Leaf(pane.clone()),
        }],
    }];

    save_state(&dir, None, &sessions).expect("save_state should succeed");
    let loaded = load_state(&dir, None).expect("load_state should succeed");

    match &loaded[0].windows[0].root {
        NodeSnapshot::Leaf(p) => {
            assert_eq!(p.cwd, pane.cwd);
            assert_eq!(p.start_command, pane.start_command);
        }
        NodeSnapshot::Split { .. } => panic!("expected a leaf pane"),
    }
}

// =====================================================================
// Covers: R245-004
// Requirement: the state file is untrusted input at the parse boundary.
// A future schema version is reported as a typed LoadError, a truncated
// or corrupt file is reported as corrupt, and a missing file loads as an
// empty Vec -- none of these ever panic the server.
// =====================================================================

// Lock 4: a state file declaring a schema version newer than this build
// understands must be reported as LoadError::UnsupportedSchemaVersion, never
// panic (the file may have been written by a future psmux version).
#[test]
fn future_schema_version_is_reported_not_panicked() {
    let dir = temp_dir("future-schema");
    let sessions = vec![sample_session("work")];
    save_state(&dir, None, &sessions).expect("save_state should succeed");

    let path = state_file_path(&dir, None);
    let raw = std::fs::read_to_string(&path).expect("read state file");
    // Bump the persisted schema version far past anything this build knows,
    // simulating a file written by a much newer psmux.
    let future_version = SCHEMA_VERSION + 1000;
    let bumped = bump_schema_version_in_raw(&raw, future_version);
    std::fs::write(&path, bumped).expect("write bumped state file");

    let result = std::panic::catch_unwind(|| load_state(&dir, None));
    let result = result.expect("load_state must not panic on a future schema version");
    match result {
        Err(LoadError::UnsupportedSchemaVersion(v)) => assert_eq!(v, future_version),
        other => panic!("expected UnsupportedSchemaVersion, got {other:?}"),
    }
}

// Lock 5: a truncated/corrupt state file must be reported as
// LoadError::Corrupt, never panic.
#[test]
fn truncated_file_is_reported_as_corrupt_not_panicked() {
    let dir = temp_dir("corrupt");
    let sessions = vec![sample_session("work")];
    save_state(&dir, None, &sessions).expect("save_state should succeed");

    let path = state_file_path(&dir, None);
    let raw = std::fs::read_to_string(&path).expect("read state file");
    // Truncate to a prefix that cannot possibly be valid, whatever the
    // hand-rolled format is (too short to contain a full record).
    let truncated = if raw.len() > 4 { &raw[..raw.len() / 4] } else { "" };
    std::fs::write(&path, truncated).expect("write truncated state file");

    let result = std::panic::catch_unwind(|| load_state(&dir, None));
    let result = result.expect("load_state must not panic on a corrupt file");
    match result {
        Err(LoadError::Corrupt(_)) => {}
        other => panic!("expected Corrupt, got {other:?}"),
    }
}

// Lock 6: a missing state file is not an error -- it means "nothing has ever
// been saved yet", so load_state returns an empty vec.
#[test]
fn missing_file_loads_as_empty_vec() {
    let dir = temp_dir("missing");
    // Deliberately do not call save_state: the directory exists but holds no
    // state file at all.
    let loaded = load_state(&dir, None).expect("a missing file must not be an error");
    assert_eq!(loaded, Vec::<SessionSnapshot>::new());
}

// =====================================================================
// Covers: R245-005
// Requirement: the persisted state file path is scoped to the data dir
// and namespace only -- it never depends on the process's current
// working directory, and two namespaces never share one file.
// =====================================================================

// Lock 7: state_file_path depends only on the given dir/namespace, never on
// the process's current working directory.
#[test]
fn state_file_path_is_independent_of_process_cwd() {
    let dir = temp_dir("cwd-independent");
    let before = state_file_path(&dir, None);

    // Changing the process cwd must not change the resolved path, since the
    // dir argument is already absolute and namespace-scoped.
    let original_cwd = std::env::current_dir().expect("read cwd");
    let elsewhere = temp_dir("elsewhere-cwd");
    std::env::set_current_dir(&elsewhere).expect("change cwd");
    let after = state_file_path(&dir, None);
    std::env::set_current_dir(&original_cwd).expect("restore cwd");

    assert_eq!(before, after);
}

// Lock 8: two different namespaces resolve to two different state file
// paths under the same data dir, so namespaces cannot clobber each other's
// persisted sessions.
#[test]
fn state_file_path_differs_per_namespace() {
    let dir = temp_dir("per-namespace");
    let default_path = state_file_path(&dir, None);
    let ns_a = state_file_path(&dir, Some("alpha"));
    let ns_b = state_file_path(&dir, Some("beta"));

    assert_ne!(default_path, ns_a);
    assert_ne!(default_path, ns_b);
    assert_ne!(ns_a, ns_b);
}

// =====================================================================
// Covers: R245-006
// Requirement: restore is gated on session-name collisions -- it refuses
// to clobber a live session of the same name (typed error naming the
// session), and accepts every snapshot when no name collides.
// =====================================================================

// Lock 9: restoring must refuse to clobber a session name that is already
// live, reporting a typed NameCollision rather than silently overwriting or
// silently skipping.
#[test]
fn restore_refuses_to_clobber_a_live_same_named_session() {
    let snapshots = vec![sample_session("work"), sample_session("scratch")];
    let live = vec!["scratch".to_string()];

    let result = restore_sessions(&snapshots, &live);

    match result {
        Err(RestoreError::NameCollision(name)) => assert_eq!(name, "scratch"),
        other => panic!("expected NameCollision(\"scratch\"), got {other:?}"),
    }
}

// Lock 10: when none of the persisted session names collide with a live
// session, every snapshot is accepted for restore.
#[test]
fn restore_accepts_all_snapshots_when_no_collision() {
    let snapshots = vec![sample_session("work"), sample_session("scratch")];
    let live: Vec<String> = vec![];

    let accepted = restore_sessions(&snapshots, &live).expect("no collision should not error");

    assert_eq!(accepted.len(), 2);
    assert!(accepted.contains(&"work".to_string()));
    assert!(accepted.contains(&"scratch".to_string()));
}

// =====================================================================
// Covers: R245-007
// Requirement: persisted state carries only the shell/command data
// needed to recreate a pane (cwd, start_command, dimensions) and never a
// child application's internal state -- a scope/security boundary on
// what psmux writes to disk.
// =====================================================================

// Lock 11 (regression lock): persisted state must contain only the shell/
// command data needed to recreate a pane's process (cwd, start_command,
// dimensions) -- never a child application's internal state such as
// scrollback, terminal emulation buffers, or copy-mode undo history. Those
// belong to the running ConPTY/vt100 layer, not to a layout snapshot, and
// leaking them here would bloat the state file and couple persistence to
// unrelated internals.
#[test]
fn persisted_state_never_contains_child_app_internal_state() {
    let dir = temp_dir("no-internal-state");
    let sessions = vec![sample_session("work")];
    save_state(&dir, None, &sessions).expect("save_state should succeed");

    let path = state_file_path(&dir, None);
    let raw = std::fs::read_to_string(&path).expect("read state file");
    let lower = raw.to_lowercase();

    for forbidden in ["scrollback", "vt100", "screen_buffer", "undo_buffer"] {
        assert!(
            !lower.contains(forbidden),
            "persisted state file must never contain {forbidden:?}, got: {raw}"
        );
    }
}
