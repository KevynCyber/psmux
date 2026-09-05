//! Action helpers, OSC dispatch, ground-state and partial-UTF-8 handling,
//! ported from vte 0.15.0's `Parser::action_*`/`osc_dispatch`/
//! `advance_ground`/`advance_partial_utf8`/`ground_dispatch`. See the parent
//! module doc comment for the source path and licence.

use std::str;

use super::{Parser, Perform, State, MAX_INTERMEDIATES, MAX_OSC_PARAMS};

impl Parser {
    #[inline]
    pub(super) fn action_csi_dispatch<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        if self.params.is_full() {
            self.ignoring = true;
        } else {
            self.params.push(self.param);
        }
        performer.csi_dispatch(self.params(), self.intermediates(), self.ignoring, byte as char);

        self.state = State::Ground;
    }

    #[inline]
    pub(super) fn action_hook<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        if self.params.is_full() {
            self.ignoring = true;
        } else {
            self.params.push(self.param);
        }
        performer.hook(self.params(), self.intermediates(), self.ignoring, byte as char);
        self.state = State::DcsPassthrough;
    }

    #[inline]
    pub(super) fn action_collect(&mut self, byte: u8) {
        if self.intermediate_idx == MAX_INTERMEDIATES {
            self.ignoring = true;
        } else {
            self.intermediates[self.intermediate_idx] = byte;
            self.intermediate_idx += 1;
        }
    }

    /// Advance to the next subparameter.
    #[inline]
    pub(super) fn action_subparam(&mut self) {
        if self.params.is_full() {
            self.ignoring = true;
        } else {
            self.params.extend(self.param);
            self.param = 0;
        }
    }

    /// Advance to the next parameter.
    #[inline]
    pub(super) fn action_param(&mut self) {
        if self.params.is_full() {
            self.ignoring = true;
        } else {
            self.params.push(self.param);
            self.param = 0;
        }
    }

    /// Advance inside the parameter without terminating it.
    #[inline]
    pub(super) fn action_paramnext(&mut self, byte: u8) {
        if self.params.is_full() {
            self.ignoring = true;
        } else {
            self.param = self.param.saturating_mul(10);
            self.param = self.param.saturating_add(u16::from(byte - b'0'));
        }
    }

    /// Add OSC param separator.
    #[inline]
    pub(super) fn action_osc_put_param(&mut self) {
        let idx = self.osc_raw.len();

        let param_idx = self.osc_num_params;
        match param_idx {
            // First param is special - 0 to current byte index.
            0 => self.osc_params[param_idx] = (0, idx),

            // Only process up to MAX_OSC_PARAMS.
            MAX_OSC_PARAMS => return,

            // All other params depend on previous indexing.
            _ => {
                let prev = self.osc_params[param_idx - 1];
                let begin = prev.1;
                self.osc_params[param_idx] = (begin, idx);
            },
        }

        self.osc_num_params += 1;
    }

    #[inline(always)]
    pub(super) fn action_osc_put(&mut self, byte: u8) {
        self.osc_raw.push(byte);
    }

    pub(super) fn osc_end<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        self.action_osc_put_param();
        self.osc_dispatch(performer, byte);
        self.osc_raw.clear();
        self.osc_num_params = 0;
    }

    /// Reset escape sequence parameters and intermediates.
    #[inline]
    pub(super) fn reset_params(&mut self) {
        self.intermediate_idx = 0;
        self.ignoring = false;
        self.param = 0;

        self.params.clear();
    }

    /// Separate method for `osc_dispatch` that borrows self as read-only.
    ///
    /// Builds the `&[&[u8]]` slices into `osc_raw` via a plain `Vec` -- see
    /// the parent module doc comment for why this differs from vte's own
    /// `MaybeUninit`-array approach (that trick exists only to support vte's
    /// `no_std` build, which this port doesn't need).
    #[inline]
    fn osc_dispatch<P: Perform>(&self, performer: &mut P, byte: u8) {
        let mut params: Vec<&[u8]> = Vec::with_capacity(self.osc_num_params);
        for i in 0..self.osc_num_params {
            let (start, end) = self.osc_params[i];
            params.push(&self.osc_raw[start..end]);
        }
        performer.osc_dispatch(&params, byte == 0x07);
    }

    /// Advance the parser state from ground.
    ///
    /// The ground state is handled separately since it can only be left using
    /// the escape character (`\x1b`).
    #[inline]
    pub(super) fn advance_ground<P: Perform>(&mut self, performer: &mut P, bytes: &[u8]) -> usize {
        // Find the next escape character.
        let num_bytes = bytes.len();
        let plain_chars = find_esc(bytes).unwrap_or(num_bytes);

        // If the next character is ESC, just process it and short-circuit.
        if plain_chars == 0 {
            self.state = State::Escape;
            self.reset_params();
            return 1;
        }

        match str::from_utf8(&bytes[..plain_chars]) {
            Ok(parsed) => {
                Self::ground_dispatch(performer, parsed);
                let mut processed = plain_chars;

                // If there's another character, it must be escape so process it directly.
                if processed < num_bytes {
                    self.state = State::Escape;
                    self.reset_params();
                    processed += 1;
                }

                processed
            },
            // Handle invalid and partial utf8.
            Err(err) => {
                // Dispatch all the valid bytes.
                let valid_bytes = err.valid_up_to();
                // Safe: `valid_bytes` is the length of the already-validated utf8 prefix.
                let parsed = str::from_utf8(&bytes[..valid_bytes]).unwrap_or("");
                Self::ground_dispatch(performer, parsed);

                match err.error_len() {
                    Some(len) => {
                        // Execute C1 escapes or emit replacement character.
                        if len == 1 && bytes[valid_bytes] <= 0x9F {
                            performer.execute(bytes[valid_bytes]);
                        } else {
                            performer.print('\u{FFFD}');
                        }

                        // Restart processing after the invalid bytes.
                        valid_bytes + len
                    },
                    None => {
                        if plain_chars < num_bytes {
                            // Process bytes cut off by escape.
                            performer.print('\u{FFFD}');
                            self.state = State::Escape;
                            self.reset_params();
                            plain_chars + 1
                        } else {
                            // Process bytes cut off by the buffer end.
                            let extra_bytes = num_bytes - valid_bytes;
                            let partial_len = self.partial_utf8_len + extra_bytes;
                            self.partial_utf8[self.partial_utf8_len..partial_len]
                                .copy_from_slice(&bytes[valid_bytes..valid_bytes + extra_bytes]);
                            self.partial_utf8_len = partial_len;
                            num_bytes
                        }
                    },
                }
            },
        }
    }

    /// Advance the parser while processing a partial utf8 codepoint.
    #[inline]
    pub(super) fn advance_partial_utf8<P: Perform>(
        &mut self,
        performer: &mut P,
        bytes: &[u8],
    ) -> usize {
        // Try to copy up to 3 more characters, to ensure the codepoint is complete.
        let old_bytes = self.partial_utf8_len;
        let to_copy = bytes.len().min(self.partial_utf8.len() - old_bytes);
        self.partial_utf8[old_bytes..old_bytes + to_copy].copy_from_slice(&bytes[..to_copy]);
        self.partial_utf8_len += to_copy;

        // Parse the unicode character.
        match str::from_utf8(&self.partial_utf8[..self.partial_utf8_len]) {
            // If the entire buffer is valid, use the first character and continue parsing.
            Ok(parsed) => match parsed.chars().next() {
                // A complete, valid utf8 buffer always yields a first character.
                Some(c) => {
                    performer.print(c);

                    self.partial_utf8_len = 0;
                    c.len_utf8() - old_bytes
                },
                None => {
                    self.partial_utf8_len = 0;
                    0
                },
            },
            Err(err) => {
                let valid_bytes = err.valid_up_to();
                // If we have any valid bytes, that means we partially copied another
                // utf8 character into `partial_utf8`. Since we only care about the
                // first character, we just ignore the rest.
                if valid_bytes > 0 {
                    // Safe: `valid_bytes` is the length of the already-validated utf8 prefix.
                    let parsed = str::from_utf8(&self.partial_utf8[..valid_bytes]).unwrap_or("");
                    if let Some(c) = parsed.chars().next() {
                        performer.print(c);

                        self.partial_utf8_len = 0;
                        return valid_bytes - old_bytes;
                    }
                }

                match err.error_len() {
                    // If the partial character was also invalid, emit the replacement
                    // character.
                    Some(invalid_len) => {
                        performer.print('\u{FFFD}');

                        self.partial_utf8_len = 0;
                        invalid_len - old_bytes
                    },
                    // If the character still isn't complete, wait for more data.
                    None => to_copy,
                }
            },
        }
    }

    /// Handle ground dispatch of print/execute for all characters in a string.
    #[inline]
    fn ground_dispatch<P: Perform>(performer: &mut P, text: &str) {
        for c in text.chars() {
            match c {
                '\x00'..='\x1f' | '\u{80}'..='\u{9f}' => performer.execute(c as u8),
                _ => performer.print(c),
            }
        }
    }
}

/// Find the first ESC (0x1B) byte in `bytes`.
///
/// Replaces vte's `memchr::memchr` (a dependency this port avoids); a plain
/// scan is behaviourally identical, only slower for very long plain-text runs.
#[inline]
fn find_esc(bytes: &[u8]) -> Option<usize> {
    bytes.iter().position(|&b| b == 0x1B)
}
