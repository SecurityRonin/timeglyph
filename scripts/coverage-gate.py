#!/usr/bin/env python3
"""Coverage gate: 100% LINE **and** function coverage, honoring `// cov:unreachable`.

The fleet standard (ADR-0008 "Coverage gate") is **100% line coverage** — "fail on
any `DA:n,0`" — *except* lines annotated `// cov:unreachable`. `cargo llvm-cov
--fail-under-lines 100` cannot exempt individual lines, and defensive arms that
are provably unreachable under a dominating invariant must be *kept* (Coverage
discipline: a backstop, not a target) rather than deleted or tested into
existence. So this gate fails on any uncovered line — and any uncovered function —
that lacks a `// cov:unreachable` marker in its dead region.

Usage:
    cargo llvm-cov --all-features --json --output-path cov.json
    cargo llvm-cov report --lcov --output-path lcov.info
    python3 scripts/coverage-gate.py cov.json lcov.info

Both inputs are required: the LCOV file carries per-line hit counts (`DA:`), the
JSON carries per-function counts. A missing input is a bootstrap failure and
exits non-zero rather than silently gating on nothing.

Scope (both gates): the crate's own `**/src/**` sources, excluding `tests/` and
`main.rs` — the Humble-Object CLI shell is exercised by `tests/cli.rs` but not
gated to 100% (decisions belong in the testable library, not thin glue).

Marker placement:
  * a **function** marker sits on the `fn`/closure line or up to 2 lines above it
    (so a short `// cov:unreachable: …` comment above the signature counts);
  * a **line** marker sits on the uncovered line itself or anywhere above it
    *within the same dead region*, so one annotation documents one contiguous
    dead block (a multi-line `return Err(…)` arm needs a single marker, not one
    per line). The upward search stops at the first covered line, at any
    unmapped line that is not blank/comment, and at a `fn` signature — a marker
    can never leak into a neighbouring function or a live branch.
"""
import json
import re
import sys
from collections import defaultdict

MARKER = "cov:unreachable"

# A function signature bounds the upward marker search: a marker in one dead
# function must not exempt lines in the next one.
FN_SIG = re.compile(
    r"^\s*(pub\s*(\([^)]*\)\s*)?)?(default\s+)?(const\s+)?(async\s+)?(unsafe\s+)?"
    r"(extern\s+\"[^\"]*\"\s+)?fn\s"
)


def in_scope(file: str) -> bool:
    """Gate the crate's own library sources only."""
    return "/src/" in file and "/tests/" not in file and not file.endswith("/main.rs")


class Source:
    """Lazily-read source lines, so a marker lookup never re-reads a file."""

    def __init__(self) -> None:
        self.cache: dict[str, list[str]] = {}

    def lines(self, file: str) -> list[str]:
        if file not in self.cache:
            try:
                self.cache[file] = open(file, encoding="utf-8").read().splitlines()
            except OSError:
                self.cache[file] = []
        return self.cache[file]

    def text(self, file: str, line: int) -> str:
        lines = self.lines(file)
        return lines[line - 1] if 0 <= line - 1 < len(lines) else ""

    def marked_at(self, file: str, line: int) -> bool:
        return MARKER in self.text(file, line)


def load_lines(lcov_path: str) -> dict[str, dict[int, int]]:
    """`{file: {line: hit_count}}` from an LCOV report."""
    per_file: dict[str, dict[int, int]] = defaultdict(dict)
    current = None
    with open(lcov_path, encoding="utf-8") as fh:
        for raw in fh:
            if raw.startswith("SF:"):
                current = raw[3:].strip()
            elif raw.startswith("DA:") and current is not None:
                fields = raw[3:].strip().split(",")
                line, count = int(fields[0]), int(fields[1])
                # Keep the highest count: llvm-cov emits one DA per instantiation.
                per_file[current][line] = max(per_file[current].get(line, 0), count)
    return per_file


