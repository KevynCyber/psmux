# Covers: AGENTS.md Test Isolation (no docs/features spec ID exists for this)
# Requirement: the latency scripts tests/test_e2e_latency.ps1,
# tests/test_typing_render_latency.ps1,
# tests/test_render_pressure_input_latency.ps1 and scripts/lat-probe.ps1 must
# never create, drive or kill sessions in the user's DEFAULT psmux namespace.
# Each script declares a $Namespace parameter (alias -L) that defaults to a
# generated unique name; every psmux invocation it makes (`& $exe ...`,
# `& psmux ...`, `Start-Process -FilePath $exe -ArgumentList ...`) passes
# `-L <ns>`; the only permitted un-namespaced invocation is a read-only
# `ls`/`list-sessions` snapshot of the default namespace (AGENTS.md mandates
# snapshotting it before/after); `kill-server` only ever appears as
# `-L <ns> kill-server` (a bare kill-server kills EVERY namespace); and
# server port/key files are looked up by the namespaced `<ns>__<session>`
# basename. scripts/lat-probe.ps1 also takes -Exe so a baseline binary and a
# new build can be compared. Static analysis only; no script is executed.
# Run with: python -m pytest scripts/test_latency_scripts_namespace.py

import os
import re
import unittest

SCRIPTS_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(SCRIPTS_DIR)

SCRIPT_PATHS = [
    os.path.join(REPO_ROOT, "tests", "test_e2e_latency.ps1"),
    os.path.join(REPO_ROOT, "tests", "test_typing_render_latency.ps1"),
    os.path.join(REPO_ROOT, "tests", "test_render_pressure_input_latency.ps1"),
    os.path.join(REPO_ROOT, "scripts", "lat-probe.ps1"),
]

# A PowerShell variable that holds the psmux/tmux executable path.
_EXE_VAR = r"\$(?:\w*(?:psmux|pmux|tmux)\w*|Exe)\b"
_CALL_OP = re.compile(
    r"&\s*(?:" + _EXE_VAR + r"|(?:psmux|pmux|tmux)(?:\.exe)?\b)", re.IGNORECASE
)
_BARE_CMD = re.compile(r"^\s*(?:psmux|pmux|tmux)(?:\.exe)?\s", re.IGNORECASE)
_START_PROCESS = re.compile(
    r"Start-Process\b.*-FilePath\s+" + _EXE_VAR, re.IGNORECASE
)
_HAS_L = re.compile(r"""(?<![\w-])["']?-L["']?(?![\w-])""")
_READ_ONLY_DEFAULT = re.compile(
    r"&\s*" + _EXE_VAR + r"\s+(?:ls|list-sessions)\b(?!\s+-L)", re.IGNORECASE
)


def _logical_lines(text):
    """Code lines with backtick continuations joined and comments dropped."""
    out = []
    buf = ""
    start = 0
    in_block_comment = False
    for idx, raw in enumerate(text.splitlines(), 1):
        line = raw.rstrip()
        if in_block_comment:
            if "#>" in line:
                in_block_comment = False
            continue
        if line.lstrip().startswith("<#"):
            in_block_comment = "#>" not in line
            continue
        if line.lstrip().startswith("#"):
            continue
        if not buf:
            start = idx
        if line.endswith("`"):
            buf += line[:-1] + " "
            continue
        out.append((start, buf + line))
        buf = ""
    if buf:
        out.append((start, buf))
    return out


def _read(path):
    with open(path, "r", encoding="utf-8-sig") as fh:
        return fh.read()


def _invocations(text):
    found = []
    for lineno, line in _logical_lines(text):
        if _CALL_OP.search(line) or _BARE_CMD.search(line) or _START_PROCESS.search(line):
            found.append((lineno, line.strip()))
    return found


