//! `VtBackend`: a ratatui `Backend` impl byte-identical to ratatui-crossterm
//! 0.1.2's `CrosstermBackend::draw` (ZDEP-027), built on this module's own
//! ANSI [`Command`]s instead of crossterm's.

use super::cursor::{Hide, MoveTo, Show};
use super::style::Print;
use super::Command;
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use ratatui::style::{Color, Modifier};
use std::io::{self, Write};

/// TUI backend over any `io::Write` sink. Same shape as
/// `ratatui::backend::CrosstermBackend<W>`.
pub struct VtBackend<W: Write> {
    writer: W,
}

impl<W: Write> VtBackend<W> {
    pub const fn new(writer: W) -> Self {
        Self { writer }
    }
    pub const fn writer(&self) -> &W {
        &self.writer
    }
    pub const fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }
}

impl<W: Write> Write for VtBackend<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

fn write_and_flush<W: Write>(writer: &mut W, s: &str) -> io::Result<()> {
    writer.write_all(s.as_bytes())?;
    writer.flush()
}

/// SGR color parameter (no leading `\x1b[` / trailing `m`): named colors as
/// 256-indexed `<base>;5;<n>`, `Rgb` as `<base>;2;r;g;b`, `Reset` as
/// `<base>+1` (39 fg / 49 bg / 59 underline) -- crossterm's `Colored`
/// encoding. `base` is 38 (fg), 48 (bg), or 58 (underline).
fn color_params(color: Color, base: u16) -> String {
    match color {
        Color::Reset => format!("{}", base + 1),
        Color::Black => format!("{};5;0", base),
        Color::Red => format!("{};5;1", base),
        Color::Green => format!("{};5;2", base),
        Color::Yellow => format!("{};5;3", base),
        Color::Blue => format!("{};5;4", base),
        Color::Magenta => format!("{};5;5", base),
        Color::Cyan => format!("{};5;6", base),
        Color::Gray => format!("{};5;7", base),
        Color::DarkGray => format!("{};5;8", base),
        Color::LightRed => format!("{};5;9", base),
        Color::LightGreen => format!("{};5;10", base),
        Color::LightYellow => format!("{};5;11", base),
        Color::LightBlue => format!("{};5;12", base),
        Color::LightMagenta => format!("{};5;13", base),
        Color::LightCyan => format!("{};5;14", base),
        Color::White => format!("{};5;15", base),
        Color::Indexed(i) => format!("{};5;{}", base, i),
        Color::Rgb(r, g, b) => format!("{};2;{};{};{}", base, r, g, b),
    }
}

fn sgr(buf: &mut String, code: u8) {
    use std::fmt::Write as _;
    let _ = write!(buf, "\x1b[{}m", code);
}

/// Ports ratatui-crossterm's `ModifierDiff::queue` verbatim (order matters:
/// it is what real terminals expect for combined bold/dim resets).
fn write_modifier_diff(buf: &mut String, from: Modifier, to: Modifier) {
    let removed = from - to;
    if removed.contains(Modifier::REVERSED) { sgr(buf, 27); }

    let reset_intensity = removed.contains(Modifier::BOLD) || removed.contains(Modifier::DIM);
    if reset_intensity {
        sgr(buf, 22);
        if to.contains(Modifier::DIM) { sgr(buf, 2); }
        if to.contains(Modifier::BOLD) { sgr(buf, 1); }
    }
    if removed.contains(Modifier::ITALIC) { sgr(buf, 23); }
    if removed.contains(Modifier::UNDERLINED) { sgr(buf, 24); }
    if removed.contains(Modifier::CROSSED_OUT) { sgr(buf, 29); }
    if removed.contains(Modifier::HIDDEN) { sgr(buf, 28); }
    if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) { sgr(buf, 25); }

    let added = to - from;
    if added.contains(Modifier::REVERSED) { sgr(buf, 7); }
    if added.contains(Modifier::BOLD) && !reset_intensity { sgr(buf, 1); }
    if added.contains(Modifier::ITALIC) { sgr(buf, 3); }
    if added.contains(Modifier::UNDERLINED) { sgr(buf, 4); }
    if added.contains(Modifier::DIM) && !reset_intensity { sgr(buf, 2); }
    if added.contains(Modifier::CROSSED_OUT) { sgr(buf, 9); }
    if added.contains(Modifier::HIDDEN) { sgr(buf, 8); }
    if added.contains(Modifier::SLOW_BLINK) { sgr(buf, 5); }
    if added.contains(Modifier::RAPID_BLINK) { sgr(buf, 6); }
}

impl<W: Write> Backend for VtBackend<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut buf = String::new();
        let mut fg = Color::Reset;
        let mut bg = Color::Reset;
        #[cfg(feature = "underline-color")]
        let mut underline_color = Color::Reset;
        let mut modifier = Modifier::empty();
        let mut last_pos: Option<(u16, u16)> = None;

        for (x, y, cell) in content {
            if !matches!(last_pos, Some((lx, ly)) if x == lx + 1 && y == ly) {
                let _ = Command::write_ansi(&MoveTo(x, y), &mut buf);
            }
            last_pos = Some((x, y));

            if cell.modifier != modifier {
                write_modifier_diff(&mut buf, modifier, cell.modifier);
                modifier = cell.modifier;
            }
            if cell.fg != fg || cell.bg != bg {
                // crossterm SetColors: one CSI, fg and bg params joined by ';'.
                buf.push_str("\x1b[");
                buf.push_str(&color_params(cell.fg, 38));
                buf.push(';');
                buf.push_str(&color_params(cell.bg, 48));
                buf.push('m');
                fg = cell.fg;
                bg = cell.bg;
            }
            #[cfg(feature = "underline-color")]
            if cell.underline_color != underline_color {
                buf.push_str("\x1b[");
                buf.push_str(&color_params(cell.underline_color, 58));
                buf.push('m');
                underline_color = cell.underline_color;
            }

            let _ = Command::write_ansi(&Print(cell.symbol()), &mut buf);
        }

        buf.push_str("\x1b[39m\x1b[49m");
        #[cfg(feature = "underline-color")]
        buf.push_str("\x1b[59m");
        buf.push_str("\x1b[0m");
        self.writer.write_all(buf.as_bytes())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        let mut s = String::new();
        let _ = Command::write_ansi(&Hide, &mut s);
        write_and_flush(&mut self.writer, &s)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        let mut s = String::new();
        let _ = Command::write_ansi(&Show, &mut s);
        write_and_flush(&mut self.writer, &s)
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        #[cfg(windows)]
        {
            let (x, y) = super::console::cursor_position()?;
            Ok(Position { x, y })
        }
        #[cfg(not(windows))]
        {
            Err(io::Error::new(io::ErrorKind::Unsupported, "psmux is Windows-only"))
        }
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let Position { x, y } = position.into();
        let mut s = String::new();
        let _ = Command::write_ansi(&MoveTo(x, y), &mut s);
        write_and_flush(&mut self.writer, &s)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.clear_region(ClearType::All)
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        let seq = match clear_type {
            ClearType::All => "\x1b[2J",
            ClearType::AfterCursor => "\x1b[J",
            ClearType::BeforeCursor => "\x1b[1J",
            ClearType::CurrentLine => "\x1b[2K",
            ClearType::UntilNewLine => "\x1b[K",
        };
        write_and_flush(&mut self.writer, seq)
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        for _ in 0..n {
            self.writer.write_all(b"\n")?;
        }
        self.writer.flush()
    }

    fn size(&self) -> io::Result<Size> {
        let (width, height) = super::terminal::size()?;
        Ok(Size::new(width, height))
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        super::terminal::window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
