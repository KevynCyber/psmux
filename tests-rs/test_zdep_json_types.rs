// Covers: ZDEP-015
// Requirement: every serde-derived type inventoried in ZDEP-015 gets a
// hand-written `psmux_json::ToJson`/`psmux_json::FromJson` impl that
// reproduces the derive semantics of its attributes exactly: field order is
// declaration order; a field without `#[serde(default)]` is required
// (absence is `Err`) except an `Option<T>` field which is `None` when
// absent; `#[serde(default)]` substitutes `Default::default()`;
// `#[serde(default = "f")]` calls `f`; unknown keys are ignored;
// `#[serde(tag = "type")]` + `#[serde(rename = ...)]` on `LayoutJson` and
// `LayoutSimple` reads/writes `"type"` and rejects an unknown or missing
// tag; `CellRunJson.link` with `skip_serializing_if = "Option::is_none"` is
// absent from the output when `None`. Type mismatches (`1.5`/`"1"` for a
// `u16`, `256` for a `u8`, `null` for a required `String`) are `Err` like
// serde. Every row replayed here is taken verbatim from the committed
// oracle fixture `tests-rs/fixtures/serde_json_1.0.151.txt` (generated once
// from serde 1.0.229 + serde_json 1.0.151).
//
// NOTE (test-engineer, for GREEN / software-engineer): `WinStatus`,
// `BindingEntry`, `ServerMenuItem`, `CustomizeOption`, and `DumpState` (plus
// its `default_*` functions) are currently declared *inside* a function
// body in `src/client.rs` (around line 1729), not at module scope. A path
// like `crate::client::WinStatus` does not exist today; these tests assume
// GREEN hoists them to module scope as `pub(crate)` (mirroring `FloatJson`,
// already `pub(crate)` at module scope in `src/client.rs:23`) so this file
// (wired via a `#[path]` `mod` per `src/tests_zdep_wiring.rs`) can name them.

use psmux_json::{from_str, to_string, FromJson, ToJson};

// ── fixture replay plumbing ──

const FIXTURE: &str = include_str!("fixtures/serde_json_1.0.151.txt");

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('p') => out.push('|'),
                Some('n') => out.push('\n'),
                Some(other) => { out.push('\\'); out.push(other); }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Returns the unescaped payload of the row `<kind>|<name>|<payload>` whose
/// name matches exactly.
fn row(kind: &str, name: &str) -> String {
    for line in FIXTURE.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(3, '|');
        let k = parts.next().unwrap_or_default();
        let n = parts.next().unwrap_or_default();
        if k == kind && n == name {
            return unescape(parts.next().unwrap_or_default());
        }
    }
    panic!("no {kind} row named {name:?} in fixture");
}

/// Full de-ok -> de-eq round trip: `from_str::<T>` on the de-ok input must
/// equal (after `to_string`) the de-eq row recorded from serde_json's own
/// re-serialisation of the same parse.
fn assert_round_trip<T: FromJson + ToJson>(name: &str) {
    let input = row("de-ok", name);
    let want = row("de-eq", name);
    let parsed: T = from_str(&input).unwrap_or_else(|e| panic!("{name} must parse {input:?}: {e:?}"));
    assert_eq!(to_string(&parsed), want, "{name} round-trip mismatch");
}

fn assert_de_err<T: FromJson>(name: &str) {
    let input = row("de-err", name);
    let result: Result<T, _> = from_str(&input);
    assert!(result.is_err(), "{name} should fail to parse: {input:?}");
}

// ── CellJson (src/layout.rs): 11 required fields, no attributes beyond
// the derive itself ──

#[test]
fn cell_json_full_key_round_trip() {
    assert_round_trip::<crate::layout::CellJson>("CellJson");
}

#[test]
fn cell_json_missing_required_text_is_err() {
    assert_de_err::<crate::layout::CellJson>("CellJson_missing_text");
}

#[test]
fn cell_json_wrong_type_for_required_string_is_err() {
    assert_de_err::<crate::layout::CellJson>("CellJson_text_null");
    assert_de_err::<crate::layout::CellJson>("CellJson_text_bool");
}

// ── CellRunJson: `link` carries `#[serde(default, skip_serializing_if =
// "Option::is_none")]` ──

#[test]
fn cell_run_json_full_key_round_trip() {
    assert_round_trip::<crate::layout::CellRunJson>("CellRunJson_full");
}

#[test]
fn cell_run_json_link_absent_defaults_to_none_on_deserialize() {
    let input = row("de-ok", "CellRunJson_minimal");
    let parsed: crate::layout::CellRunJson = from_str(&input).expect("must parse without link key");
    assert!(parsed.link.is_none());
}

