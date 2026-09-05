/// serde_json-shaped number storage: an unsigned integer, a signed integer,
/// or a float, chosen by the parser based on the literal's syntax (a `.` or
/// `e`/`E` makes it a float; otherwise the sign and magnitude pick PosInt vs
/// NegInt). `-0` is the one syntactic special case: serde_json represents it
/// as the float `-0.0` since neither integer variant can hold a negative
/// zero distinct from zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    PosInt(u64),
    NegInt(i64),
    Float(f64),
}

impl Number {
    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Number::PosInt(v) => i64::try_from(v).ok(),
            Number::NegInt(v) => Some(v),
            Number::Float(_) => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            Number::PosInt(v) => Some(v),
            Number::NegInt(v) => u64::try_from(v).ok(),
            Number::Float(_) => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Number::PosInt(v) => Some(v as f64),
            Number::NegInt(v) => Some(v as f64),
            Number::Float(v) => Some(v),
        }
    }

    /// Compact decimal text matching serde_json's writer for every fixture
    /// row: integers print as plain decimal; floats use Rust's own
    /// shortest-round-trip `Debug` formatting (which already matches
    /// serde_json's ryu-based output for the digits and the plain/scientific
    /// threshold) with a `+` inserted after `e` for a non-negative exponent,
    /// since `{:?}` omits it but serde_json's writer does not.
    pub fn to_display_string(self) -> String {
        match self {
            Number::PosInt(v) => v.to_string(),
            Number::NegInt(v) => v.to_string(),
            Number::Float(v) => format_float(v),
        }
    }
}

fn format_float(f: f64) -> String {
    let s = format!("{f:?}");
    match s.find(['e', 'E']) {
        Some(epos) => {
            let (mantissa, exp) = s.split_at(epos);
            let exp_digits = &exp[1..];
            if exp_digits.starts_with('-') || exp_digits.starts_with('+') {
                s
            } else {
                format!("{mantissa}e+{exp_digits}")
            }
        }
        None => s,
    }
}
