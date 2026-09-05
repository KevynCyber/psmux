# Covers: ZDEP-012
# Requirement: scripts/gen_unicode_width.py exposes generate(fixture_path) ->
# str, which reads the committed oracle fixture
# tests-rs/fixtures/unicode_width_0.2.2.txt (run-length rows
# <start-hex>..<end-hex>|<N|0|1|2|3>) and returns the full source text of
# crates/psmux-unicode/src/tables.rs. That module defines
# `pub(crate) const WIDTHS: &[(u32, u32, u8)]`, a table of (start, end, code)
# triples sorted ascending by start, where code is 255 for None width, else
# the literal width value 0, 1, 2, or 3 (rows whose fixture width is 1 MAY be
# omitted from the table entirely, since 1 is the implementation's default
# for any code point not covered by a row). Run with:
#   python -m unittest scripts/test_gen_unicode_width.py
# from the worktree root.

import importlib.util
import os
import unittest

SCRIPTS_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(SCRIPTS_DIR)


def _load_generator():
    """Import gen_unicode_width.py from the scripts dir by file path."""
    module_path = os.path.join(SCRIPTS_DIR, "gen_unicode_width.py")
    spec = importlib.util.spec_from_file_location("gen_unicode_width", module_path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class RegeneratesCommittedTablesByteForByte(unittest.TestCase):
    """(a) generate() on the real committed fixture must reproduce the
    committed crates/psmux-unicode/src/tables.rs byte-for-byte."""

    def test_generate_matches_committed_tables_rs(self):
        gen = _load_generator()
        fixture_path = os.path.join(
            REPO_ROOT, "tests-rs", "fixtures", "unicode_width_0.2.2.txt"
        )
        tables_path = os.path.join(
            REPO_ROOT, "crates", "psmux-unicode", "src", "tables.rs"
        )
        with open(tables_path, "r", encoding="utf-8", newline="") as f:
            committed = f.read()

        actual = gen.generate(fixture_path)

        self.assertEqual(
            actual,
            committed,
            "generate(fixture_path) must reproduce tables.rs byte-for-byte; "
            "tables.rs must always be the committed fixture's output, never "
            "hand-edited",
        )


class SyntheticFixtureProducesEachRangeOnce(unittest.TestCase):
    """(b) A minimal synthetic 3-row fixture must produce a table string
    containing each range exactly once."""

    def test_three_row_fixture_yields_each_range_once(self):
        gen = _load_generator()
        synthetic = (
            "# synthetic 3-row fixture for gen_unicode_width contract test\n"
            "0000..001F|N\n"
            "0020..007E|1\n"
            "4E00..4E00|2\n"
        )
        tmp_path = os.path.join(SCRIPTS_DIR, "_synthetic_fixture_for_test.txt")
        with open(tmp_path, "w", encoding="utf-8", newline="\n") as f:
            f.write(synthetic)
        try:
            table_src = gen.generate(tmp_path)
        finally:
            os.remove(tmp_path)

        self.assertIn("WIDTHS", table_src, "generated source must define WIDTHS")

        # The None row must appear once, with code 255.
        self.assertEqual(
            table_src.count("(0x0000, 0x001F, 255)"),
            1,
            "the None-width range 0000..001F must appear exactly once, coded 255",
        )
        # The wide row must appear once, with code 2.
        self.assertEqual(
            table_src.count("(0x4E00, 0x4E00, 2)"),
            1,
            "the width-2 range 4E00..4E00 must appear exactly once",
        )
        # The width-1 row MAY be omitted (1 is the default); it must not
        # appear as a spurious duplicate if the generator chooses to include
        # it either -- there must be at most one occurrence.
        self.assertLessEqual(
            table_src.count("0x0020, 0x007E"),
            1,
            "the width-1 range 0020..007E must not be duplicated",
        )


if __name__ == "__main__":
    unittest.main()