#[test]
fn cell_run_json_link_null_is_none() {
    let input = row("de-ok", "CellRunJson_link_null");
    let parsed: crate::layout::CellRunJson = from_str(&input).expect("must parse null link");
    assert!(parsed.link.is_none());
}

#[test]
fn cell_run_json_unknown_key_is_ignored() {
    assert_round_trip::<crate::layout::CellRunJson>("CellRunJson_unknown_key");
}

/// The field-absence contract for `skip_serializing_if`: when `link` is
/// `None`, the serialized output must not contain the `"link"` key at all.
#[test]
fn cell_run_json_link_none_is_absent_from_serialized_output() {
    let run = crate::layout::CellRunJson {
        text: "x".into(), fg: "r".into(), bg: "b".into(), flags: 0, width: 1, link: None,
    };
    let json = to_string(&run);
    assert!(!json.contains("link"), "link key must be absent when None: {json}");
}

/// ...and present (as a string) when `Some`.
#[test]
fn cell_run_json_link_some_is_present_in_serialized_output() {
    let run = crate::layout::CellRunJson {
        text: "x".into(), fg: "r".into(), bg: "b".into(), flags: 0, width: 1, link: Some("u".into()),
    };
    let json = to_string(&run);
    assert!(json.contains("\"link\":\"u\""), "link key must carry the value when Some: {json}");
}

#[test]
fn cell_run_json_width_type_mismatches_are_err() {
    assert_de_err::<crate::layout::CellRunJson>("CellRunJson_width_string");
    assert_de_err::<crate::layout::CellRunJson>("CellRunJson_width_float");
}

#[test]
fn cell_run_json_flags_u8_out_of_range_is_err() {
    assert_de_err::<crate::layout::CellRunJson>("CellRunJson_flags_256");
    assert_de_err::<crate::layout::CellRunJson>("CellRunJson_flags_negative");
}

// ── RowRunsJson: plain nested Vec<CellRunJson> ──

#[test]
fn row_runs_json_round_trip() {
    assert_round_trip::<crate::layout::RowRunsJson>("RowRunsJson");
}

// ── LayoutJson: `#[serde(tag = "type")]` with `rename = "split"`/`"leaf"`,
// several `#[serde(default)]` leaf fields ──

#[test]
fn layout_json_split_round_trip() {
    assert_round_trip::<crate::layout::LayoutJson>("LayoutJson_split");
}

#[test]
fn layout_json_leaf_full_key_round_trip() {
    assert_round_trip::<crate::layout::LayoutJson>("LayoutJson_leaf_full");
}

/// Every `#[serde(default)]` field on the Leaf variant (alternate_screen,
/// wants_mouse, hide_cursor, cursor_shape, sel_mode, copy_cursor_row,
/// copy_cursor_col, content, rows_v2, title) is absent from this row and
/// must default rather than error; the tag key is deliberately last, not
/// first, to prove tag position doesn't matter on read.
#[test]
fn layout_json_leaf_minimal_defaults_every_optional_field() {
    let input = row("de-ok", "LayoutJson_leaf_minimal");
    let parsed: crate::layout::LayoutJson = from_str(&input).expect("must parse minimal leaf");
    match parsed {
        crate::layout::LayoutJson::Leaf {
            alternate_screen, wants_mouse, hide_cursor, cursor_shape,
            sel_mode, copy_cursor_row, copy_cursor_col, content, rows_v2, title, ..
        } => {
            assert!(!alternate_screen);
            assert!(!wants_mouse);
            assert!(!hide_cursor);
            assert_eq!(cursor_shape, 0);
            assert_eq!(sel_mode, None);
            assert_eq!(copy_cursor_row, None);
            assert_eq!(copy_cursor_col, None);
            assert!(content.is_empty());
            assert!(rows_v2.is_empty());
            assert_eq!(title, None);
        }
        _ => panic!("expected Leaf"),
    }
}

#[test]
fn layout_json_missing_tag_is_err() {
    assert_de_err::<crate::layout::LayoutJson>("LayoutJson_missing_tag");
}

#[test]
fn layout_json_unknown_tag_is_err() {
    assert_de_err::<crate::layout::LayoutJson>("LayoutJson_unknown_tag");
}

#[test]
fn layout_json_tag_wrong_type_is_err() {
    assert_de_err::<crate::layout::LayoutJson>("LayoutJson_tag_wrong_type");
}

