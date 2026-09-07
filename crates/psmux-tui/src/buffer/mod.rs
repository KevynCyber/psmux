//! Ported from `ratatui-core` 0.1.2 `src/buffer/{cell,buffer}.rs`
//! (ZDEP-031), reduced per the spec: no `set_string` (no widget in this
//! crate's reduced surface needs it directly; `render_text.rs` writes cells
//! one at a time). `resize`/`reset`/`diff` are added in ZDEP-042 (S8c) for
//! `Terminal`. `Cell` stores its symbol as a plain `String` rather than
//! upstream's `compact_str::CompactString` -- see `lib.rs` for why.

mod buffer;
mod cell;

pub use buffer::Buffer;
pub use cell::Cell;
