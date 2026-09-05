//! JSON payload types for `src/layout.rs`, split into their own file to
//! respect the file-structure line-count gate. Re-exported at
//! `crate::layout::{CellJson, CellRunJson, RowRunsJson, LayoutJson}` (kept
//! there since `tests-rs/test_zdep_json_types.rs` names them by that path).
//!
//! ZDEP-015: hand-written `ToJson`/`FromJson` impls replacing the old
//! `#[derive(Serialize, Deserialize)]` + `#[serde(...)]` attributes, with the
//! same field-order/required/default/rename semantics as before.

use psmux_json::{Error, FromJson, ToJson, Value};

use crate::json_util::{def, opt, req};

#[derive(Clone)]
pub struct CellJson {
    pub text: String,
    pub fg: String,
    pub bg: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    pub dim: bool,
    pub blink: bool,
    pub hidden: bool,
    pub strikethrough: bool,
}

impl ToJson for CellJson {
    fn to_json(&self) -> Value {
        Value::Object(vec![
            ("text".into(), self.text.to_json()),
            ("fg".into(), self.fg.to_json()),
            ("bg".into(), self.bg.to_json()),
            ("bold".into(), self.bold.to_json()),
            ("italic".into(), self.italic.to_json()),
            ("underline".into(), self.underline.to_json()),
            ("inverse".into(), self.inverse.to_json()),
            ("dim".into(), self.dim.to_json()),
            ("blink".into(), self.blink.to_json()),
            ("hidden".into(), self.hidden.to_json()),
            ("strikethrough".into(), self.strikethrough.to_json()),
        ])
    }
}

impl FromJson for CellJson {
    fn from_json(v: &Value) -> Result<Self, Error> {
        Ok(CellJson {
            text: req(v, "text")?,
            fg: req(v, "fg")?,
            bg: req(v, "bg")?,
            bold: req(v, "bold")?,
            italic: req(v, "italic")?,
            underline: req(v, "underline")?,
            inverse: req(v, "inverse")?,
            dim: req(v, "dim")?,
            blink: req(v, "blink")?,
            hidden: req(v, "hidden")?,
            strikethrough: req(v, "strikethrough")?,
        })
    }
}

#[derive(Clone)]
pub struct CellRunJson {
    pub text: String,
    pub fg: String,
    pub bg: String,
    pub flags: u8,
    pub width: u16,
    /// OSC 8 hyperlink URI for this run, if any (#361). Omitted from the JSON
    /// when absent — links are rare, so the per-frame payload is unchanged for
    /// normal output. The client re-emits OSC 8 around runs that carry it.
    pub link: Option<String>,
}

impl ToJson for CellRunJson {
    fn to_json(&self) -> Value {
        let mut fields = vec![
            ("text".into(), self.text.to_json()),
            ("fg".into(), self.fg.to_json()),
            ("bg".into(), self.bg.to_json()),
            ("flags".into(), self.flags.to_json()),
            ("width".into(), self.width.to_json()),
        ];
        // Skipped entirely when `None`, matching the old
        // `skip_serializing_if = "Option::is_none"` (#361).
        if let Some(link) = &self.link {
            fields.push(("link".into(), link.to_json()));
        }
        Value::Object(fields)
    }
}

impl FromJson for CellRunJson {
    fn from_json(v: &Value) -> Result<Self, Error> {
        Ok(CellRunJson {
            text: req(v, "text")?,
            fg: req(v, "fg")?,
            bg: req(v, "bg")?,
            flags: req(v, "flags")?,
            width: req(v, "width")?,
            link: opt(v, "link")?,
        })
    }
}

#[derive(Clone)]
pub struct RowRunsJson {
    pub runs: Vec<CellRunJson>,
}

impl ToJson for RowRunsJson {
    fn to_json(&self) -> Value {
        Value::Object(vec![("runs".into(), self.runs.to_json())])
    }
}

impl FromJson for RowRunsJson {
    fn from_json(v: &Value) -> Result<Self, Error> {
        Ok(RowRunsJson { runs: req(v, "runs")? })
    }
}

#[derive(Clone)]
pub enum LayoutJson {
    Split { kind: String, sizes: Vec<u16>, children: Vec<LayoutJson> },
    Leaf {
        id: usize,
        rows: u16,
        cols: u16,
        cursor_row: u16,
        cursor_col: u16,
        /// True when the pane's app EXPLICITLY enabled a mouse protocol
        /// (DECSET 1000/1002/1003).  Strict on purpose — no alt-screen or
        /// fullscreen heuristic — so the client only yields its drag
        /// selection to apps that really consume mouse events; alt-screen
        /// apps without mouse support (e.g. `less`) keep psmux selection.
        alternate_screen: bool,
        wants_mouse: bool,
        hide_cursor: bool,
        cursor_shape: u8,
        active: bool,
        copy_mode: bool,
        scroll_offset: usize,
        sel_start_row: Option<u16>,
        sel_start_col: Option<u16>,
        sel_end_row: Option<u16>,
        sel_end_col: Option<u16>,
        sel_mode: Option<String>,
        copy_cursor_row: Option<u16>,
        copy_cursor_col: Option<u16>,
        content: Vec<Vec<CellJson>>,
        rows_v2: Vec<RowRunsJson>,
        /// Pane title for border label expansion
        title: Option<String>,
    },
}

