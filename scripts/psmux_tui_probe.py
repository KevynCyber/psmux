# Covers: R246-001
# Drive a psmux TUI from a script so an agent can paste text, send keys and
# capture a pane without ever touching the user's live sessions (AGENTS.md
# test isolation). Every psmux invocation is namespace-scoped with -L <ns>,
# the namespace is torn down in a finally block, and the DEFAULT namespace's
# session list is snapshotted before and after: any change aborts nonzero.
#
#   python scripts/psmux_tui_probe.py --ns probe-ns --send-paste payload.txt \
#       --send-keys Enter --wait 0.5 --capture

import argparse
import subprocess
import sys
import time

# Seam: every child process goes through this name so tests can intercept it.
_run = subprocess.run

SESSION = "probe"
PASTE_OPEN = "\x1b[200~"
PASTE_CLOSE = "\x1b[201~"

RC_BAD_NAMESPACE = 2
RC_DEFAULT_NS_CHANGED = 3
RC_PROBE_FAILED = 1


def _psmux(argv):
    return _run(
        argv,
        capture_output=True,
        encoding="utf-8",
        errors="replace",
    )


def _scoped(ns, *rest):
    return _psmux(["psmux", "-L", ns] + list(rest))


def snapshot_default_sessions():
    """Session list of the DEFAULT namespace -- deliberately unscoped."""
    return _psmux(["psmux", "list-sessions"]).stdout


def ensure_session(ns):
    """One implicit session named `probe`, created only if absent."""
    if _scoped(ns, "has-session", "-t", SESSION).returncode != 0:
        _scoped(ns, "new-session", "-d", "-s", SESSION)


def send_paste(ns, path):
    with open(path, "rb") as handle:
        payload = handle.read().decode("utf-8", "replace")
    # One bracketed paste around the whole payload, newlines included.
    literal = PASTE_OPEN + payload + PASTE_CLOSE
    _scoped(ns, "send-keys", "-t", SESSION, "-l", literal)


def send_keys(ns, keys):
    _scoped(ns, "send-keys", "-t", SESSION, *keys.split())


def capture_pane(ns):
    return _scoped(ns, "capture-pane", "-t", SESSION, "-p").stdout


def cleanup(ns):
    _psmux(["psmux", "-L", ns, "kill-server"])


def parse_args(argv):
    parser = argparse.ArgumentParser(
        description="Drive a psmux TUI in an isolated namespace."
    )
    parser.add_argument("--ns", help="psmux namespace (-L); required, not 'default'")
    parser.add_argument("--send-paste", help="file whose bytes are bracketed-pasted")
    parser.add_argument("--send-keys", help="key names to send, space separated")
    parser.add_argument("--capture", action="store_true", help="run capture-pane -p")
    parser.add_argument("--wait", type=float, default=0.0, help="seconds to settle")
    parser.add_argument(
        "--keep", action="store_true", help="leave the namespace alive"
    )
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)

    # Refuse before any shell-out: the default namespace holds live sessions.
    ns = args.ns
    if not ns or ns == "default":
        sys.stderr.write(
            "refusing to run: --ns is required, must be non-empty and not "
            '"default"\n'
        )
        return RC_BAD_NAMESPACE

    before = snapshot_default_sessions()
    rc = 0
    try:
        ensure_session(ns)
        if args.send_paste:
            send_paste(ns, args.send_paste)
        if args.send_keys:
            send_keys(ns, args.send_keys)
        if args.wait > 0:
            time.sleep(args.wait)
        if args.capture:
            sys.stdout.write(capture_pane(ns))
    except Exception as exc:
        sys.stderr.write("probe failed: %s\n" % (exc,))
        rc = RC_PROBE_FAILED
    finally:
        if not args.keep:
            cleanup(ns)

    if snapshot_default_sessions() != before:
        sys.stderr.write(
            "default-namespace session list changed during the run -- aborting\n"
        )
        return RC_DEFAULT_NS_CHANGED
    return rc


if __name__ == "__main__":
    sys.exit(main())
