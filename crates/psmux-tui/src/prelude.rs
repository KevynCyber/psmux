//! Mirrors `ratatui::prelude::*`'s re-export surface (ZDEP-044), restricted
//! to what this crate actually has, so every `use ratatui::prelude::*;` /
//! `use ratatui::widgets::*;` glob-import site listed in the S8c plan entry
//! (`src/client.rs:6-7`, `src/input.rs:6`, `src/popup.rs:543-544`,
//! `src/rendering.rs:8-9`, `src/style.rs:7`, `src/tree.rs:2`,
//! `src/window_ops.rs:5`, `examples/pipeline_diag.rs:13`) is a one-line
//! `ratatui::` -> `psmux_tui::` swap at flip time. `HorizontalAlignment`/
//! `VerticalAlignment`/`Margin`/`Masked`/`Stylize`/`BlockExt`/the backend
//! trait-conversion helpers (`FromCrossterm` etc.) are upstream-prelude
//! members this crate never ported (see each module's own doc for why) and
//! are therefore not re-exported here.

pub use crate::backend::{self, Backend};
pub use crate::buffer::{self, Buffer};
pub use crate::layout::{self, Alignment, Constraint, Direction, Layout, Position, Rect, Size};
pub use crate::style::{self, Color, Modifier, Style};
pub use crate::symbols;
pub use crate::text::{self, Line, Span, Text};
pub use crate::widgets::{StatefulWidget, Widget};
pub use crate::{CompletedFrame, Frame, Terminal};
