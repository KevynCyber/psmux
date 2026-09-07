//! Ported from `ratatui-core` 0.1.2 `src/layout/direction.rs` (ZDEP-028).
//! `strum`'s `Display`/`EnumString` derives are dropped along with
//! `perpendicular()`: neither is used by this repo's reduced surface.

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Direction {
    Horizontal,
    #[default]
    Vertical,
}
