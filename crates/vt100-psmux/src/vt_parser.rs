//! In-tree, dependency-free port of the `vte` crate (version 0.15.0, std
//! build, default features -- no `no_std`/`arrayvec` codepaths).
//!
//! `vte` is dual-licensed MIT/Apache-2.0, same licence family as this crate
//! (vt100-psmux is MIT). This module is a line-for-line behavioural port of
//! the parser state machine (Paul Williams' ANSI parser state machine) from:
//!
//!   `C:/Users/Kev/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vte-0.15.0/src/lib.rs`
//!   `C:/Users/Kev/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vte-0.15.0/src/params.rs`
//!
//! The `ansi` feature (vte's `ansi.rs`) is not ported: vt100-psmux depends on
//! `vte` with default features only (no `ansi`), so that module is dead code
//! for this crate's purposes.
//!
//! Split across files (mirroring the size limit this repo enforces, not
//! vte's own layout, which is a single `lib.rs`):
//! - `params.rs` -- the `Params`/`ParamsIter` port of vte's `params.rs`.
//! - `transitions.rs` -- the per-state `advance_*`/`anywhere` byte-dispatch
//!   table (private `impl Parser` methods).
//! - `actions.rs` -- the `action_*` helpers, OSC dispatch, ground-state and
//!   partial-UTF-8 handling (private `impl Parser` methods).
//!
//! Constants (must match vte 0.15.0 exactly for byte-identical behaviour):
//! - `MAX_PARAMS` = 32 (max CSI/DCS parameters + subparameters, combined;
//!   see `vt_parser::params`)
//! - `MAX_INTERMEDIATES` = 2
//! - `MAX_OSC_PARAMS` = 16 (extra OSC separators beyond this are folded into
//!   the last param's raw range, per vte's `action_osc_put_param`)
//! - OSC raw buffer: unbounded `Vec<u8>` (the std-build codepath; vte's
//!   `no_std` build uses a fixed-capacity `ArrayVec` instead, not ported here)
//! - CSI/DCS param accumulation: `u16`, saturating on multiply-by-10 and add
//!
//! One deliberate, behaviour-preserving deviation from vte's own source: the
//! OSC-dispatch step (in `actions.rs`) builds the `&[&[u8]]` param slice via
//! a plain `Vec` instead of vte's `MaybeUninit`-array-plus-unsafe-cast trick
//! (vte needs that trick to stay `no_std`-compatible without allocation;
//! this crate is std-only, so a `Vec` is simpler and equally exact).
//!
//! This module (and its `actions`/`transitions` children) is allowed a
//! handful of pedantic/nursery lints that the rest of this crate warns on:
//! `inline_always`, `match_same_arms`, `single_match_else`, `as_conversions`.
//! vte 0.15.0's own lint policy is just `#![deny(clippy::all,
//! clippy::if_not_else, clippy::enum_glob_use)]` -- none of pedantic/
//! nursery/cargo -- and every flagged spot here (the `#[inline(always)]`
//! hints, the per-byte-range match arms, the `byte as char`/`c as u8`
//! narrowing casts) is copied verbatim from vte for behavioural and
//! auditability parity; "fixing" them (splitting/merging match arms,
//! dropping inline hints) would only obscure the 1:1 correspondence with
//! the upstream source this module is ported from.
#![allow(
    clippy::inline_always,
    clippy::match_same_arms,
    clippy::single_match_else,
    clippy::as_conversions
)]

mod actions;
mod params;
mod transitions;
pub use params::{Params, ParamsIter};

#[cfg(test)]
mod tests;

const MAX_INTERMEDIATES: usize = 2;
const MAX_OSC_PARAMS: usize = 16;

/// Parser for raw VTE protocol which delegates actions to a [`Perform`].
#[derive(Default)]
pub struct Parser {
    state: State,
    intermediates: [u8; MAX_INTERMEDIATES],
    intermediate_idx: usize,
    params: Params,
    param: u16,
    osc_raw: Vec<u8>,
    osc_params: [(usize, usize); MAX_OSC_PARAMS],
    osc_num_params: usize,
    ignoring: bool,
    partial_utf8: [u8; 4],
    partial_utf8_len: usize,
}

