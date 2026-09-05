//! JSON payload types for `src/client.rs`, split into their own file to
//! respect the file-structure line-count gate. Re-exported at
//! `crate::client::{FloatJson, WinStatus, BindingEntry, ServerMenuItem,
//! CustomizeOption}` (kept there since `tests-rs/test_zdep_json_types.rs`
//! names them by that path). `DumpState` -- which embeds several of these --
//! lives in the sibling file `src/dump_state_json.rs` to stay under the same
//! gate.
//!
//! ZDEP-015: `WinStatus`, `BindingEntry`, `ServerMenuItem`, and
//! `CustomizeOption` used to be declared *inside* a function body in
//! client.rs; hoisted here to module scope (mirroring `FloatJson`) so
//! they're nameable from outside the crate's render loop, with hand-written
//! `ToJson`/`FromJson` impls replacing the old
//! `#[derive(serde::Deserialize)]` + `#[serde(...)]` attributes.

use psmux_json::{Error, FromJson, ToJson, Value};

use crate::json_util::{def, req};
use crate::layout::RowRunsJson;

/// A floating pane (tmux new-pane) as shipped from the server: position, size,
/// border style, focus, title, and the pane's rendered rows.
#[derive(Clone, Default)]
pub(crate) struct FloatJson {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    pub border: String,
    pub focused: bool,
    pub title: String,
    pub rows: Vec<RowRunsJson>,
}

impl ToJson for FloatJson {
    fn to_json(&self) -> Value {
        Value::Object(vec![
            ("x".into(), self.x.to_json()),
            ("y".into(), self.y.to_json()),
            ("w".into(), self.w.to_json()),
            ("h".into(), self.h.to_json()),
            ("border".into(), self.border.to_json()),
            ("focused".into(), self.focused.to_json()),
            ("title".into(), self.title.to_json()),
            ("rows".into(), self.rows.to_json()),
        ])
    }
}

impl FromJson for FloatJson {
    fn from_json(v: &Value) -> Result<Self, Error> {
        Ok(FloatJson {
            x: def(v, "x")?,
            y: def(v, "y")?,
            w: def(v, "w")?,
            h: def(v, "h")?,
            border: def(v, "border")?,
            focused: def(v, "focused")?,
            title: def(v, "title")?,
            rows: def(v, "rows")?,
        })
    }
}

#[derive(Default)]
pub(crate) struct WinStatus {
    pub id: usize,
    pub name: String,
    pub active: bool,
    pub activity: bool,
    pub bell: bool,
    pub last: bool,
    pub tab_text: String,
    pub idx: usize,
}

impl FromJson for WinStatus {
    fn from_json(v: &Value) -> Result<Self, Error> {
        Ok(WinStatus {
            id: req(v, "id")?,
            name: req(v, "name")?,
            active: req(v, "active")?,
            activity: def(v, "activity")?,
            bell: def(v, "bell")?,
            last: def(v, "last")?,
            tab_text: def(v, "tab_text")?,
            idx: def(v, "idx")?,
        })
    }
}

impl ToJson for WinStatus {
    fn to_json(&self) -> Value {
        Value::Object(vec![
            ("id".into(), self.id.to_json()),
            ("name".into(), self.name.to_json()),
            ("active".into(), self.active.to_json()),
            ("activity".into(), self.activity.to_json()),
            ("bell".into(), self.bell.to_json()),
            ("last".into(), self.last.to_json()),
            ("tab_text".into(), self.tab_text.to_json()),
            ("idx".into(), self.idx.to_json()),
        ])
    }
}

/// A single key binding synced from the server.
#[derive(Clone, Debug)]
pub(crate) struct BindingEntry {
    /// Key table name (e.g. "prefix", "root")
    pub t: String,
    /// Key string (e.g. "C-a", "-", "F12")
    pub k: String,
    /// Command string (e.g. "split-window -v")
    pub c: String,
    /// Whether the binding is repeatable
    pub r: bool,
}

impl ToJson for BindingEntry {
    fn to_json(&self) -> Value {
        Value::Object(vec![
            ("t".into(), self.t.to_json()),
            ("k".into(), self.k.to_json()),
            ("c".into(), self.c.to_json()),
            ("r".into(), self.r.to_json()),
        ])
    }
}

impl FromJson for BindingEntry {
    fn from_json(v: &Value) -> Result<Self, Error> {
        Ok(BindingEntry {
            t: req(v, "t")?,
            k: req(v, "k")?,
            c: req(v, "c")?,
            r: def(v, "r")?,
        })
    }
}

/// A menu item from server-side MenuMode
#[derive(Clone, Debug, Default)]
pub(crate) struct ServerMenuItem {
    pub name: Option<String>,
    pub key: Option<String>,
    pub sep: bool,
}

impl ToJson for ServerMenuItem {
    fn to_json(&self) -> Value {
        Value::Object(vec![
            ("name".into(), self.name.to_json()),
            ("key".into(), self.key.to_json()),
            ("sep".into(), self.sep.to_json()),
        ])
    }
}

impl FromJson for ServerMenuItem {
    fn from_json(v: &Value) -> Result<Self, Error> {
        Ok(ServerMenuItem {
            name: def(v, "name")?,
            key: def(v, "key")?,
            sep: def(v, "sep")?,
        })
    }
}

/// A customize-mode option row from server
#[derive(Clone, Debug, Default)]
pub(crate) struct CustomizeOption {
    /// Original index in the full options list
    pub i: usize,
    /// Option name
    pub n: String,
    /// Current value
    pub v: String,
    /// Scope (server/session/window/pane)
    pub s: String,
}

impl ToJson for CustomizeOption {
    fn to_json(&self) -> Value {
        Value::Object(vec![
            ("i".into(), self.i.to_json()),
            ("n".into(), self.n.to_json()),
            ("v".into(), self.v.to_json()),
            ("s".into(), self.s.to_json()),
        ])
    }
}

impl FromJson for CustomizeOption {
    fn from_json(v: &Value) -> Result<Self, Error> {
        Ok(CustomizeOption {
            i: req(v, "i")?,
            n: req(v, "n")?,
            v: req(v, "v")?,
            s: req(v, "s")?,
        })
    }
}
