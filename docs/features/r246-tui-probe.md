# r246 TUI probe tooling feature area

Spec home for this repo is `docs/features/*.md`, not `.claude/spec-cache/`:
`.gitignore:89` ignores `.claude/` here, so no spec-cache area can live in
this repo.

Repo-wide collision check before allocating: `git grep -oE
"[A-Z]{3,6}-[0-9]{3}"` over master returns only `DECSET-100`, `HOOKS-129`,
`TOKEN-272`, `ZDEP-001..051`, plus `R245-001..007` (docs/features/
r245-psmux-slices.md). No `R246` prefix existed. This probe is tooling, not
one of the three r245 product slices, so it gets its own area rather than an
r245 ID.

## R246-001 Scripted TUI probe that cannot touch live sessions

`scripts/psmux_tui_probe.py` drives a psmux TUI non-interactively so an agent
can paste text, send keys and read a pane back, under AGENTS.md test
isolation.

CLI:

- `--ns <name>` REQUIRED. Refuses an empty value and refuses the literal
  `default`. A refusal happens before any psmux invocation.
- `--send-paste <file>` reads the file's bytes, wraps them in the
  bracketed-paste opener `ESC[200~` and terminator `ESC[201~` (once for the
  whole payload, not per line) and delivers them via `send-keys -l`.
- `--send-keys <keys>` delivers key names via `send-keys`, unwrapped.
- `--capture` runs `capture-pane -p`.
- `--wait <seconds>` settle delay between steps.
- `--keep` suppresses cleanup, on both the success and the failure path.

Session handling: `ensure_session` uses `has-session` / `new-session` against
ONE implicit session named `probe`.

Cleanup: ALWAYS `psmux -L <ns> kill-server` from a `finally` block. A bare
`psmux kill-server` is forbidden -- it would kill every namespace, including
the user's live sessions.

Safety: snapshot the DEFAULT namespace session list (unscoped `psmux
list-sessions`) before and after the run; abort nonzero on any change.

Seam: module-level `_run = subprocess.run`, the single choke point every
child process goes through, so tests intercept all argv without starting a
server.

Tests: `scripts/test_psmux_tui_probe.py` (stdlib `unittest`, run with
`python -m unittest scripts/test_psmux_tui_probe.py`).
