//! Native terminal I/O for psmux (ZDEP-025..027): event types shape-identical
//! to crossterm 0.29, native Win32 console I/O, and a ratatui `Backend` impl
//! byte-identical to ratatui-crossterm 0.1.2's draw path. Replaces the
//! `crossterm` crate and `ratatui-crossterm`.
//!
//! INVARIANT: no crate-rooted paths anywhere under `src/term/` -- examples and
//! `tests/monitor` `#[path]`-include this module tree into their own binary
//! crate (same trick `src/pty` uses for `examples/latency_harness.rs`), so a
//! crate-rooted path here would resolve to the wrong crate root there
//! (`$crate` in the macros below is fine: it resolves to whichever crate
//! includes this tree, and every includer mounts it as `term`).
//!
//! This tree is compiled once per binary that includes it (the psmux/pmux/
//! tmux bins, plus each diagnostic example and tests/monitor, all `mod
//! term;`/`#[path]`-include a fresh copy) -- a small example using only a
//! slice of the API surface makes the rest look dead in ITS compilation, so
//! dead-code/unused-import lints are suppressed here (mirrors main.rs's own
//! top-level `#![allow(dead_code)]` for the same multi-binary reason).
#![allow(dead_code, unused_imports)]

pub mod backend;
#[cfg(windows)]
pub mod console;
pub mod cursor;
pub mod event;
pub mod style;
pub mod terminal;

/// A command whose effect is an ANSI escape sequence written to a
/// `fmt::Write` sink (mirrors crossterm's `Command` trait).
pub trait Command {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result;
}

/// Write one or more [`Command`]s to an `io::Write` sink without flushing.
///
/// Method-call syntax (`$writer.write_all(...)`), not `&mut $writer`: call
/// sites pass either an owned writer (needs a `mut` binding, which they
/// already have) or an already-`&mut` reference (e.g. `terminal.backend_mut()`)
/// whose binding is NOT `mut` -- an explicit `&mut $writer` would fail to
/// borrow that case. Method syntax reborrows the pointee either way. The
/// `#[allow]`'d import brings `Write` into scope for callers that don't
/// already import it themselves (harmlessly redundant -- hence unused -- for
/// the ones that do).
#[macro_export]
macro_rules! queue {
    ($writer:expr $(, $command:expr)* $(,)?) => {{
        #[allow(unused_imports)]
        use ::std::io::Write as _;
        (|| -> ::std::io::Result<()> {
            let mut _psmux_term_buf = ::std::string::String::new();
            $(
                if let Err(_e) = $crate::term::Command::write_ansi(&$command, &mut _psmux_term_buf) {
                    return Err(::std::io::Error::new(::std::io::ErrorKind::Other, _e.to_string()));
                }
            )*
            $writer.write_all(_psmux_term_buf.as_bytes())
        })()
    }};
}

/// Write and flush one or more [`Command`]s to an `io::Write` sink.
#[macro_export]
macro_rules! execute {
    ($writer:expr $(, $command:expr)* $(,)?) => {{
        #[allow(unused_imports)]
        use ::std::io::Write as _;
        (|| -> ::std::io::Result<()> {
            $crate::queue!($writer $(, $command)*)?;
            $writer.flush()
        })()
    }};
}
