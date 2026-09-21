# Agent Instructions

## Test Isolation

- Never run psmux tests or commands that may create sessions in the default
  namespace when working from an active psmux session.
- Some Rust tests create sessions internally and bypass the CLI `-L` option.
  Do not run `cargo test --all-targets` locally unless the process is inside a
  disposable Windows account, VM, or CI environment.
- Prefer safe targeted unit tests that do not start psmux servers, together
  with `cargo check`.
- Run runtime and integration checks with a unique namespace:
  `psmux -L <unique-test-namespace> ...`.
- Snapshot the default session list before and after runtime checks. Stop and
  investigate if it changes.
- Clean up only the test namespace with
  `psmux -L <unique-test-namespace> kill-server`.
- Never use bare `psmux kill-server` for test cleanup because it affects every
  namespace.
- Run the full test suite only in CI or another disposable Windows environment
  where user sessions cannot be affected.

## Cargo Test Target Selection

- psmux has no `[lib]` target: `cargo test --lib` fails with "no library
  targets found" -- a target-selection error, not a test failure. Use
  `cargo test --bin psmux <filter>`.
- Filters are substring matches: the bare filter `session_persist` matches 47
  unrelated tests. Use a module path such as
  `persist::tests_session_persist::`.
- Scope repo-wide greps to `src/`, `tests-rs/`, `scripts/` -- a root-level
  `grep -rn` walks every `.claude/worktrees/` copy and takes >120s.
