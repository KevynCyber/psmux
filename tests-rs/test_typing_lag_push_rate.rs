// Covers: LAG-006
// Requirement: under streaming output the server must not serialize and push
// a full JSON frame on every Wake (~1 kHz). Pushes are spaced at least
// MIN_FRAME_PUSH_INTERVAL (4ms) apart, but the first push after an idle
// period goes out at once so a keystroke echo is never delayed.
// frame_push_wait(last_push, now) is the pure decision: Duration::ZERO means
// push now, anything else is how long to hold the dirty frame (the server
// loop caps its recv timeout at it so the frame goes out at the deadline).
//
// No real time passes: every instant is derived from one Instant::now() base
// by Duration arithmetic, so results do not depend on scheduling.

use crate::wake::{frame_push_wait, MIN_FRAME_PUSH_INTERVAL};
use std::time::{Duration, Instant};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn interval_is_four_milliseconds() {
    assert_eq!(MIN_FRAME_PUSH_INTERVAL, ms(4));
}

#[test]
fn first_push_ever_goes_out_immediately() {
    let now = Instant::now();
    assert_eq!(frame_push_wait(None, now), Duration::ZERO);
}

#[test]
fn first_push_after_idle_period_goes_out_immediately() {
    let last = Instant::now();
    assert_eq!(frame_push_wait(Some(last), last + Duration::from_secs(1)), Duration::ZERO);
    assert_eq!(frame_push_wait(Some(last), last + ms(50)), Duration::ZERO);
}

#[test]
fn push_exactly_one_interval_after_last_goes_out_immediately() {
    let last = Instant::now();
    assert_eq!(frame_push_wait(Some(last), last + MIN_FRAME_PUSH_INTERVAL), Duration::ZERO);
}

#[test]
fn push_right_after_last_waits_full_interval() {
    let last = Instant::now();
    assert_eq!(frame_push_wait(Some(last), last), MIN_FRAME_PUSH_INTERVAL);
}

#[test]
fn push_inside_interval_waits_only_the_remainder() {
    let last = Instant::now();
    assert_eq!(frame_push_wait(Some(last), last + ms(1)), ms(3));
    assert_eq!(frame_push_wait(Some(last), last + ms(3)), ms(1));
}

#[test]
fn push_just_under_interval_waits_one_microsecond() {
    let last = Instant::now();
    let now = last + MIN_FRAME_PUSH_INTERVAL - Duration::from_micros(1);
    assert_eq!(frame_push_wait(Some(last), now), Duration::from_micros(1));
}

#[test]
fn now_earlier_than_last_push_waits_full_interval_without_panicking() {
    let now = Instant::now();
    let last = now + ms(2);
    assert_eq!(frame_push_wait(Some(last), now), MIN_FRAME_PUSH_INTERVAL);
}

#[test]
fn wait_is_bounded_and_ends_exactly_at_the_interval() {
    // Sweep elapsed 0..=10ms in 37us steps (covers both sides of 4ms).
    let last = Instant::now();
    let mut elapsed = Duration::ZERO;
    while elapsed <= ms(10) {
        let wait = frame_push_wait(Some(last), last + elapsed);
        assert!(wait <= MIN_FRAME_PUSH_INTERVAL, "wait {wait:?} exceeds interval at elapsed {elapsed:?}");
        if elapsed >= MIN_FRAME_PUSH_INTERVAL {
            assert_eq!(wait, Duration::ZERO, "elapsed {elapsed:?} is past the interval: push now");
        } else {
            assert_eq!(elapsed + wait, MIN_FRAME_PUSH_INTERVAL, "deferred push must land exactly on the deadline (elapsed {elapsed:?})");
        }
        elapsed += Duration::from_micros(37);
    }
}
