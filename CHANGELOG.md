# Changelog

## [4.5.0] - 2026-09-25

### Performance

- Typing-lag review follow-ups (spec `docs/features/typing-lag.md`, LAG-006..LAG-008).
  - LAG-006: server frame pushes are spaced at least 4ms apart
    (`crate::wake::MIN_FRAME_PUSH_INTERVAL`). The first push after an idle
    period goes out at once; a deferred dirty frame goes out at its deadline,
    so streaming output costs at most ~250 JSON frames per second instead of
    one per Wake.
  - LAG-007: the 1ms system timer (`timeBeginPeriod(1)`) is held through a
    ref-counted `TimerResolutionGuard` only while clients are attached (one
    per `WriterHandle`, plus the client attach loop), instead of for the whole
    server lifetime.

### Fixed

- LAG-008: the persistent-connection setup registers the frame waker before the
  frame slot opens, so the first frame pushed to a new client always wakes its
  writer instead of waiting out the 5ms fallback poll.

## [4.4.0] - 2026-09-25

### Performance

- Typing-lag signalled wakeups (spec `docs/features/typing-lag.md`, LAG-001..LAG-005).
  Each hop on the keystroke-to-echo path used to wait out a timed poll that
  Windows rounds up to its 15.6ms tick; the hops are now signalled.
  - LAG-001: pane parser threads send one `CtrlReq::Wake` when PTY data becomes
    ready, so the server loop returns immediately instead of waiting out its
    timeout. `Wake` is not counted as client activity.
  - LAG-002: `push_frame` sends `WriterMsg::FrameReady` to the client writer when
    its frame slot goes empty to full; the 5ms poll remains as a fallback.
  - LAG-003: the client reader thread sets a Win32 event after each frame, and
    `console::poll` waits on stdin and that event, so the client renders a frame
    as soon as it arrives. Input keeps priority.
  - LAG-004: batches under 256 bytes are parsed immediately instead of sitting
    through the adaptive coalescing wait. Large multi-chunk frames still coalesce.
  - LAG-005: small batches that end mid-escape-sequence or with the cursor hidden
    are still coalesced, so a partial frame is not rendered.
  - The server and client opt out of power throttling (EcoQoS), request a 1ms
    timer, and run the hot threads (ConPTY reader, parser, server loop,
    per-client writer, client main thread) at ABOVE_NORMAL priority. This is best
    effort and does nothing off Windows.
- Measured keystroke-to-frame latency, base build vs this build:
  - Idle: p50 46ms vs 16ms; p99 197-321ms vs 17-31ms.
  - Under 8 CPU hogs: p99 509ms vs 76ms.
  - One earlier idle run of the new build had 26 misses. It did not reproduce
    in two reruns.

### Tests

- The latency test scripts (`tests/test_e2e_latency.ps1`,
  `tests/test_typing_render_latency.ps1`,
  `tests/test_render_pressure_input_latency.ps1`) are scoped to `-L $Namespace`
  (default `lat-test-<pid>-<random>`) and pass `-L` on every psmux call.
  Cleanup runs `psmux -L <ns> kill-server` only, instead of a bare
  `kill-server` that ended every session.
- New `scripts/lat-probe.ps1` measures keystroke-to-frame latency. `-Exe`
  compares a baseline binary with a new build, and `-Hogs` adds CPU pressure.
  It refuses to run in the default namespace.
