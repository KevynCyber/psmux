//! Ported from `ratatui-core` 0.1.2 `src/layout/constraint.rs` (ZDEP-028),
//! reduced to the three variants the plan scopes in: `Length`, `Min`,
//! `Percentage`. `Ratio`/`Fill`/`Max` and the `strum::EnumIs` derive are not
//! ported -- see `layout::split` for the direct algorithm replacing
//! upstream's `kasuari` solve over this reduced variant set.

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Constraint {
    /// Applies a minimum size: the element gets at least this many cells,
    /// growing to absorb any space left over after `Length`/`Percentage`.
    Min(u16),
    /// A fixed size in cells.
    Length(u16),
    /// A percentage of the total space being divided.
    Percentage(u16),
}