/// The tag key is written first on output (matches serde_json's field-order
/// behavior for `#[serde(tag = "type")]`).
#[test]
fn layout_json_writes_tag_key_first() {
    let leaf = crate::layout::LayoutJson::Leaf {
        id: 1, rows: 1, cols: 1, cursor_row: 0, cursor_col: 0,
        alternate_screen: false, wants_mouse: false, hide_cursor: false, cursor_shape: 0,
        active: false, copy_mode: false, scroll_offset: 0,
        sel_start_row: None, sel_start_col: None, sel_end_row: None, sel_end_col: None,
        sel_mode: None, copy_cursor_row: None, copy_cursor_col: None,
        content: vec![], rows_v2: vec![], title: None,
    };
    let json = to_string(&leaf);
    assert!(json.starts_with("{\"type\":\"leaf\""), "tag must be first key: {json}");
}

// ── util.rs types ──

#[test]
fn win_info_full_key_round_trip() {
    assert_round_trip::<crate::util::WinInfo>("WinInfo_full");
}

#[test]
fn win_info_minimal_defaults_activity_bell_last_tab_text_idx() {
    let input = row("de-ok", "WinInfo_minimal");
    let parsed: crate::util::WinInfo = from_str(&input).expect("must parse minimal WinInfo");
    assert!(!parsed.activity);
    assert!(!parsed.bell);
    assert!(!parsed.last);
    assert_eq!(parsed.tab_text, "");
    assert_eq!(parsed.idx, 0);
}

#[test]
fn win_info_id_type_mismatches_are_err() {
    assert_de_err::<crate::util::WinInfo>("WinInfo_id_negative");
    assert_de_err::<crate::util::WinInfo>("WinInfo_id_string");
}

#[test]
fn pane_info_round_trip() {
    assert_round_trip::<crate::util::PaneInfo>("PaneInfo");
}

#[test]
fn win_tree_full_key_round_trip() {
    assert_round_trip::<crate::util::WinTree>("WinTree_full");
}

#[test]
fn win_tree_minimal_defaults_idx() {
    let input = row("de-ok", "WinTree_minimal");
    let parsed: crate::util::WinTree = from_str(&input).expect("must parse minimal WinTree");
    assert_eq!(parsed.idx, 0);
}

#[test]
fn layout_simple_split_round_trip() {
    assert_round_trip::<crate::util::LayoutSimple>("LayoutSimple_split");
}

#[test]
fn layout_simple_leaf_minimal_defaults_active() {
    let input = row("de-ok", "LayoutSimple_leaf_minimal");
    let parsed: crate::util::LayoutSimple = from_str(&input).expect("must parse minimal leaf");
    match parsed {
        crate::util::LayoutSimple::Leaf { active, .. } => assert!(!active),
        _ => panic!("expected Leaf"),
    }
}

#[test]
fn layout_simple_unknown_tag_is_err() {
    assert_de_err::<crate::util::LayoutSimple>("LayoutSimple_unknown_tag");
}

// ── src/client.rs types ──

#[test]
fn float_json_full_key_round_trip() {
    assert_round_trip::<crate::client::FloatJson>("FloatJson_full");
}

#[test]
fn float_json_minimal_defaults_every_field() {
    let input = row("de-ok", "FloatJson_minimal");
    let parsed: crate::client::FloatJson = from_str(&input).expect("must parse empty object");
    assert_eq!(parsed.x, 0);
    assert_eq!(parsed.y, 0);
    assert_eq!(parsed.w, 0);
    assert_eq!(parsed.h, 0);
    assert_eq!(parsed.border, "");
    assert!(!parsed.focused);
    assert_eq!(parsed.title, "");
    assert!(parsed.rows.is_empty());
}

#[test]
fn win_status_full_key_round_trip() {
    assert_round_trip::<crate::client::WinStatus>("WinStatus_full");
}

#[test]
fn win_status_minimal_defaults_activity_bell_last_tab_text_idx() {
    let input = row("de-ok", "WinStatus_minimal");
    let parsed: crate::client::WinStatus = from_str(&input).expect("must parse minimal WinStatus");
    assert!(!parsed.activity);
    assert!(!parsed.bell);
    assert!(!parsed.last);
    assert_eq!(parsed.tab_text, "");
    assert_eq!(parsed.idx, 0);
}

#[test]
fn binding_entry_full_key_round_trip() {
    assert_round_trip::<crate::client::BindingEntry>("BindingEntry_full");
}

#[test]
fn binding_entry_minimal_defaults_r() {
    let input = row("de-ok", "BindingEntry_minimal");
    let parsed: crate::client::BindingEntry = from_str(&input).expect("must parse minimal BindingEntry");
    assert!(!parsed.r);
}

#[test]
fn server_menu_item_full_key_round_trip() {
    assert_round_trip::<crate::client::ServerMenuItem>("ServerMenuItem_full");
}