impl LayoutJson {
    /// Counts the total number of leaf panes in this layout tree.
    pub fn count_leaves(&self) -> usize {
        match self {
            LayoutJson::Leaf { .. } => 1,
            LayoutJson::Split { children, .. } => children.iter().map(|c| c.count_leaves()).sum(),
        }
    }
}

impl ToJson for LayoutJson {
    fn to_json(&self) -> Value {
        match self {
            LayoutJson::Split { kind, sizes, children } => Value::Object(vec![
                ("type".into(), Value::String("split".into())),
                ("kind".into(), kind.to_json()),
                ("sizes".into(), sizes.to_json()),
                ("children".into(), children.to_json()),
            ]),
            LayoutJson::Leaf {
                id, rows, cols, cursor_row, cursor_col, alternate_screen, wants_mouse,
                hide_cursor, cursor_shape, active, copy_mode, scroll_offset,
                sel_start_row, sel_start_col, sel_end_row, sel_end_col, sel_mode,
                copy_cursor_row, copy_cursor_col, content, rows_v2, title,
            } => Value::Object(vec![
                ("type".into(), Value::String("leaf".into())),
                ("id".into(), id.to_json()),
                ("rows".into(), rows.to_json()),
                ("cols".into(), cols.to_json()),
                ("cursor_row".into(), cursor_row.to_json()),
                ("cursor_col".into(), cursor_col.to_json()),
                ("alternate_screen".into(), alternate_screen.to_json()),
                ("wants_mouse".into(), wants_mouse.to_json()),
                ("hide_cursor".into(), hide_cursor.to_json()),
                ("cursor_shape".into(), cursor_shape.to_json()),
                ("active".into(), active.to_json()),
                ("copy_mode".into(), copy_mode.to_json()),
                ("scroll_offset".into(), scroll_offset.to_json()),
                ("sel_start_row".into(), sel_start_row.to_json()),
                ("sel_start_col".into(), sel_start_col.to_json()),
                ("sel_end_row".into(), sel_end_row.to_json()),
                ("sel_end_col".into(), sel_end_col.to_json()),
                ("sel_mode".into(), sel_mode.to_json()),
                ("copy_cursor_row".into(), copy_cursor_row.to_json()),
                ("copy_cursor_col".into(), copy_cursor_col.to_json()),
                ("content".into(), content.to_json()),
                ("rows_v2".into(), rows_v2.to_json()),
                ("title".into(), title.to_json()),
            ]),
        }
    }
}

impl FromJson for LayoutJson {
    fn from_json(v: &Value) -> Result<Self, Error> {
        let tag: String = req(v, "type")?;
        match tag.as_str() {
            "split" => Ok(LayoutJson::Split {
                kind: req(v, "kind")?,
                sizes: req(v, "sizes")?,
                children: req(v, "children")?,
            }),
            "leaf" => Ok(LayoutJson::Leaf {
                id: req(v, "id")?,
                rows: req(v, "rows")?,
                cols: req(v, "cols")?,
                cursor_row: req(v, "cursor_row")?,
                cursor_col: req(v, "cursor_col")?,
                alternate_screen: def(v, "alternate_screen")?,
                wants_mouse: def(v, "wants_mouse")?,
                hide_cursor: def(v, "hide_cursor")?,
                cursor_shape: def(v, "cursor_shape")?,
                active: req(v, "active")?,
                copy_mode: req(v, "copy_mode")?,
                scroll_offset: req(v, "scroll_offset")?,
                sel_start_row: opt(v, "sel_start_row")?,
                sel_start_col: opt(v, "sel_start_col")?,
                sel_end_row: opt(v, "sel_end_row")?,
                sel_end_col: opt(v, "sel_end_col")?,
                sel_mode: opt(v, "sel_mode")?,
                copy_cursor_row: opt(v, "copy_cursor_row")?,
                copy_cursor_col: opt(v, "copy_cursor_col")?,
                content: def(v, "content")?,
                rows_v2: def(v, "rows_v2")?,
                title: opt(v, "title")?,
            }),
            _ => Err(Error::new(format!("unknown LayoutJson tag {tag:?}"))),
        }
    }
}
