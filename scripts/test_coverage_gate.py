"""Black-box tests for scripts/coverage-gate.py: run the script the way a
developer does, against a synthetic source file, LCOV report and llvm-cov JSON.

Run: python3 -m unittest discover -s scripts -p 'test_*.py'
"""

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

GATE = pathlib.Path(__file__).with_name("coverage-gate.py")


def run_gate(source: str, uncovered_line: int) -> subprocess.CompletedProcess:
    """Gate a one-file crate whose line `uncovered_line` was never executed."""
    with tempfile.TemporaryDirectory() as tmp:
        root = pathlib.Path(tmp)
        src = root / "crate" / "src" / "lib.rs"  # in_scope() keys on "/src/"
        src.parent.mkdir(parents=True)
        src.write_text(source, encoding="utf-8")
        n = len(source.splitlines())
        da = "".join(
            f"DA:{ln},{0 if ln == uncovered_line else 1}\n" for ln in range(1, n + 1)
        )
        lcov = root / "lcov.info"
        lcov.write_text(f"SF:{src}\n{da}end_of_record\n", encoding="utf-8")
        cov = root / "cov.json"
        cov.write_text(json.dumps({"data": [{"functions": []}]}), encoding="utf-8")
        return subprocess.run(
            [sys.executable, str(GATE), str(cov), str(lcov)],
            capture_output=True,
            text=True,
        )


def dead_arm(marker_comment: str) -> str:
    """A function whose `return 1` arm is the uncovered line 4."""
    return (
        "fn f(x: u8) -> u8 {\n"  # 1
        "    if x > 9 {\n"  # 2
        f"        {marker_comment}\n"  # 3
        "        return 1;\n"  # 4  <- uncovered
        "    }\n"  # 5
        "    0\n"  # 6
        "}\n"  # 7
    )


class MarkerReason(unittest.TestCase):
    def test_a_justified_marker_exempts_the_dead_line(self):
        r = run_gate(dead_arm("// cov:unreachable: x is a BCD digit, so x <= 9"), 4)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)

    def test_a_bare_marker_does_not_exempt(self):
        # The marker asserts a guard cannot be reached; with no stated reason the
        # assertion cannot be reviewed, so it must not turn the gate green.
        r = run_gate(dead_arm("// cov:unreachable"), 4)
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("bare", r.stdout)

    def test_a_reason_before_the_colon_is_still_bare(self):
        # The form that once passed this script yet failed the fleet gate: the
        # fleet regex wants the reason after a colon DIRECTLY following the marker.
        r = run_gate(dead_arm("// cov:unreachable (the arm below): x <= 9"), 4)
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("bare", r.stdout)

    def test_an_unmarked_uncovered_line_still_fails(self):
        # Control: the pre-existing behaviour is unchanged.
        r = run_gate(dead_arm("// just a comment"), 4)
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)


if __name__ == "__main__":
    unittest.main()
