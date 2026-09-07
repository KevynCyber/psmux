//! Ported from `ratatui-core` 0.1.2 `src/layout/alignment.rs` (ZDEP-028),
//! reduced to `Left`/`Center` -- `Right` is not used anywhere in this repo
//! per the 2026-09-06 inventory. `strum`'s `Display`/`EnumString` derives
//! are dropped: nothing parses or prints an `Alignment`.

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Alignment {
    #[default]
    Left,
    Center,
}
