# r245 psmux slices feature area

Spec home for this repo is `docs/features/*.md`, not `.claude/spec-cache/`:
`.gitignore:89` ignores `.claude/` here, so no spec-cache area can live in
this repo. (Established after four agents independently searched only
`.claude/spec-cache/` and `.claude/skills/*/references/spec.md`, found
neither, and each proposed inventing a new FEAT-ID.)

Three slices from the r245 work: incremental layout dump (B, merged),
bracketed-paste client detection (A, in flight), session persistence (C,
landed on branch `r245/session-persist`). All IDs below are allocated in
this one pass; none are invented later.

## R245-001 Incremental layout dump skips unchanged panes

`dump_layout_json_fast_incremental` (`src/layout.rs`), built on a shared
`dump_layout_json_fast_impl(app, known: Option<&HashMap<usize,(u64,u16,u16)>>)`.
Emits `{"type":"leaf","id":<id>,"unchanged":true}` (src/layout.rs:665) for a
pane whose (generation, width, height) matches the caller's `known` map;
full leaf JSON otherwise. Wire-backward-safe: a client that sends no
revision map gets a full dump identical to `dump_layout_json_fast`. Merged
0264219 (v4.1.0).
Tests: `tests-rs/test_r245_incremental_dump.rs` (currently untagged --
retag with this ID): `incremental_dump_is_full_when_client_sends_no_known_revisions`,
`incremental_dump_marks_an_unchanged_pane_without_a_cell_grid`,
`incremental_dump_is_full_when_the_revision_advanced`,
`incremental_dump_mixed_window_one_changed_one_unchanged`,
`incremental_dump_is_full_for_a_pane_the_client_has_never_seen`,
`incremental_dump_treats_a_resize_as_changed_even_with_same_revision`,
`incremental_dump_unchanged_marker_is_driven_by_the_real_counter_not_a_constant`

## R245-002 Client-side bracketed-paste passthrough detection

`enum BracketedPasteWrapper { None, Incomplete, Complete { payload: String,
rest: String } }` and `fn scan_bracketed_paste_wrapper(buf: &str) ->
BracketedPasteWrapper`, scanning for `ESC[200~ ... ESC[201~` in the
client-side input buffer. The defect is DETECTION at
`src/client.rs:1617-2065`, not the server/ConPTY path. In flight, RED not
yet committed.

REFUTED theory, do not rebuild a fix for it: the "ConPTY strips the
bracket sequences" hypothesis at `src/input.rs:2978-2996` was empirically
refuted. The server path is not the defect.

Implementation prerequisites, both required for this ID to close:
- the literal Esc key must be routed into the accumulated buffer instead
  of immediately emitting `send-key esc\n` (today's unconditional path at
  `src/client.rs:3776`, `KeyCode::Esc => cmd_batch.push("send-key esc\n")`);
- `Incomplete` must never be swallowed -- the existing 300ms stage-2 timeout
  (`src/client.rs:1626`) stays as the fallback when a paste sequence never
  completes.
Tests: none yet (RED not committed); tag with this ID when written.

## R245-003 Session-persist snapshot round trip

Hand-rolled ASCII (no serde) round trip for `PaneSnapshot` /
`NodeSnapshot` / `WindowSnapshot` / `SessionSnapshot`, including nested
splits (direction and sizes preserved) and the `Pane` `cwd` /
`start_command` fields. Landed GREEN, branch `r245/session-persist`,
026e657. Replaces placeholder tag `R245-PERSIST`.
Tests: `tests-rs/test_session_persist.rs`:
`single_pane_round_trip_is_identity`,
`nested_split_round_trip_preserves_direction_order_and_sizes`,
`pane_cwd_and_start_command_round_trip`

## R245-004 Session-persist parse boundary treats file content as untrusted

`load_state` / `SCHEMA_VERSION` handling of a state file that is not
trusted input: bounds-checked, depth-capped, never panics. A future
schema version is reported as a `LoadError`, not panicked on; a truncated
or corrupt file is reported as corrupt, not panicked on; a missing file
loads as an empty `Vec`. Replaces placeholder tag `R245-PERSIST`.
Tests: `tests-rs/test_session_persist.rs`:
`future_schema_version_is_reported_not_panicked`,
`truncated_file_is_reported_as_corrupt_not_panicked`,
`missing_file_loads_as_empty_vec`

## R245-005 Session-persist state file path scoping

`state_file_path` reuses `paths::namespace_instance_file`: the path
differs per namespace and does not depend on the process's current
working directory. Replaces placeholder tag `R245-PERSIST`.
Tests: `tests-rs/test_session_persist.rs`:
`state_file_path_is_independent_of_process_cwd`,
`state_file_path_differs_per_namespace`

## R245-006 Session-persist restore name-collision gate

`restore_sessions` is a pure name-collision gate (no real process spawn):
it refuses to clobber a live session with the same name, and accepts all
snapshots when no name collides. Replaces placeholder tag `R245-PERSIST`.
Tests: `tests-rs/test_session_persist.rs`:
`restore_refuses_to_clobber_a_live_same_named_session`,
`restore_accepts_all_snapshots_when_no_collision`

## R245-007 Session-persist excludes child-app internal state

Persisted state carries only shell/command data (cwd, start_command,
layout) and never child-app internal state (scrollback, vt100 buffers,
undo history) -- a security/scope boundary on what gets written to disk.
Replaces placeholder tag `R245-PERSIST`.
Tests: `tests-rs/test_session_persist.rs`:
`persisted_state_never_contains_child_app_internal_state`

Out of scope for R245-003..R245-007 (session persistence), explicitly
excluded, not partially covered by any ID above: floating panes,
zoom-saved state, per-window options/hooks/key-tables, real process spawn
during restore, Windows service registration, installer changes, autostart
entries.