class LatencyScriptsStayOutOfDefaultNamespace(unittest.TestCase):
    def _check_each_script(self, check):
        for path in SCRIPT_PATHS:
            rel = os.path.relpath(path, REPO_ROOT)
            with self.subTest(script=rel):
                self.assertTrue(os.path.isfile(path), "missing script: %s" % rel)
                check(rel, _read(path))

    def test_detector_finds_psmux_invocations_in_every_script(self):
        def check(rel, text):
            self.assertTrue(
                _invocations(text),
                "%s: no psmux invocation detected; the analysis would pass vacuously" % rel,
            )

        self._check_each_script(check)

    def test_every_psmux_invocation_passes_dash_L(self):
        def check(rel, text):
            offenders = [
                "%s:%d: %s" % (rel, n, line)
                for n, line in _invocations(text)
                if not _HAS_L.search(line) and not _READ_ONLY_DEFAULT.search(line)
            ]
            self.assertEqual(
                offenders, [], "psmux invocations without -L <ns>:\n" + "\n".join(offenders)
            )

        self._check_each_script(check)

    def test_kill_server_is_always_namespace_scoped(self):
        def check(rel, text):
            offenders = [
                "%s:%d: %s" % (rel, n, line.strip())
                for n, line in _logical_lines(text)
                if re.search(r"\bkill-server\b", line) and not _HAS_L.search(line)
            ]
            self.assertEqual(
                offenders, [], "bare kill-server (kills every namespace):\n" + "\n".join(offenders)
            )

        self._check_each_script(check)

    def test_declares_namespace_param_with_generated_default(self):
        def check(rel, text):
            m = re.search(r"\bparam\s*\((.*?)\n\)", text, re.IGNORECASE | re.DOTALL)
            self.assertIsNotNone(m, "%s: no top-level param() block" % rel)
            block = m.group(1)
            self.assertRegex(
                block,
                r"(?i)\[Alias\(\s*['\"]L['\"]\s*\)\]\s*\[string\]\s*\$Namespace\s*=\s*\S",
                "%s: param block lacks [Alias('L')] [string]$Namespace = <generated default>" % rel,
            )

        self._check_each_script(check)

    def test_port_and_key_files_use_namespaced_basename(self):
        def check(rel, text):
            # A variable assigned a "..__.." string (e.g. $base = "${ns}__${sess}")
            # also counts as a namespaced basename.
            ns_vars = set(re.findall(r"\$(\w+)\s*=\s*\"[^\"]*__", text))

            def namespaced(line):
                if "__" in line:
                    return True
                refs = re.findall(r"\$\{?(\w+)\}?\.(?:port|key)\b", line)
                return bool(refs) and all(r in ns_vars for r in refs)

            offenders = [
                "%s:%d: %s" % (rel, n, line.strip())
                for n, line in _logical_lines(text)
                if re.search(r"\.(?:port|key)\b[\"']", line) and not namespaced(line)
            ]
            self.assertEqual(
                offenders,
                [],
                "port/key file lookups not using <ns>__<session>:\n" + "\n".join(offenders),
            )

        self._check_each_script(check)


class LatProbeComparesBuilds(unittest.TestCase):
    def test_lat_probe_takes_exe_param(self):
        path = SCRIPT_PATHS[-1]
        self.assertTrue(os.path.isfile(path), "missing script: scripts/lat-probe.ps1")
        m = re.search(r"\bparam\s*\((.*?)\n\)", _read(path), re.IGNORECASE | re.DOTALL)
        self.assertIsNotNone(m, "scripts/lat-probe.ps1: no param() block")
        self.assertRegex(m.group(1), r"(?i)\[string\]\s*\$Exe\b")


class DetectorSelfCheck(unittest.TestCase):
    def test_flags_unscoped_and_accepts_scoped_forms(self):
        bad = [
            "& $PSMUX send-keys -t $SESSION C-c",
            "try { & $psmuxExe kill-server -t $s 2>$null } catch {}",
            '$p = Start-Process -FilePath $PSMUX -ArgumentList "new-session","-s",$S -PassThru',
            "psmux new-session -d -s x",
        ]
        good = [
            "& $PSMUX -L $ns send-keys -t $SESSION C-c",
            '$p = Start-Process -FilePath $PSMUX -ArgumentList "-L",$ns,"new-session" -PassThru',
            "& $Exe -L $ns kill-server 2>&1 | Out-Null",
        ]
        for line in bad:
            with self.subTest(bad=line):
                inv = _invocations(line)
                self.assertTrue(inv)
                self.assertIsNone(_HAS_L.search(inv[0][1]))
        for line in good:
            with self.subTest(good=line):
                inv = _invocations(line)
                self.assertTrue(inv)
                self.assertIsNotNone(_HAS_L.search(inv[0][1]))
        self.assertIsNotNone(_READ_ONLY_DEFAULT.search("$before = (& $Exe ls 2>&1 | Out-String)"))
        self.assertIsNone(_READ_ONLY_DEFAULT.search("& $Exe kill-server"))


if __name__ == "__main__":
    unittest.main()
