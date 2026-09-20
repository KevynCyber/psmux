# Covers: R246-001
# Requirement: scripts/psmux_tui_probe.py drives a psmux TUI from a script so
# an agent can paste text, send keys and capture a pane WITHOUT ever touching
# the user's live sessions. It refuses to run unless given an explicit
# non-empty --ns that is not the literal "default"; every psmux invocation it
# makes is namespace-scoped (`psmux -L <ns> ...`); it ALWAYS tears the test
# namespace down with `psmux -L <ns> kill-server` in a finally block (never a
# bare `psmux kill-server`, which would kill every namespace including the
# user's); --keep suppresses that teardown; --send-paste wraps the payload
# file's bytes in the bracketed-paste opener ESC[200~ and terminator ESC[201~
# and delivers them literally via `send-keys -l`; --capture runs
# `capture-pane -p`; and it snapshots the DEFAULT namespace session list
# before and after the run, aborting if that list changed.
#
# Per AGENTS.md test isolation, these tests NEVER start a psmux server and
# never shell out: they stub the module-level `_run = subprocess.run` seam and
# assert on the argv vectors the script would have executed.
#
# Run with:
#   python -m unittest scripts/test_psmux_tui_probe.py
# from the worktree root.

import importlib.util
import os
import subprocess
import tempfile
import unittest
from unittest import mock

SCRIPTS_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(SCRIPTS_DIR)

PASTE_OPEN = "\x1b[200~"
PASTE_CLOSE = "\x1b[201~"

TEST_NS = "probe-unit-ns"
DEFAULT_SESSIONS = "0: 1 windows (created Sat Sep 20 10:00:00 2026)\n"