impl Parser {
    /// Create a new Parser.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn params(&self) -> &Params {
        &self.params
    }

    #[inline]
    fn intermediates(&self) -> &[u8] {
        &self.intermediates[..self.intermediate_idx]
    }

    /// Advance the parser state.
    ///
    /// Requires a [`Perform`] implementation to handle the triggered actions.
    #[inline]
    pub fn advance<P: Perform>(&mut self, performer: &mut P, bytes: &[u8]) {
        let mut i = 0;

        // Handle partial codepoints from previous calls to `advance`.
        if self.partial_utf8_len != 0 {
            i += self.advance_partial_utf8(performer, bytes);
        }

        while i != bytes.len() {
            match self.state {
                State::Ground => i += self.advance_ground(performer, &bytes[i..]),
                _ => {
                    let byte = bytes[i];
                    self.change_state(performer, byte);
                    i += 1;
                },
            }
        }
    }

    #[inline(always)]
    fn change_state<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match self.state {
            State::CsiEntry => self.advance_csi_entry(performer, byte),
            State::CsiIgnore => self.advance_csi_ignore(performer, byte),
            State::CsiIntermediate => self.advance_csi_intermediate(performer, byte),
            State::CsiParam => self.advance_csi_param(performer, byte),
            State::DcsEntry => self.advance_dcs_entry(performer, byte),
            State::DcsIgnore => self.anywhere(performer, byte),
            State::DcsIntermediate => self.advance_dcs_intermediate(performer, byte),
            State::DcsParam => self.advance_dcs_param(performer, byte),
            State::DcsPassthrough => self.advance_dcs_passthrough(performer, byte),
            State::Escape => self.advance_esc(performer, byte),
            State::EscapeIntermediate => self.advance_esc_intermediate(performer, byte),
            State::OscString => self.advance_osc_string(performer, byte),
            State::SosPmApcString => self.anywhere(performer, byte),
            State::Ground => unreachable!(),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Default, Copy, Clone)]
enum State {
    CsiEntry,
    CsiIgnore,
    CsiIntermediate,
    CsiParam,
    DcsEntry,
    DcsIgnore,
    DcsIntermediate,
    DcsParam,
    DcsPassthrough,
    Escape,
    EscapeIntermediate,
    OscString,
    SosPmApcString,
    #[default]
    Ground,
}

/// Performs actions requested by the Parser.
///
/// Actions in this case mean, for example, handling a CSI escape sequence
/// describing cursor movement, or simply printing characters to the screen.
///
/// The methods on this type correspond to actions described in
/// <http://vt100.net/emu/dec_ansi_parser>.
pub trait Perform {
    /// Draw a character to the screen and update states.
    fn print(&mut self, _c: char) {}

    /// Execute a C0 or C1 control function.
    fn execute(&mut self, _byte: u8) {}

    /// Invoked when a final character arrives in first part of device control
    /// string.
    ///
    /// The control function should be determined from the private marker, final
    /// character, and execute with a parameter list. A handler should be
    /// selected for remaining characters in the string; the handler
    /// function should subsequently be called by `put` for every character in
    /// the control string.
    ///
    /// The `ignore` flag indicates that more than two intermediates arrived and
    /// subsequent characters were ignored.
    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}

    /// Pass bytes as part of a device control string to the handle chosen in
    /// `hook`. C0 controls will also be passed to the handler.
    fn put(&mut self, _byte: u8) {}

    /// Called when a device control string is terminated.
    ///
    /// The previously selected handler should be notified that the DCS has
    /// terminated.
    fn unhook(&mut self) {}

    /// Dispatch an operating system command.
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

    /// A final character has arrived for a CSI sequence.
    ///
    /// The `ignore` flag indicates that either more than two intermediates
    /// arrived or the number of parameters exceeded the maximum supported
    /// length, and subsequent characters were ignored.
    fn csi_dispatch(
        &mut self,
        _params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
    }

    /// The final character of an escape sequence has arrived.
    ///
    /// The `ignore` flag indicates that more than two intermediates arrived and
    /// subsequent characters were ignored.
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}
