//! Ported from `ratatui-core` 0.1.2 `src/style/color.rs` (ZDEP-029): the 16
//! named ANSI variants plus `Reset`/`Indexed`/`Rgb`. `FromStr` parsing and
//! `from_u32` are not ported -- nothing in this repo parses a `Color` from
//! text.

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Color {
    #[default]
    Reset,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    White,
    Rgb(u8, u8, u8),
    Indexed(u8),
}
