//! ZDEP-028..032: in-tree, dependency-free port of `ratatui-core` 0.1.2's
//! CORE layer (layout, style, text, buffer, backend trait) -- the surface
//! actually used by this repo, per the 2026-09-06 project-wide inventory in
//! `docs/plans/2026-09-04-zero-third-party-deps.md` slice S8a.
//!
//! ZDEP-033..039 (slice S8b) add the WIDGET layer on top: `symbols::border`,
//! the `Widget`/`StatefulWidget` traits, and `Block`/`Paragraph`/`Clear`/
//! `Gauge`/`List`/`BarChart` -- see `widgets/mod.rs` for the full list and
//! what each widget's reduced surface deliberately drops.
//!
//! Upstream sources (MIT, same licence family as this crate):
//!   `C:/Users/Kev/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ratatui-core-0.1.2/src/`
//!   `C:/Users/Kev/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ratatui-widgets-0.3.2/src/`
//!
//! ZDEP-040..044 (slice S8c, additive half) add the last layer: a
//! `Layout::default().direction(...).constraints(...)` builder (matching
//! every real call site's shape, not just `Layout::new`), `Rect::from(Size)`,
//! `Buffer::{resize,reset,diff}`, `backend::TestBackend`, and a
//! fullscreen-only `terminal::{Terminal, Frame, CompletedFrame}` -- see
//! `terminal/mod.rs` for what's dropped (`Viewport`/`with_options`/inline
//! mode/`try_draw`/`insert_before`, none of which any call site uses).
//! `prelude` re-exports the flip's glob-import surface
//! (`ratatui::prelude::*` / `ratatui::widgets::*` sites, see
//! `docs/features/zero-deps-2.md`).
//!
//! S8a, S8b, and this additive half of S8c are ADDITIVE AND INERT: this
//! crate is still not wired into any call site. `ratatui` itself still stays
//! in the root and `tests/monitor` manifests. Flipping the ~130 real call
//! sites in `src/`, `examples/`, `tests-rs/`, and `tests/monitor/` off
//! `ratatui` needs `ratatui::` -> `psmux_tui::` path edits inside ~56
//! `tests-rs/*.rs` files and `tests/monitor/src/*.rs` -- both gated to
//! test-engineer by `agent-write-gate`/HOOKS-129's path classification
//! (verified live: a one-line probe `Edit` to `tests-rs/test_zoom_bleed.rs`
//! and to `tests/monitor/src/ui.rs` were both denied with "test-engineer owns
//! all test writes"), so the flip itself is a separate dispatch.
//!
//! Upstream deps stripped and how each was replaced (none of them appear in
//! this crate's `Cargo.toml`):
//! - `bitflags` -- `style::modifier` hand-rolls a `u16`-backed flag type.
//! - `compact_str` -- `buffer::cell::Cell` stores its symbol as a plain
//!   `String` (small-string inlining is a perf optimization upstream, not a
//!   behavioural requirement of the reduced surface).
//! - `strum` -- `Direction`/`Alignment`/`ClearType` drop the `Display`/
//!   `EnumString` derives; nothing in the reduced surface parses or prints
//!   them.
//! - `hashbrown`, `itertools`, `kasuari`, `lru`, `thiserror`,
//!   `unicode-segmentation`, `unicode-truncate`, `unicode-width` -- pulled
//!   only by upstream surface this port deliberately excludes (the full
//!   kasuari constraint solver, widgets, grapheme-aware wrapping/rendering).
//!   `unicode-width` is separately already replaced in this repo by
//!   `crates/psmux-unicode`; this crate has no grapheme/width-sensitive
//!   surface yet, so it takes no dependency on it.

pub mod backend;
pub mod buffer;
pub mod layout;
pub mod prelude;
pub mod style;
pub mod symbols;
pub mod terminal;
pub mod text;
pub mod widgets;

pub use terminal::{CompletedFrame, Frame, Terminal};
