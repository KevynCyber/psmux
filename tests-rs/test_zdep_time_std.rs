// Covers: ZDEP-011
// Requirement: `AppState::created_at` (`src/types.rs`) becomes a
// `std::time::Instant` (no chrono), and the `Modifier::Time` format
// modifier (`#{t:<epoch-secs>}`, `src/format.rs`) renders via
// `crate::timefmt::local_from_epoch_secs` + `strftime(.., "%a %b %e
// %H:%M:%S %Y")` instead of chrono, keeping the raw value unformatted when
// the timestamp is out of the representable range.

use crate::format::expand_format_for_window;
use crate::timefmt;
use crate::types::AppState;

/// Type-level pin: `AppState::created_at` must be `std::time::Instant`.
/// This only compiles once ZDEP-011 replaces the chrono field.
fn created_at_is_instant(app: &AppState) -> &std::time::Instant {
    &app.created_at
}

#[test]
fn app_state_created_at_is_a_std_instant() {
    let mut app = AppState::new("zdep_time_std_session".to_string());
    app.created_at = std::time::Instant::now();
    let _: &std::time::Instant = created_at_is_instant(&app);
}

const WEEKDAYS: &[&str] = &["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: &[&str] = &[
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The `t` format modifier renders a valid epoch-seconds value as
/// `%a %b %e %H:%M:%S %Y`, matching `crate::timefmt` exactly (no chrono).
#[test]
fn time_modifier_renders_via_timefmt_for_valid_epoch() {
    let app = AppState::new("zdep_time_std_session2".to_string());
    let ts: i64 = 1_700_000_000;
    let result = expand_format_for_window(&format!("#{{t:{ts}}}"), &app, 0);

    let lt = timefmt::local_from_epoch_secs(ts).expect("timestamp must convert");
    let expected = timefmt::strftime(&lt, "%a %b %e %H:%M:%S %Y")
        .expect("supported fixed format must render");
    assert_eq!(result, expected, "#{{t:..}} must match crate::timefmt output exactly");

    let fields: Vec<&str> = result.split(' ').filter(|f| !f.is_empty()).collect();
    assert_eq!(fields.len(), 5, "expected 5 space-separated fields, got {result:?}");
    assert!(WEEKDAYS.contains(&fields[0]), "field 0 must be a weekday abbreviation: {result:?}");
    assert!(MONTHS.contains(&fields[1]), "field 1 must be a month abbreviation: {result:?}");
}

/// Out-of-range timestamps keep the raw value unformatted (mirrors the
/// existing chrono behavior: `DateTime::from_timestamp` returning `None`
/// falls back to the input string unchanged).
#[test]
fn time_modifier_keeps_raw_value_when_out_of_range() {
    let app = AppState::new("zdep_time_std_session3".to_string());
    let raw = "-99999999999999";
    let result = expand_format_for_window(&format!("#{{t:{raw}}}"), &app, 0);
    assert_eq!(result, raw, "out-of-range timestamp must be kept as raw text");
}
