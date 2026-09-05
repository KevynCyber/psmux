// Covers: ZDEP-010, ZDEP-011
// Requirement: `crate::timefmt` replaces chrono's strftime/clock/epoch
// conversions with a native Win32-backed implementation. `LocalTime` mirrors
// SYSTEMTIME fields (weekday: 0=Sunday..6=Saturday). `strftime` supports
// exactly the documented status-line specifier set (`%H %I %M %S %p %R %d
// %b %Y %a`, plus `%e` space-padded day, `%.3f` milliseconds, and `%%`);
// any other `%` sequence (including a trailing `%`) returns `None`,
// mirroring chrono 0.4.45's `DelayedFormat` write failure on an unknown
// specifier. `now()` reads the live local clock. `local_from_epoch_secs`
// converts UTC epoch seconds to local time, returning `None` outside the
// FILETIME-representable range (before 1601-01-01, at/after year 30827).

use crate::timefmt::{self, LocalTime};

use std::time::{SystemTime, UNIX_EPOCH};

const FIXTURE: &str = include_str!("fixtures/strftime_chrono_0.4.45.txt");

fn parse_row(line: &str) -> (LocalTime, String, Option<String>) {
    let parts: Vec<&str> = line.splitn(10, '|').collect();
    assert_eq!(parts.len(), 10, "malformed fixture row: {line:?}");
    let lt = LocalTime {
        year: parts[0].parse().unwrap(),
        month: parts[1].parse().unwrap(),
        day: parts[2].parse().unwrap(),
        hour: parts[3].parse().unwrap(),
        minute: parts[4].parse().unwrap(),
        second: parts[5].parse().unwrap(),
        millisecond: parts[6].parse().unwrap(),
        weekday: parts[7].parse().unwrap(),
    };
    let fmt = parts[8].to_string();
    let expected = parts[9].to_string();
    let expected = if expected == "<ERR>" { None } else { Some(expected) };
    (lt, fmt, expected)
}

/// (a) Fixture replay: every row generated from real chrono 0.4.45 output
/// must match `timefmt::strftime` exactly, with zero mismatches.
#[test]
fn strftime_matches_chrono_fixture_with_zero_mismatches() {
    let mut mismatches = Vec::new();
    let mut rows = 0usize;
    for line in FIXTURE.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        rows += 1;
        let (lt, fmt, expected) = parse_row(line);
        let actual = timefmt::strftime(&lt, &fmt);
        if actual != expected {
            mismatches.push(format!(
                "fmt={fmt:?} lt={lt:?} expected={expected:?} actual={actual:?}"
            ));
        }
    }
    assert!(rows > 0, "fixture must not be empty");
    assert!(
        mismatches.is_empty(),
        "{} mismatch(es) out of {} rows:\n{}",
        mismatches.len(),
        rows,
        mismatches.join("\n")
    );
}

fn sample_time() -> LocalTime {
    LocalTime {
        year: 2025,
        month: 1,
        day: 5,
        hour: 0,
        minute: 7,
        second: 9,
        millisecond: 4,
        weekday: 0,
    }
}

/// (b) `%%` escapes to a literal `%`, plain literal text passes through
/// unchanged, and an empty format string yields `Some("")`.
#[test]
fn percent_escape_literal_text_and_empty_format() {
    let t = sample_time();
    assert_eq!(timefmt::strftime(&t, "%%"), Some("%".to_string()));
    assert_eq!(timefmt::strftime(&t, "hello world"), Some("hello world".to_string()));
    assert_eq!(timefmt::strftime(&t, ""), Some(String::new()));
}

/// (c) Unknown specifiers, an unsupported fractional-second width, and a
/// trailing bare `%` all return `None`.
#[test]
fn unknown_specifiers_and_trailing_percent_return_none() {
    let t = sample_time();
    assert_eq!(timefmt::strftime(&t, "%Q"), None);
    assert_eq!(timefmt::strftime(&t, "%.4f"), None);
    assert_eq!(timefmt::strftime(&t, "x%"), None);
}

