// Covers: ZDEP-014
// Requirement: `psmux_json::parse` never panics on arbitrary or corrupted
// input. A seeded xorshift PRNG feeds >= 100,000 inputs: pure random byte
// strings (turned into a `String` via `from_utf8_lossy` so invalid UTF-8
// doesn't block the call), and random single-byte mutations of every
// value-ok input recorded in the oracle fixture. Only panics fail this
// test; a parse `Err` is an expected, non-failing outcome.

const FIXTURE: &str = include_str!("../../../tests-rs/fixtures/serde_json_1.0.151.txt");

/// Minimal deterministic xorshift64* PRNG so the run is reproducible without
/// pulling in a random-number crate (this repo is going zero-third-party-deps).
struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Xorshift64 { state: seed | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_byte(&mut self) -> u8 {
        (self.next_u64() & 0xff) as u8
    }

    fn next_len(&mut self, max: usize) -> usize {
        (self.next_u64() as usize) % (max + 1)
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('p') => out.push('|'),
                Some('n') => out.push('\n'),
                Some(other) => { out.push('\\'); out.push(other); }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn value_ok_inputs() -> Vec<String> {
    let mut inputs = Vec::new();
    for line in FIXTURE.lines() {
        if let Some(rest) = line.strip_prefix("value-ok|") {
            if let Some((_, payload)) = rest.split_once('|') {
                inputs.push(unescape(payload));
            }
        }
    }
    assert!(!inputs.is_empty(), "fixture must carry value-ok rows to mutate");
    inputs
}

/// Pure random bytes (0..=255), 1..=64 bytes long, turned into a String via
/// lossy UTF-8 conversion so `parse` always receives a valid `&str`.
#[test]
fn random_byte_strings_never_panic() {
    let mut rng = Xorshift64::new(0xC0FFEE_u64);
    for _ in 0..60_000 {
        let len = rng.next_len(64);
        let bytes: Vec<u8> = (0..len).map(|_| rng.next_byte()).collect();
        let s = String::from_utf8_lossy(&bytes).into_owned();
        let _ = psmux_json::parse(&s);
    }
}

/// Random single-byte mutations of every value-ok fixture input: flips one
/// byte to a random value, then feeds the (possibly invalid-UTF-8, hence
/// lossy-converted) result to the parser.
#[test]
fn mutated_fixture_inputs_never_panic() {
    let inputs = value_ok_inputs();
    let mut rng = Xorshift64::new(0xBADC0DE_u64);
    let mut iterations = 0usize;
    // Cycle through the fixture inputs enough times to reach >= 40,000
    // mutated attempts (combined with the 60,000 random-byte-string test
    // above, total >= 100,000 inputs as required by ZDEP-014).
    while iterations < 40_000 {
        for input in &inputs {
            if input.is_empty() {
                iterations += 1;
                continue;
            }
            let mut bytes = input.as_bytes().to_vec();
            let idx = rng.next_len(bytes.len() - 1);
            bytes[idx] = rng.next_byte();
            let mutated = String::from_utf8_lossy(&bytes).into_owned();
            let _ = psmux_json::parse(&mutated);
            iterations += 1;
            if iterations >= 40_000 {
                break;
            }
        }
    }
    assert!(iterations >= 40_000, "must exercise at least 40,000 mutated inputs, ran {iterations}");
}
