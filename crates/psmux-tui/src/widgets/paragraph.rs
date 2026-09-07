//! Ported from `ratatui-widgets` 0.3.2 `src/paragraph.rs` (ZDEP-035),
//! reduced to `new`/`.block`/`.scroll`/`.alignment`/`.style` -- `Wrap` is
//! never used anywhere in this repo's inventory, so this port drops word
//! wrapping entirely along with `reflow`'s `LineComposer`/`WordWrapper`/
//! `LineTruncator`: each line is truncated to the inner area's width
//! (never wrapped to the next line), matching upstream's own un-wrapped
//! (`self.wrap.is_none()`) code path.

use crate::buffer::Buffer;
use crate::layout::{Alignment, Position, Rect};
use crate::style::Style;
use crate::text::Text;
use crate::widgets::render_text::{fill_style, render_line};
use crate::widgets::{Block, Widget};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Paragraph<'a> {
    block: Option<Block<'a>>,
    style: Style,
    text: Text<'a>,
    scroll: Position,
    alignment: Alignment,
}

impl<'a> Paragraph<'a> {
    pub fn new<T: Into<Text<'a>>>(text: T) -> Self {
        let text: Text<'a> = text.into();
        let alignment = text.alignment.unwrap_or_default();
        Self { block: None, style: Style::default(), text, scroll: Position::default(), alignment }
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// `(y, x)` scroll offset, matching upstream's own (unconventional)
    /// tuple order.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn scroll(mut self, offset: (u16, u16)) -> Self {
        self.scroll = Position { x: offset.1, y: offset.0 };
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }
}

impl<'a> Widget for &Paragraph<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        fill_style(area, buf, self.style);
        if let Some(block) = &self.block {
            block.render(area, buf);
        }
        let inner = self.block.as_ref().map_or(area, |b| b.inner(area));
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        for (row, line) in self.text.lines.iter().skip(self.scroll.y as usize).enumerate() {
            if row as u16 >= inner.height {
                break;
            }
            let alignment = line.alignment.unwrap_or(self.alignment);
            render_line(line, inner, buf, row as u16, alignment, self.scroll.x);
        }
    }
}

impl<'a> Widget for Paragraph<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        (&self).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Color;
    use crate::text::Line;
    use crate::widgets::{BorderType, Borders};

    fn buf(w: u16, h: u16) -> Buffer {
        Buffer::empty(Rect::new(0, 0, w, h))
    }

    // Covers: ZDEP-035
    #[test]
    fn new_from_str_renders_left_aligned() {
        let mut b = buf(10, 1);
        Paragraph::new("hi").render(b.area, &mut b);
        assert_eq!(b[(0, 0)].symbol(), "h");
        assert_eq!(b[(1, 0)].symbol(), "i");
    }

    // Covers: ZDEP-035
    #[test]
    fn new_from_vec_of_lines_renders_each_row() {
        let mut b = buf(5, 2);
        Paragraph::new(vec![Line::from("a"), Line::from("b")]).render(b.area, &mut b);
        assert_eq!(b[(0, 0)].symbol(), "a");
        assert_eq!(b[(0, 1)].symbol(), "b");
    }

    // Covers: ZDEP-035
    #[test]
    fn alignment_centers_text() {
        let mut b = buf(6, 1);
        Paragraph::new("ab").alignment(Alignment::Center).render(b.area, &mut b);
        assert_eq!(b[(2, 0)].symbol(), "a");
        assert_eq!(b[(3, 0)].symbol(), "b");
    }

    // Covers: ZDEP-035
    #[test]
    fn scroll_skips_leading_rows() {
        let mut b = buf(5, 1);
        Paragraph::new(vec![Line::from("first"), Line::from("second")]).scroll((1, 0)).render(b.area, &mut b);
        assert_eq!(b[(0, 0)].symbol(), "s");
    }

    // Covers: ZDEP-035
    #[test]
    fn block_reserves_inner_area() {
        let mut b = buf(5, 3);
        let block = Block::default().borders(Borders::ALL).border_type(BorderType::Plain);
        Paragraph::new("x").block(block).render(b.area, &mut b);
        assert_eq!(b[(1, 1)].symbol(), "x");
        assert_eq!(b[(0, 0)].symbol(), "\u{250C}");
    }

    // Covers: ZDEP-035
    #[test]
    fn style_paints_whole_area_background() {
        let mut b = buf(3, 1);
        Paragraph::new("x").style(Style::default().bg(Color::Blue)).render(b.area, &mut b);
        assert_eq!(b[(2, 0)].bg, Color::Blue);
    }
}
