//! JSON payload types for `src/util.rs`, split into their own file to
//! respect the file-structure line-count gate. Re-exported at
//! `crate::util::{WinInfo, PaneInfo, WinTree, LayoutSimple}` (kept there
//! since `tests-rs/test_zdep_json_types.rs` names them by that path).
//!
//! ZDEP-015: hand-written `ToJson`/`FromJson` impls replacing the old
//! `#[derive(Serialize, Deserialize)]` + `#[serde(...)]` attributes.

use psmux_json::{Error, FromJson, ToJson, Value};

use crate::json_util::{def, req};

pub struct WinInfo {
    pub id: usize,
    pub name: String,
    pub active: bool,
    pub activity: bool,
    pub bell: bool,
    pub last: bool,
    pub tab_text: String,
    pub idx: usize,
}

impl ToJson for WinInfo {
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

impl FromJson for WinInfo {
    fn from_json(v: &Value) -> Result<Self, Error> {
        Ok(WinInfo {
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

pub struct PaneInfo {
    pub id: usize,
    pub title: String,
}

impl ToJson for PaneInfo {
    fn to_json(&self) -> Value {
        Value::Object(vec![("id".into(), self.id.to_json()), ("title".into(), self.title.to_json())])
    }
}

impl FromJson for PaneInfo {
    fn from_json(v: &Value) -> Result<Self, Error> {
        Ok(PaneInfo { id: req(v, "id")?, title: req(v, "title")? })
    }
}

pub struct WinTree {
    pub id: usize,
    pub name: String,
    pub active: bool,
    pub panes: Vec<PaneInfo>,
    pub idx: usize,
}

impl ToJson for WinTree {
    fn to_json(&self) -> Value {
        Value::Object(vec![
            ("id".into(), self.id.to_json()),
            ("name".into(), self.name.to_json()),
            ("active".into(), self.active.to_json()),
            ("panes".into(), self.panes.to_json()),
            ("idx".into(), self.idx.to_json()),
        ])
    }
}

impl FromJson for WinTree {
    fn from_json(v: &Value) -> Result<Self, Error> {
        Ok(WinTree {
            id: req(v, "id")?,
            name: req(v, "name")?,
            active: req(v, "active")?,
            panes: req(v, "panes")?,
            idx: def(v, "idx")?,
        })
    }
}

/// Lightweight layout description for cross-session preview rendering
/// (issue #257). Mirrors the structural part of `LayoutJson` without any
/// pane content. Uses the same `type` discriminant so it deserializes
/// alongside the heavier dump-state layout.
#[derive(Clone, Debug)]
pub enum LayoutSimple {
    Split { kind: String, sizes: Vec<u16>, children: Vec<LayoutSimple> },
    Leaf { id: usize, active: bool },
}

impl ToJson for LayoutSimple {
    fn to_json(&self) -> Value {
        match self {
            LayoutSimple::Split { kind, sizes, children } => Value::Object(vec![
                ("type".into(), Value::String("split".into())),
                ("kind".into(), kind.to_json()),
                ("sizes".into(), sizes.to_json()),
                ("children".into(), children.to_json()),
            ]),
            LayoutSimple::Leaf { id, active } => Value::Object(vec![
                ("type".into(), Value::String("leaf".into())),
                ("id".into(), id.to_json()),
                ("active".into(), active.to_json()),
            ]),
        }
    }
}

impl FromJson for LayoutSimple {
    fn from_json(v: &Value) -> Result<Self, Error> {
        let tag: String = req(v, "type")?;
        match tag.as_str() {
            "split" => Ok(LayoutSimple::Split {
                kind: req(v, "kind")?,
                sizes: req(v, "sizes")?,
                children: req(v, "children")?,
            }),
            "leaf" => Ok(LayoutSimple::Leaf { id: req(v, "id")?, active: def(v, "active")? }),
            _ => Err(Error::new(format!("unknown LayoutSimple tag {tag:?}"))),
        }
    }
}
