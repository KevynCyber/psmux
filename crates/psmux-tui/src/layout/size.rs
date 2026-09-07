//! Ported from `ratatui-core` 0.1.2 `src/layout/size.rs` (ZDEP-028): fields
//! and `new` only.

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

impl Size {
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-028
    #[test]
    fn new() {
        assert_eq!(Size::new(80, 24), Size { width: 80, height: 24 });
    }
}
