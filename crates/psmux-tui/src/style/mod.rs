//! Ported from `ratatui-core` 0.1.2 `src/style.rs` + `src/style/color.rs`
//! (ZDEP-029), reduced per the 2026-09-06 inventory: `Style` without
//! `.patch` (nothing in this repo merges styles incrementally outside the
//! VT backend's own diff, which reads fields directly), `Modifier` without
//! `.all()`, and no `Stylize` shorthand trait (never used in this repo).
//!
//! `Modifier` is upstream's own `bitflags!`-generated type; `modifier.rs`
//! hand-rolls the same bit layout without the `bitflags` crate.

mod color;
mod modifier;
mod style;

pub use color::Color;
pub use modifier::Modifier;
pub use style::Style;
