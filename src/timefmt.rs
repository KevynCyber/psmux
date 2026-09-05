//! ZDEP-010/ZDEP-011: native Win32-backed replacement for the crate's
//! former third-party local clock, strftime, and epoch-seconds
//! conversions. `LocalTime` mirrors the fields of `SYSTEMTIME`
//! (weekday: 0=Sunday..6=Saturday).

use crate::win32::{self, FILETIME, SYSTEMTIME};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millisecond: u32,
    pub weekday: u32,
}

impl LocalTime {
    fn from_systemtime(st: &SYSTEMTIME) -> LocalTime {
        LocalTime {
            year: st.wyear as i32,
            month: st.wmonth as u32,
            day: st.wday as u32,
            hour: st.whour as u32,
            minute: st.wminute as u32,
            second: st.wsecond as u32,
            millisecond: st.wmilliseconds as u32,
            weekday: st.wdayofweek as u32,
        }
    }
}

const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The live local clock, read via `GetLocalTime`.
pub fn now() -> LocalTime {
    let mut st = SYSTEMTIME::default();
    // FFI call into kernel32; `st` is a valid out-param.
    unsafe { win32::GetLocalTime(&mut st) };
    LocalTime::from_systemtime(&st)
}

/// Converts UTC epoch seconds to local time via `FileTimeToLocalFileTime` +
/// `FileTimeToSystemTime`. Returns `None` outside the FILETIME-representable
/// range (before 1601-01-01, at/after year 30827) or on Win32 failure.
pub fn local_from_epoch_secs(secs: i64) -> Option<LocalTime> {
    const EPOCH_DIFF_SECS: i64 = 11_644_473_600; // 1601-01-01 -> 1970-01-01
    let filetime_secs = secs.checked_add(EPOCH_DIFF_SECS)?;
    if filetime_secs < 0 {
        return None;
    }
    let filetime_100ns = (filetime_secs as u64).checked_mul(10_000_000)?;
    let utc_ft = FILETIME {
        dwlowdatetime: (filetime_100ns & 0xFFFF_FFFF) as u32,
        dwhighdatetime: (filetime_100ns >> 32) as u32,
    };

    let mut local_ft = FILETIME::default();
    // FFI call; both pointers point at valid stack locals.
    let ok = unsafe { win32::FileTimeToLocalFileTime(&utc_ft, &mut local_ft) };
    if ok == 0 {
        return None;
    }

    let mut st = SYSTEMTIME::default();
    // FFI call; both pointers point at valid stack locals.
    let ok = unsafe { win32::FileTimeToSystemTime(&local_ft, &mut st) };
    if ok == 0 {
        return None;
    }

    Some(LocalTime::from_systemtime(&st))
}

/// Formats `t` per the documented status-line specifier set plus `%m`
/// (zero-padded month, needed to match the golden fixture's `strftime`
/// output on composite formats such as `%Y-%m-%d`). Any other `%` sequence,
/// including a trailing bare `%`, returns `None`, matching the prior
/// strftime crate's write failure on an unknown specifier.
pub fn strftime(t: &LocalTime, fmt: &str) -> Option<String> {
    let mut out = String::with_capacity(fmt.len());
    let mut chars = fmt.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            None => return None,
            Some('%') => out.push('%'),
            Some('H') => out.push_str(&format!("{:02}", t.hour)),
            Some('I') => {
                let h12 = match t.hour % 12 {
                    0 => 12,
                    h => h,
                };
                out.push_str(&format!("{:02}", h12));
            }
            Some('M') => out.push_str(&format!("{:02}", t.minute)),
            Some('S') => out.push_str(&format!("{:02}", t.second)),
            Some('p') => out.push_str(if t.hour < 12 { "AM" } else { "PM" }),
            Some('R') => out.push_str(&format!("{:02}:{:02}", t.hour, t.minute)),
            Some('d') => out.push_str(&format!("{:02}", t.day)),
            Some('e') => out.push_str(&format!("{:2}", t.day)),
            Some('m') => out.push_str(&format!("{:02}", t.month)),
            Some('b') => out.push_str(*MONTHS.get(t.month.wrapping_sub(1) as usize)?),
            Some('Y') => out.push_str(&t.year.to_string()),
            Some('a') => out.push_str(*WEEKDAYS.get(t.weekday as usize)?),
            Some('.') => {
                if chars.next() != Some('3') || chars.next() != Some('f') {
                    return None;
                }
                out.push_str(&format!(".{:03}", t.millisecond));
            }
            Some(_) => return None,
        }
    }
    Some(out)
}

/// `"%H:%M:%S%.3f"` of the current local time, for the crate's various
/// debug-log timestamp prefixes.
pub fn log_timestamp() -> String {
    strftime(&now(), "%H:%M:%S%.3f").unwrap_or_default()
}

/// Wall-clock UTC epoch seconds corresponding to a monotonic `Instant`
/// (e.g. `AppState::created_at`). `Instant` has no epoch of its own, so this
/// derives it from the current wall clock minus the elapsed monotonic
/// duration.
pub fn epoch_secs_since(instant: std::time::Instant) -> i64 {
    std::time::SystemTime::now()
        .checked_sub(instant.elapsed())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `instant` rendered as `"%a %b %e %H:%M:%S %Y"` (empty string if the
/// derived epoch seconds fall outside the representable range).
pub fn display_since(instant: std::time::Instant) -> String {
    local_from_epoch_secs(epoch_secs_since(instant))
        .and_then(|lt| strftime(&lt, "%a %b %e %H:%M:%S %Y"))
        .unwrap_or_default()
}