def gate_functions(cov_path: str, src: Source) -> tuple[list[str], list[str]]:
    data = json.load(open(cov_path, encoding="utf-8"))
    export = data["data"][0]
    if "functions" not in export:
        # `--summary-only` drops the per-function array, so the gate would silently
        # measure nothing. Name the cause instead of degrading to a pass.
        raise SystemExit(
            f"FAIL: {cov_path!r} has no per-function data (keys: {sorted(export)}). "
            "Generate it WITHOUT `--summary-only`."
        )
    funcs = export["functions"]

    # Merge instantiations by SOURCE LOCATION, not symbol name: llvm-cov embeds
    # the crate-disambiguator hash in each mangled name, so the same source
    # function compiled into different test binaries has different names but the
    # same (file, line, col). Covered if ANY instantiation at that location ran.
    merged: dict[tuple[str, int, int], int] = defaultdict(int)
    for f in funcs:
        regions = f.get("regions", [])
        if not regions:
            continue
        file = (f.get("filenames") or ["?"])[0]
        start = min(regions, key=lambda r: (r[0], r[1]))
        merged[(file, start[0], start[1])] += f["count"]

    failing, exempted = [], []
    for (file, line, _col), count in merged.items():
        if count > 0 or not in_scope(file):
            continue
        short = f"{file.split('/src/')[-1]}:{line}"
        # The marker may sit on the function/closure line or up to two lines
        # above it, so a short `// cov:unreachable: …` comment block counts.
        if any(src.marked_at(file, ln) for ln in (line - 2, line - 1, line)):
            exempted.append(short)
        else:
            failing.append(short)
    return failing, exempted


def gate_lines(lcov_path: str, src: Source) -> tuple[list[str], list[str]]:
    failing, exempted = [], []
    for file, hits in load_lines(lcov_path).items():
        if not in_scope(file):
            continue
        mapped = set(hits)
        covered = {ln for ln, c in hits.items() if c > 0}
        for line in sorted(ln for ln, c in hits.items() if c == 0):
            found = False
            for probe in range(line, 0, -1):
                if src.marked_at(file, probe):
                    found = True
                    break
                text = src.text(file, probe).strip()
                if FN_SIG.match(text):
                    break  # reached this function's signature: region ends here
                in_region = probe in mapped and probe not in covered
                filler = probe not in mapped and (not text or text.startswith("//"))
                if not (in_region or filler):
                    break  # a covered line, or unmapped code: not the same dead block
            short = f"{file.split('/src/')[-1]}:{line}"
            (exempted if found else failing).append(short)
    return failing, exempted


def report(kind: str, failing: list[str], exempted: list[str]) -> None:
    if exempted:
        print(f"cov:unreachable {kind} exemptions ({len(exempted)}):")
        for e in sorted(exempted):
            print(f"  - {e}")
    if failing:
        print(f"\nUNCOVERED {kind} without a `// cov:unreachable` marker ({len(failing)}):")
        for x in sorted(failing):
            print(f"  x {x}")


def main() -> int:
    cov_path = sys.argv[1] if len(sys.argv) > 1 else "cov.json"
    lcov_path = sys.argv[2] if len(sys.argv) > 2 else "lcov.info"
    for path, what in ((cov_path, "llvm-cov JSON"), (lcov_path, "LCOV report")):
        try:
            open(path, encoding="utf-8").close()
        except OSError as exc:
            # Bootstrap failure: never degrade to "nothing to gate".
            print(f"FAIL: cannot read the {what} at {path!r}: {exc}")
            return 2

    src = Source()
    fn_fail, fn_exempt = gate_functions(cov_path, src)
    ln_fail, ln_exempt = gate_lines(lcov_path, src)

    report("functions", fn_fail, fn_exempt)
    print()
    report("lines", ln_fail, ln_exempt)

    if fn_fail or ln_fail:
        print(
            "\nFAIL: cover these, or annotate provably-dead defensive arms with "
            "`// cov:unreachable: <the dominating invariant>`.\n"
            "Never delete or restructure a defensive guard to turn a line green (ADR-0008)."
        )
        return 1
    print("\nOK: 100% line + function coverage, modulo annotated provably-unreachable arms.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