def _load_probe():
    """Import psmux_tui_probe.py from the scripts dir by file path."""
    module_path = os.path.join(SCRIPTS_DIR, "psmux_tui_probe.py")
    spec = importlib.util.spec_from_file_location("psmux_tui_probe", module_path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FakeRun(object):
    """Stand-in for subprocess.run that records argv and never executes."""

    def __init__(self, session_lists=None, has_session_rc=1, raise_on=None):
        self.calls = []
        # Successive stdout values for unscoped `psmux list-sessions`
        # (the default-namespace snapshot). Last value repeats.
        self.session_lists = list(session_lists or [DEFAULT_SESSIONS])
        self.has_session_rc = has_session_rc
        self.raise_on = raise_on

    @property
    def argv_strings(self):
        return [" ".join(c) for c in self.calls]

    def _find(self, argv, name):
        return name in argv

    def __call__(self, argv, *args, **kwargs):
        argv = list(argv)
        self.calls.append(argv)
        if self.raise_on is not None and self._find(argv, self.raise_on):
            raise RuntimeError("probe body blew up on " + self.raise_on)
        returncode = 0
        stdout = ""
        if self._find(argv, "list-sessions"):
            if len(self.session_lists) > 1:
                stdout = self.session_lists.pop(0)
            else:
                stdout = self.session_lists[0]
        elif self._find(argv, "has-session"):
            returncode = self.has_session_rc
        elif self._find(argv, "capture-pane"):
            stdout = "captured pane text\n"
        return subprocess.CompletedProcess(argv, returncode, stdout, "")


class ProbeTestCase(unittest.TestCase):
    def setUp(self):
        self.probe = _load_probe()

    def run_main(self, argv, fake):
        """Call main(argv) with the _run seam stubbed. Returns (rc, fake)."""
        patcher = mock.patch.object(self.probe, "_run", fake)
        patcher.start()
        self.addCleanup(patcher.stop)
        try:
            rc = self.probe.main(argv)
        except SystemExit as exc:
            rc = exc.code if exc.code is not None else 0
        return rc, fake

    def write_payload(self, text):
        tmpdir = tempfile.mkdtemp(prefix="psmux-probe-test-")
        self.addCleanup(_rmtree, tmpdir)
        path = os.path.join(tmpdir, "payload.txt")
        with open(path, "wb") as handle:
            handle.write(text.encode("utf-8"))
        return path

    def assert_no_bare_kill_server(self, fake):
        for argv in fake.calls:
            if "kill-server" in argv:
                self.assertIn(
                    "-L",
                    argv,
                    "bare `psmux kill-server` would destroy every namespace: "
                    + " ".join(argv),
                )

    def kill_server_calls(self, fake):
        return [c for c in fake.calls if "kill-server" in c]


def _rmtree(path):
    import shutil

    shutil.rmtree(path, ignore_errors=True)


class NamespaceIsRequiredAndNeverDefault(ProbeTestCase):
    """(a) The probe must not be able to operate on the user's live default
    namespace: --ns is mandatory, must be non-empty, and must not be
    "default". A refused invocation must not shell out at all."""

    def test_missing_ns_is_refused_and_runs_nothing(self):
        fake = FakeRun()
        rc, fake = self.run_main(["--capture"], fake)
        self.assertNotEqual(rc, 0, "--ns is required; missing --ns must fail")
        self.assertEqual(
            fake.calls, [], "a refused invocation must not invoke psmux at all"
        )

    def test_empty_ns_is_refused_and_runs_nothing(self):
        fake = FakeRun()
        rc, fake = self.run_main(["--ns", "", "--capture"], fake)
        self.assertNotEqual(rc, 0, 'an empty --ns value must be refused')
        self.assertEqual(
            fake.calls, [], "a refused invocation must not invoke psmux at all"
        )

    def test_literal_default_ns_is_refused_and_runs_nothing(self):
        fake = FakeRun()
        rc, fake = self.run_main(["--ns", "default", "--capture"], fake)
        self.assertNotEqual(
            rc, 0, 'the literal namespace "default" must be refused'
        )
        self.assertEqual(
            fake.calls,
            [],
            "refusing --ns default must happen before any psmux invocation",
        )


class EverySessionCommandIsNamespaceScoped(ProbeTestCase):
    """(b) ensure_session works against one implicit session named "probe" via
    has-session / new-session, and every mutating call carries -L <ns>."""

    def test_missing_session_is_created_with_new_session_named_probe(self):
        fake = FakeRun(has_session_rc=1)
        rc, fake = self.run_main(["--ns", TEST_NS, "--capture"], fake)
        self.assertEqual(rc, 0, "a clean capture run must succeed")

        has = [c for c in fake.calls if "has-session" in c]
        new = [c for c in fake.calls if "new-session" in c]
        self.assertEqual(
            len(has), 1, "ensure_session must probe with has-session exactly once"
        )
        self.assertEqual(
            len(new), 1, "a missing session must be created with new-session"
        )
        for argv in has + new:
            self.assertEqual(
                argv[:3],
                ["psmux", "-L", TEST_NS],
                "session commands must be namespace-scoped: " + " ".join(argv),
            )
            self.assertIn(
                "probe",
                argv,
                "the probe uses ONE implicit session named probe: " + " ".join(argv),
            )

    def test_existing_session_is_not_recreated(self):
        fake = FakeRun(has_session_rc=0)
        rc, fake = self.run_main(["--ns", TEST_NS, "--capture"], fake)
        self.assertEqual(rc, 0)
        self.assertEqual(
            [c for c in fake.calls if "new-session" in c],
            [],
            "an existing probe session must not be recreated",
        )


class SendPasteWrapsPayloadInBracketedPaste(ProbeTestCase):
    """(c) --send-paste <file> reads the file's bytes, wraps them in ESC[200~
    ... ESC[201~ and delivers them literally through `send-keys -l`."""

    def test_payload_is_wrapped_and_sent_literally(self):
        payload = "hello paste world"
        path = self.write_payload(payload)
        fake = FakeRun()
        rc, fake = self.run_main(["--ns", TEST_NS, "--send-paste", path], fake)
        self.assertEqual(rc, 0, "a clean paste run must succeed")

        sends = [c for c in fake.calls if "send-keys" in c]
        self.assertEqual(
            len(sends), 1, "--send-paste must issue exactly one send-keys call"
        )
        argv = sends[0]
        self.assertEqual(argv[:3], ["psmux", "-L", TEST_NS])
        self.assertIn(
            "-l", argv, "the wrapped payload must be sent literally (send-keys -l)"
        )
        literal = argv[-1]
        self.assertTrue(
            literal.startswith(PASTE_OPEN),
            "payload must start with the bracketed-paste opener ESC[200~, got "
            + repr(literal),
        )
        self.assertTrue(
            literal.endswith(PASTE_CLOSE),
            "payload must end with the bracketed-paste terminator ESC[201~, got "
            + repr(literal),
        )
        self.assertEqual(
            literal,
            PASTE_OPEN + payload + PASTE_CLOSE,
            "the file's bytes must be passed through verbatim between the "
            "bracketed-paste markers",
        )

    def test_multiline_payload_is_wrapped_once_not_per_line(self):
        payload = "line one\nline two\nline three"
        path = self.write_payload(payload)
        fake = FakeRun()
        rc, fake = self.run_main(["--ns", TEST_NS, "--send-paste", path], fake)
        self.assertEqual(rc, 0)

        sends = [c for c in fake.calls if "send-keys" in c]
        self.assertEqual(
            len(sends),
            1,
            "a multi-line payload is ONE bracketed paste, not one per line",
        )
        literal = sends[0][-1]
        self.assertEqual(literal, PASTE_OPEN + payload + PASTE_CLOSE)
        self.assertEqual(
            literal.count(PASTE_OPEN), 1, "exactly one bracketed-paste opener"
        )
        self.assertEqual(
            literal.count(PASTE_CLOSE), 1, "exactly one bracketed-paste terminator"
        )

    def test_empty_payload_file_still_sends_the_markers(self):
        path = self.write_payload("")
        fake = FakeRun()
        rc, fake = self.run_main(["--ns", TEST_NS, "--send-paste", path], fake)
        self.assertEqual(rc, 0, "an empty payload file is a valid empty paste")
        sends = [c for c in fake.calls if "send-keys" in c]
        self.assertEqual(len(sends), 1)
        self.assertEqual(sends[0][-1], PASTE_OPEN + PASTE_CLOSE)


class SendKeysDeliversKeysUnwrapped(ProbeTestCase):
    """(d) --send-keys delivers key names through send-keys WITHOUT the
    bracketed-paste wrapper (that wrapper is paste-only)."""

    def test_send_keys_is_not_bracketed(self):
        fake = FakeRun()
        rc, fake = self.run_main(["--ns", TEST_NS, "--send-keys", "Enter"], fake)
        self.assertEqual(rc, 0)
        sends = [c for c in fake.calls if "send-keys" in c]
        self.assertEqual(len(sends), 1, "--send-keys must issue one send-keys call")
        argv = sends[0]
        self.assertEqual(argv[:3], ["psmux", "-L", TEST_NS])
        self.assertIn("Enter", argv)
        for part in argv:
            self.assertNotIn(
                PASTE_OPEN, part, "--send-keys must not be bracketed-paste wrapped"
            )
            self.assertNotIn(PASTE_CLOSE, part)


class CaptureRunsCapturePaneDashP(ProbeTestCase):
    """(e) --capture runs `capture-pane -p` in the probe namespace."""

    def test_capture_issues_capture_pane_p(self):
        fake = FakeRun()
        rc, fake = self.run_main(["--ns", TEST_NS, "--capture"], fake)
        self.assertEqual(rc, 0)
        caps = [c for c in fake.calls if "capture-pane" in c]
        self.assertEqual(len(caps), 1, "--capture must issue one capture-pane call")
        argv = caps[0]
        self.assertEqual(argv[:3], ["psmux", "-L", TEST_NS])
        self.assertIn("-p", argv, "capture-pane must use -p to print to stdout")

    def test_no_capture_flag_means_no_capture_pane(self):
        fake = FakeRun()
        rc, fake = self.run_main(["--ns", TEST_NS, "--send-keys", "Enter"], fake)
        self.assertEqual(rc, 0)
        self.assertEqual(
            [c for c in fake.calls if "capture-pane" in c],
            [],
            "capture-pane must only run when --capture is given",
        )


class CleanupAlwaysKillsOnlyTheTestNamespace(ProbeTestCase):
    """(f) Teardown is namespace-scoped and unconditional: `psmux -L <ns>
    kill-server` runs in a finally block even when the body raises, and a bare
    `psmux kill-server` is never issued."""

    def test_cleanup_kills_the_namespace_on_a_clean_run(self):
        fake = FakeRun()
        rc, fake = self.run_main(["--ns", TEST_NS, "--capture"], fake)
        self.assertEqual(rc, 0)
        kills = self.kill_server_calls(fake)
        self.assertEqual(len(kills), 1, "cleanup must kill the namespace exactly once")
        self.assertEqual(
            kills[0][:3],
            ["psmux", "-L", TEST_NS],
            "kill-server must be namespace-scoped",
        )
        self.assertEqual(
            kills[0],
            ["psmux", "-L", TEST_NS, "kill-server"],
            "cleanup argv must be exactly `psmux -L <ns> kill-server`",
        )
        self.assert_no_bare_kill_server(fake)

    def test_cleanup_still_runs_when_the_body_raises(self):
        fake = FakeRun(raise_on="capture-pane")
        rc, fake = self.run_main(["--ns", TEST_NS, "--capture"], fake)
        self.assertNotEqual(rc, 0, "a failed probe body must report failure")
        kills = self.kill_server_calls(fake)
        self.assertEqual(
            len(kills),
            1,
            "kill-server must run from a finally block even when the body raises",
        )
        self.assertEqual(kills[0], ["psmux", "-L", TEST_NS, "kill-server"])
        self.assert_no_bare_kill_server(fake)

    def test_no_invocation_is_ever_a_bare_kill_server(self):
        fake = FakeRun()
        rc, fake = self.run_main(["--ns", TEST_NS, "--capture"], fake)
        self.assertEqual(rc, 0)
        self.assertNotIn(
            "psmux kill-server",
            fake.argv_strings,
            "a bare `psmux kill-server` kills every namespace and is forbidden",
        )


class KeepSuppressesCleanup(ProbeTestCase):
    """(g) --keep leaves the probe namespace alive for inspection."""

    def test_keep_issues_no_kill_server(self):
        fake = FakeRun()
        rc, fake = self.run_main(["--ns", TEST_NS, "--capture", "--keep"], fake)
        self.assertEqual(rc, 0)
        self.assertEqual(
            self.kill_server_calls(fake),
            [],
            "--keep must suppress the kill-server teardown",
        )

    def test_keep_still_suppresses_cleanup_when_the_body_raises(self):
        fake = FakeRun(raise_on="capture-pane")
        rc, fake = self.run_main(["--ns", TEST_NS, "--capture", "--keep"], fake)
        self.assertNotEqual(rc, 0)
        self.assertEqual(
            self.kill_server_calls(fake),
            [],
            "--keep must suppress teardown on the failure path too",
        )


class DefaultNamespaceSnapshotGuard(ProbeTestCase):
    """(h) The default namespace's session list is snapshotted before and
    after the run; any difference aborts with a nonzero exit."""

    def test_default_namespace_is_snapshotted_before_and_after(self):
        fake = FakeRun(session_lists=[DEFAULT_SESSIONS])
        rc, fake = self.run_main(["--ns", TEST_NS, "--capture"], fake)
        self.assertEqual(rc, 0)
        snaps = [c for c in fake.calls if "list-sessions" in c and "-L" not in c]
        self.assertEqual(
            len(snaps),
            2,
            "the DEFAULT namespace session list must be read before AND after",
        )
        self.assertEqual(snaps[0], snaps[1], "both snapshots use the same argv")
        self.assertNotIn(
            "-L",
            snaps[0],
            "the safety snapshot must read the DEFAULT namespace, unscoped",
        )

    def test_changed_default_session_list_aborts(self):
        fake = FakeRun(
            session_lists=[
                DEFAULT_SESSIONS,
                DEFAULT_SESSIONS + "1: 1 windows (created Sat Sep 20 10:05:00 2026)\n",
            ]
        )
        rc, fake = self.run_main(["--ns", TEST_NS, "--capture"], fake)
        self.assertNotEqual(
            rc,
            0,
            "a changed default-namespace session list means the probe touched "
            "the user's live sessions and must abort nonzero",
        )

    def test_default_session_disappearing_also_aborts(self):
        fake = FakeRun(session_lists=[DEFAULT_SESSIONS, ""])
        rc, fake = self.run_main(["--ns", TEST_NS, "--capture"], fake)
        self.assertNotEqual(
            rc, 0, "a default session vanishing during the run must abort nonzero"
        )


class RunSeamIsExposedForStubbing(ProbeTestCase):
    """(i) The module exposes `_run` bound to subprocess.run so tests (and
    AGENTS.md test isolation) can intercept every child process."""

    def test_module_exposes_run_seam_defaulting_to_subprocess_run(self):
        self.assertTrue(
            hasattr(self.probe, "_run"),
            "psmux_tui_probe must expose a module-level _run seam",
        )
        self.assertIs(
            self.probe._run,
            subprocess.run,
            "_run must default to subprocess.run",
        )


if __name__ == "__main__":
    unittest.main()
