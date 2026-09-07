//! Ported from `ratatui-core` 0.1.2 `src/symbols.rs` + `src/symbols/{border,line}.rs`
//! (ZDEP-033), reduced to the one thing the widget layer's `Block` needs:
//! the four `BorderType`s actually used in this repo (Plain/Rounded/Double/
//! Thick). Upstream's `border::Set` is generic over a lifetime and derived
//! from a separate `line::Set` (11 fields, for table/box mid-lines this
//! repo never draws); this port collapses both into one 6-field `'static`
//! `Set` since nothing here needs a custom border set or a bare `line::Set`.
//! `symbols::merge` (border-collapsing) is not ported -- no call site joins
//! adjacent blocks' borders.

pub mod border;
