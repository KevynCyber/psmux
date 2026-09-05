// Covers: ZDEP-021
// Requirement: docs/plans/2026-09-04-zero-third-party-deps.md A13 -- a
// hand-written parser replacing a fuzz-hardened crate needs a permanent,
// zero-dependency, deterministic random-bytes no-panic smoke test. A fixed-
// seed xorshift64* PRNG (no crates) generates 1,000,000 bytes, biased so
// ~30% are drawn from a set of bytes that are meaningful to the VT state
// machine (ESC, CSI/OSC/DCS introducers, `;`, `:`, the C1 8-bit forms
// 0x9B/0x9D/0x90/0x9C, BEL, CAN, SUB, DEL, the 0x80..=0x9F C1 range,
// ASCII digits, `?`, `m`, `H`, `\`) and the rest uniform over 0..=255. This
// buffer is fed, in chunk sizes cycling through 1,2,3,5,8,13,64,1000
// bytes, through (a) `vt100_psmux::vt_parser::Parser` with a no-op
// `Perform` and (b) the screen parser `vt100_psmux::Parser::new(24, 80,
// 100)` via `process`; neither may panic. After (b), `screen().contents()`
// and `screen().contents_formatted()` are also called on the resulting
// screen to exercise the output-formatting code paths (this also
// exercises the itoa replacement, since those methods format cursor
// positions and SGR parameters as decimal text).

use vt100_psmux::vt_parser;

const NUM_BYTES: usize = 1_000_000;
const CHUNK_SIZES: &[usize] = &[1, 2, 3, 5, 8, 13, 64, 1000];

/// xorshift64* PRNG. Deterministic for a fixed seed; no external crate.
struct Xorshift64Star {
    state: u64,
}

impl Xorshift64Star {
    fn new(seed: u64) -> Self {
        // xorshift64* requires a nonzero seed.
        Self { state: if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed } }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

fn bias_set() -> Vec<u8> {
    let mut set = vec![
        0x1B, 0x5B, 0x5D, 0x50, 0x3B, 0x3A, 0x9B, 0x9D, 0x90, 0x9C, 0x07, 0x18, 0x1A, 0x7F, b'?',
        b'm', b'H', b'\\',
    ];
    for b in 0x80u16..=0x9F {
        set.push(b as u8);
    }
    for b in b'0'..=b'9' {
        set.push(b);
    }
    set
}

fn generate_biased_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut rng = Xorshift64Star::new(seed);
    let bias = bias_set();
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        let roll = rng.next_u64() % 100;
        if roll < 30 {
            let idx = (rng.next_u64() as usize) % bias.len();
            out.push(bias[idx]);
        } else {
            out.push((rng.next_u64() % 256) as u8);
        }
    }
    out
}

struct NoopPerform;

impl vt_parser::Perform for NoopPerform {}

fn feed_in_cycling_chunks<F: FnMut(&[u8])>(bytes: &[u8], mut feed: F) {
    let mut offset = 0;
    let mut chunk_idx = 0;
    while offset < bytes.len() {
        let size = CHUNK_SIZES[chunk_idx % CHUNK_SIZES.len()];
        let end = (offset + size).min(bytes.len());
        feed(&bytes[offset..end]);
        offset = end;
        chunk_idx += 1;
    }
}

#[test]
fn one_million_biased_random_bytes_do_not_panic_vt_parser() {
    let bytes = generate_biased_bytes(0x1234_5678_9ABC_DEF0, NUM_BYTES);
    assert_eq!(bytes.len(), NUM_BYTES);

    let mut parser = vt_parser::Parser::new();
    let mut perf = NoopPerform;
    feed_in_cycling_chunks(&bytes, |chunk| parser.advance(&mut perf, chunk));
}

#[test]
fn one_million_biased_random_bytes_do_not_panic_screen_parser() {
    let bytes = generate_biased_bytes(0x1234_5678_9ABC_DEF0, NUM_BYTES);
    assert_eq!(bytes.len(), NUM_BYTES);

    let mut parser = vt100_psmux::Parser::new(24, 80, 100);
    feed_in_cycling_chunks(&bytes, |chunk| {
        parser.process(chunk);
    });

    // Exercise the output-formatting paths (and the itoa replacement) on
    // whatever state the screen ended up in after 1M biased random bytes.
    let _ = parser.screen().contents();
    let _ = parser.screen().contents_formatted();
}
