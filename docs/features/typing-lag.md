# Typing latency: signalled wakeups feature area

Spec home for this repo is `docs/features/*.md`, not `.claude/spec-cache/`
(`.gitignore` ignores `.claude/` here; see `r245-psmux-slices.md`).

Root cause: a keystroke echo crosses several un-signalled hops (parser ->
server loop -> writer thread -> client input loop), each gated by a timed
wait. On Windows every timed wait rounds up to the ~15.6ms scheduler tick,
so the hops stack into visible typing lag. Each ID below replaces one timed
wait with an explicit signal. IDs allocated via `next_feature_id.py LAG`.

## LAG-001 Parser output wakes the server loop

The parser thread's "PTY data ready" transition (flag false -> true) sends
exactly one `CtrlReq::Wake` to the server's registered wake sender
(`crate::types::register_server_waker(Sender<CtrlReq>)`); the transition is
performed by `crate::types::mark_pty_data_ready()`. Repeated marks while
the flag is still set send nothing; after the server loop clears the flag
the next mark sends one Wake again. The server loop drops Wake and does not
count a Wake-only batch as client activity.
Tests: `tests-rs/test_typing_lag_server_wake.rs`.

## LAG-002 Frame push wakes the connection writer

`push_frame` sends exactly one `WriterMsg::FrameReady` to a client's
registered waker (`crate::types::register_frame_waker(client_id,
Sender<WriterMsg>)`) when that client's frame slot goes empty -> full. A
push that overwrites an undrained frame sends nothing. Writer channel items
are `WriterMsg { Resp(Receiver<String>), FrameReady }`.
Tests: `tests-rs/test_typing_lag_frame_waker.rs`.

## LAG-003 Client input wait wakes on a received frame

`src/term/console` owns a process-wide auto-reset frame event. The client
frame reader thread calls `signal_frame_ready()` after each frame it
forwards. `wait_input_or_frame(input_handle, timeout) -> io::Result<Wakeup>`
(`Wakeup { Input, FrameReady, Timeout }`) waits on both the input handle
and the frame event, so a long input timeout returns as soon as a frame
arrives. Input wins when both are signalled; the event auto-resets after
one FrameReady.
Tests: `tests-rs/test_typing_lag_console_frame_event.rs`.

## LAG-004 Small output batches skip the coalescing wait

`crate::pane::should_coalesce(len: usize) -> bool` is false for a staged
batch under 256 bytes (keystroke echo) and true at 256 bytes and above.
The parser thread runs the adaptive coalesce loop only when it is true.
Tests: `tests-rs/test_typing_lag_coalesce.rs`.

## LAG-005 Small batches that end mid-frame still coalesce

`crate::pane::should_coalesce_batch(bytes: &[u8]) -> bool` decides the
coalesce wait from the staged bytes, not only their length. It is true when
`should_coalesce(bytes.len())` is true, when the batch ends inside an
unterminated escape sequence (a trailing lone ESC, a CSI without its final
byte, or an OSC without its BEL/ST terminator), or when the batch leaves the
cursor hidden (its last `ESC[?25l` has no later `ESC[?25h`). A multi-chunk
ConPTY redraw that opens with a small chunk therefore waits for the rest
instead of being snapshotted half-applied. A small complete echo (`a`,
`a ESC[?25h`) is still false, so LAG-004 holds. The parser thread uses
`should_coalesce_batch` on the current staged bytes.
Tests: `tests-rs/test_typing_lag_coalesce.rs`.

## LAG-006 Server frame pushes are rate limited, idle pushes are not

`crate::wake::MIN_FRAME_PUSH_INTERVAL` is 4ms.
`crate::wake::frame_push_wait(last_push: Option<Instant>, now: Instant) ->
Duration` returns how long the server loop must hold a dirty frame before
pushing it: `Duration::ZERO` when there was no previous push or at least
`MIN_FRAME_PUSH_INTERVAL` has elapsed since it (the first push after an
idle period goes out at once, so a keystroke echo is never delayed), else
the remainder of the interval. It never exceeds the interval and never
panics when `now` is earlier than `last_push` (treated as zero elapsed).
The server loop pushes only when the wait is zero, keeps the frame dirty
otherwise, and caps its recv timeout at the wait so the deferred frame goes
out at the deadline. Streaming output therefore costs at most ~250 full
JSON frames per second instead of one per Wake (~1 kHz).
Tests: `tests-rs/test_typing_lag_push_rate.rs`.

## LAG-007 1ms timer resolution is held only while clients are attached

`crate::sched_priority::TimerResolutionGuard::acquire()` is an RAII guard:
the first live guard requests a 1ms system timer (`timeBeginPeriod(1)`) and
dropping the last one releases it (`timeEndPeriod(1)`).
`crate::sched_priority::timer_resolution_holders() -> usize` reports the
live guard count on every platform (the OS calls are Windows-only).
`opt_out_of_power_throttling()` no longer calls `timeBeginPeriod`. Each
`crate::wake::WriterHandle` (one per attached persistent client connection)
holds a guard for its lifetime, so a server with no attached clients runs
at the default timer resolution. The client process holds a guard for the
lifetime of its attach loop.
Tests: `tests-rs/test_typing_lag_timer_resolution.rs`.

## LAG-008 Frame waker is registered before the frame slot opens

In the persistent-connection setup (`src/server/connection.rs`), the
`WriterHandle` (which registers the frame waker) is created before
`register_frame_channel(client_id)` and before the writer thread is
spawned. A frame pushed into the new slot therefore always finds a waker:
no window exists in which the slot goes empty -> full with no FrameReady
sent (after which overwrites send nothing, leaving the frame to the
writer's 5ms fallback poll).
Tests: `tests-rs/test_typing_lag_waker_order.rs`.

## Out of scope: SSH input path frame wakeups

`InputSource::Ssh` (`src/ssh_input.rs`) waits in `rx.recv_timeout` on its
reader thread's `sync_channel<Event>`, so `signal_frame_ready()` does not
end that wait (LAG-003 covers only the native console path). Not fixed
here: the fix changes the reader channel's item type in both
`start_ssh_reader` variants, SSH round-trip latency already dominates the
tick it would save, and no in-process test can drive the console-backed
reader. Track as a separate item if SSH typing lag is reported.
