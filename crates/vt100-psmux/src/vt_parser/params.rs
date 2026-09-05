//! Fixed size parameters list with optional subparameters.
//!
//! Ported from vte 0.15.0's `src/params.rs` (MIT/Apache-2.0):
//!   `C:/Users/Kev/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vte-0.15.0/src/params.rs`
//!
//! `MAX_PARAMS` = 32, matching vte exactly (CSI/DCS parameters and
//! subparameters share this one combined budget).

use std::fmt::{self, Debug, Formatter};

const MAX_PARAMS: usize = 32;

/// Fixed-capacity list of CSI/DCS parameters, each possibly split into
/// subparameters (e.g. the `2:255:0:255` in `CSI 38:2:255:0:255 m`).
///
/// `params` deliberately keeps vte's own field name (matching upstream for
/// auditability), which clippy's `struct_field_names` flags as prefix-
/// duplicating the type name `Params` -- allowed here rather than renamed.
#[derive(Default)]
#[allow(clippy::struct_field_names)]
pub struct Params {
    /// Number of subparameters for each parameter.
    ///
    /// For each entry in the `params` slice, this stores the length of the
    /// param as number of subparams at the same index as the param in the
    /// `params` slice.
    ///
    /// At the subparam positions the length will always be `0`.
    subparams: [u8; MAX_PARAMS],

    /// All parameters and subparameters.
    params: [u16; MAX_PARAMS],

    /// Number of subparameters in the current parameter.
    current_subparams: u8,

    /// Total number of parameters and subparameters.
    len: usize,
}

impl Params {
    /// Returns the number of parameters (including subparameters).
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if there are no parameters present.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns an iterator over all parameters and subparameters.
    #[must_use]
    #[inline]
    pub fn iter(&self) -> ParamsIter<'_> {
        ParamsIter::new(self)
    }

    /// Returns `true` if there is no more space for additional parameters.
    #[inline]
    pub(crate) fn is_full(&self) -> bool {
        self.len == MAX_PARAMS
    }

    /// Clear all parameters.
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.current_subparams = 0;
        self.len = 0;
    }

    /// Add an additional parameter.
    #[inline]
    pub(crate) fn push(&mut self, item: u16) {
        self.subparams[self.len - self.current_subparams as usize] = self.current_subparams + 1;
        self.params[self.len] = item;
        self.current_subparams = 0;
        self.len += 1;
    }

    /// Add an additional subparameter to the current parameter.
    #[inline]
    pub(crate) fn extend(&mut self, item: u16) {
        self.subparams[self.len - self.current_subparams as usize] = self.current_subparams + 1;
        self.params[self.len] = item;
        self.current_subparams += 1;
        self.len += 1;
    }
}

impl<'a> IntoIterator for &'a Params {
    type IntoIter = ParamsIter<'a>;
    type Item = &'a [u16];

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Immutable subparameter iterator.
pub struct ParamsIter<'a> {
    params: &'a Params,
    index: usize,
}

impl<'a> ParamsIter<'a> {
    fn new(params: &'a Params) -> Self {
        Self { params, index: 0 }
    }
}

impl<'a> Iterator for ParamsIter<'a> {
    type Item = &'a [u16];

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.params.len() {
            return None;
        }

        // Get all subparameters for the current parameter.
        let num_subparams = self.params.subparams[self.index];
        let param = &self.params.params[self.index..self.index + num_subparams as usize];

        // Jump to the next parameter.
        self.index += num_subparams as usize;

        Some(param)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.params.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl Debug for Params {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;

        for (i, param) in self.iter().enumerate() {
            if i != 0 {
                write!(f, ";")?;
            }

            for (i, subparam) in param.iter().enumerate() {
                if i != 0 {
                    write!(f, ":")?;
                }

                subparam.fmt(f)?;
            }
        }

        write!(f, "]")
    }
}
