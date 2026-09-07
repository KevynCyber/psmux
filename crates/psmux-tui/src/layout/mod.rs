//! Ported from `ratatui-core` 0.1.2 `src/layout.rs` + `src/layout/*.rs`
//! (ZDEP-028). Reduced to the subset the 2026-09-06 project inventory found
//! in use or explicitly requested: `Rect`, `Position`, `Size`, `Alignment`
//! (`Left`/`Center` only), and a hand-rolled solver for
//! `Layout` + `Direction` + `Constraint::{Length, Min, Percentage}`.
//!
//! Deliberately NOT ported: `Rect::{intersection, union, offset, area, rows,
//! columns, clamp}`, `Constraint::{Ratio, Fill, Max}`, `Flex`, and the full
//! `kasuari` cassowary-style constraint solver -- upstream pulls in `lru`
//! for a layout cache around that solver, which is droppable once the
//! solver itself is replaced by a direct algorithm (see `layout::split`).

mod alignment;
mod constraint;
mod direction;
mod position;
mod rect;
mod size;
mod split;

pub use alignment::Alignment;
pub use constraint::Constraint;
pub use direction::Direction;
pub use position::Position;
pub use rect::Rect;
pub use size::Size;
pub use split::Layout;