/// (d) `%e` space-pads single-digit days; `%I`/`%p` follow 12-hour clock
/// conventions at the 00:xx / 12:xx / 13:xx boundaries.
#[test]
fn e_space_pad_and_twelve_hour_boundaries() {
    let mut t = sample_time();
    t.day = 5;
    assert_eq!(timefmt::strftime(&t, "%e"), Some(" 5".to_string()));

    t.hour = 0;
    assert_eq!(timefmt::strftime(&t, "%I"), Some("12".to_string()));
    assert_eq!(timefmt::strftime(&t, "%p"), Some("AM".to_string()));

    t.hour = 12;
    assert_eq!(timefmt::strftime(&t, "%I"), Some("12".to_string()));
    assert_eq!(timefmt::strftime(&t, "%p"), Some("PM".to_string()));

    t.hour = 13;
    assert_eq!(timefmt::strftime(&t, "%I"), Some("01".to_string()));
    assert_eq!(timefmt::strftime(&t, "%p"), Some("PM".to_string()));
}

/// (e) `now()` returns sane field ranges and agrees with a
/// `local_from_epoch_secs` conversion of `SystemTime::now()` to within 2
/// seconds (allowing for clock tick skew between the two calls).
#[test]
fn now_returns_sane_ranges_and_matches_epoch_conversion() {
    let now = timefmt::now();
    assert!((1..=9999).contains(&now.year));
    assert!((1..=12).contains(&now.month));
    assert!((1..=31).contains(&now.day));
    assert!(now.hour <= 23);
    assert!(now.minute <= 59);
    assert!(now.second <= 60); // allow leap second tolerance
    assert!(now.millisecond <= 999);
    assert!(now.weekday <= 6);

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs() as i64;
    let converted = timefmt::local_from_epoch_secs(secs).expect("epoch conversion must succeed for current time");

    let now_total = now.hour as i64 * 3600 + now.minute as i64 * 60 + now.second as i64;
    let conv_total = converted.hour as i64 * 3600 + converted.minute as i64 * 60 + converted.second as i64;
    let same_day = now.year == converted.year && now.month == converted.month && now.day == converted.day;
    assert!(
        same_day && (now_total - conv_total).abs() <= 2,
        "now()={now:?} converted={converted:?} must agree within 2 seconds"
    );
}

/// (f) The Unix epoch (secs=0) converts to a local time whose year is 1969
/// or 1970 depending on the local UTC offset.
#[test]
fn epoch_zero_converts_to_1969_or_1970() {
    let lt = timefmt::local_from_epoch_secs(0).expect("epoch 0 must convert");
    assert!(
        lt.year == 1969 || lt.year == 1970,
        "epoch 0 local year must be 1969 or 1970, got {}",
        lt.year
    );
}

/// (g) Values outside the FILETIME-representable range return `None`:
/// `i64::MIN`, one second before 1601-01-01 (secs = -11644473600 - 1), and
/// `i64::MAX`.
#[test]
fn out_of_range_epoch_values_return_none() {
    assert_eq!(timefmt::local_from_epoch_secs(i64::MIN), None);
    assert_eq!(timefmt::local_from_epoch_secs(-11644473600 - 1), None);
    assert_eq!(timefmt::local_from_epoch_secs(i64::MAX), None);
}

/// Sakamoto's algorithm: day-of-week for a Gregorian y/m/d, 0=Sunday.
fn sakamoto_weekday(y: i32, m: u32, d: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut y = y;
    if m < 3 {
        y -= 1;
    }
    let w = (y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] as i32 + d as i32) % 7;
    w.rem_euclid(7) as u32
}

/// (h) Round-trip: converting known epoch seconds to local time must yield
/// a weekday matching Sakamoto's algorithm for that local y/m/d.
#[test]
fn epoch_round_trip_weekday_matches_sakamoto() {
    let secs_list = [
        0i64,
        86400 * 365 * 30,
        1_700_000_000,
        4_102_444_800, // 2100-01-01 UTC
    ];
    for secs in secs_list {
        let lt = timefmt::local_from_epoch_secs(secs)
            .unwrap_or_else(|| panic!("secs={secs} must convert to a local time"));
        let expected_wd = sakamoto_weekday(lt.year, lt.month, lt.day);
        assert_eq!(
            lt.weekday, expected_wd,
            "secs={secs} lt={lt:?} weekday must match Sakamoto's algorithm"
        );
    }
}
