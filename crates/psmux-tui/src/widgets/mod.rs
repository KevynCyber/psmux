//! ZDEP-033..039: in-tree, dependency-free port of `ratatui-widgets` 0.3.2
//! (plus the `Widget`/`StatefulWidget` traits from `ratatui-core` 0.1.2) --
//! the widget surface actually used by this repo, per the 2026-09-06
//! project-wide inventory in `docs/plans/2026-09-04-zero-third-party-deps.md`
//! slice S8b. See each module's own doc comment for exactly what was
//! dropped from the reduced surface and why.
//!
//! Upstream sources (MIT, same licence family as this crate):
//!   `C:/Users/Kev/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ratatui-widgets-0.3.2/src/`
//!   `C:/Users/Kev/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ratatui-core-0.1.2/src/widgets/`
//!
//! S8b is ADDITIVE AND INERT, same invariant as S8a: `ratatui` stays in
//! both the root and `tests/monitor` manifests and no call site is flipped
//! (that is S8c). Deps stripped and how each was replaced (none of them
//! appear in this crate's `Cargo.toml`):
//! - `bitflags` -- `borders::Borders` hand-rolls a `u8`-backed single
//!   constant (`ALL`) rather than a full bitflag type (no call site
//!   composes flags).
//! - `strum`, `instability` -- unused: nothing parses/prints a `BorderType`,
//!   and no `#[unstable]`-gated API is ported.
//! - `itertools`, `unicode-segmentation`, `unicode-truncate` -- pulled only
//!   by upstream's grapheme-aware wrapping/title-merging, which this
//!   reduced surface doesn't need (`Wrap`/multi-title/`merge_borders` are
//!   all unused per the inventory). Per-char width instead comes from
//!   `psmux-unicode` (`render_text.rs`), already the root crate's own
//!   `unicode-width` replacement.
//! - `compact_str` -- not pulled by the widgets this repo uses (`Bar`'s
//!   `text_value` is a plain `String`, matching upstream's own field type).

mod barchart;
mod block;
mod borders;
mod clear;
mod gauge;
mod list;
mod paragraph;
mod render_text;
mod stateful_widget;
mod widget;

pub use barchart::{Bar, BarChart, BarGroup};
pub use block::Block;
pub use borders::{BorderType, Borders};
pub use clear::Clear;
pub use gauge::Gauge;
pub use list::{List, ListItem, ListState};
pub use paragraph::Paragraph;
pub use stateful_widget::StatefulWidget;
pub use widget::Widget;