#[test]
fn server_menu_item_minimal_defaults_every_field() {
    let input = row("de-ok", "ServerMenuItem_minimal");
    let parsed: crate::client::ServerMenuItem = from_str(&input).expect("must parse empty object");
    assert_eq!(parsed.name, None);
    assert_eq!(parsed.key, None);
    assert!(!parsed.sep);
}

#[test]
fn customize_option_round_trip() {
    assert_round_trip::<crate::client::CustomizeOption>("CustomizeOption");
}

// ── DumpState: 54 fields, mix of required, #[serde(default)], and
// #[serde(default = "fn")] ──

#[test]
fn dump_state_full_key_round_trip() {
    assert_round_trip::<crate::client::DumpState>("DumpState_full");
}

#[test]
fn dump_state_missing_windows_required_field_is_err() {
    assert_de_err::<crate::client::DumpState>("DumpState_missing_windows");
}

/// Every `#[serde(default = "fn_name")]` field on `DumpState` must equal the
/// named function's return value when absent from the input (the minimal
/// sample carries only `layout` and `windows`, the two required fields).
#[test]
fn dump_state_default_fns_applied_when_absent() {
    let input = row("de-ok", "DumpState_minimal");
    let parsed: crate::client::DumpState = from_str(&input).expect("must parse minimal DumpState");
    assert_eq!(parsed.base_index, 1, "default_base_index");
    assert_eq!(parsed.status_left_length, 10, "default_status_left_length");
    assert_eq!(parsed.status_right_length, 40, "default_status_right_length");
    assert_eq!(parsed.status_lines, 1, "default_status_lines");
    assert!(parsed.status_visible, "default_status_visible");
    assert_eq!(parsed.repeat_time, 500, "default_repeat_time");
    assert!(parsed.bold_is_bright, "default_bold_is_bright");
    assert!(parsed.paste_detection, "default_paste_detection");
    assert!(parsed.mouse_selection, "default_mouse_selection");
    assert!(parsed.scroll_enter_copy_mode, "default_scroll_enter_copy_mode");
    // default_prediction_dimming delegates to dim_predictions_enabled();
    // just assert the field deserializes without requiring the key, its
    // exact value is an environment concern out of scope for this test.
    let _ = parsed.prediction_dimming;
}

/// A representative sweep of plain `#[serde(default)]` fields (Option<T>,
/// bool, usize, Vec<T>) absent from the minimal sample.
#[test]
fn dump_state_plain_defaults_applied_when_absent() {
    let input = row("de-ok", "DumpState_minimal");
    let parsed: crate::client::DumpState = from_str(&input).expect("must parse minimal DumpState");
    assert_eq!(parsed.prefix, None);
    assert_eq!(parsed.tree.len(), 0);
    assert_eq!(parsed.status_style, None);
    assert_eq!(parsed.copy_hsize, 0);
    assert_eq!(parsed.clock_mode, false);
    assert_eq!(parsed.bindings.len(), 0);
    assert_eq!(parsed.defaults_suppressed, false);
    assert_eq!(parsed.status_format.len(), 0);
    assert_eq!(parsed.cursor_style_code, None);
    assert_eq!(parsed.bell, false);
    assert_eq!(parsed.zoomed, false);
    assert_eq!(parsed.popup_active, false);
    assert_eq!(parsed.floats.len(), 0);
    assert_eq!(parsed.confirm_active, false);
    assert_eq!(parsed.menu_items.len(), 0);
    assert_eq!(parsed.display_panes, false);
    assert_eq!(parsed.pane_base_index, 0);
    assert_eq!(parsed.customize_options.len(), 0);
}

// ── tests-rs/test_client.rs local `Partial` (declared inline in that test
// file per ZDEP-015; exercised here directly against the fixture rows) ──

#[test]
fn partial_status_format_default_when_absent() {
    #[derive(Default)]
    struct Partial { status_format: Vec<String> }
    impl FromJson for Partial {
        fn from_json(v: &psmux_json::Value) -> Result<Self, psmux_json::Error> {
            let status_format = match v.get("status_format") {
                Some(sf) => Vec::<String>::from_json(sf)?,
                None => Vec::new(),
            };
            Ok(Partial { status_format })
        }
    }
    let input = row("de-ok", "Partial_minimal");
    let parsed: Partial = from_str(&input).expect("must parse empty object");
    assert!(parsed.status_format.is_empty());

    let input_full = row("de-ok", "Partial_full");
    let parsed_full: Partial = from_str(&input_full).expect("must parse status_format array");
    assert_eq!(parsed_full.status_format.len(), 2);
    assert_eq!(parsed_full.status_format[1], "#[fg=red]Hello");
}
