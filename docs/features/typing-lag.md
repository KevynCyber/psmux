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
