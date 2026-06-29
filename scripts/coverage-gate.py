#!/usr/bin/env python3
"""Function-coverage gate that honors `// cov:unreachable` markers.

`cargo llvm-cov --fail-under-functions 100` cannot exempt individual
functions, but defensive arms that are provably unreachable under a current
invariant should be *kept* (Coverage discipline) rather than tested into
existence. This gate fails only on functions that are BOTH uncovered AND lack a
`// cov:unreachable` marker on (or just above) their first line.

Usage:
    cargo llvm-cov --all-features --json --output-path cov.json
    python3 scripts/coverage-gate.py cov.json

A function is "covered" if ANY instantiation has a non-zero count (the merged
view), matching how llvm-cov reports per-file summaries.
"""
import json
import sys
from collections import defaultdict

MARKER = "cov:unreachable"


def main() -> int:
    cov_path = sys.argv[1] if len(sys.argv) > 1 else "cov.json"
    data = json.load(open(cov_path, encoding="utf-8"))
    funcs = data["data"][0]["functions"]

    # Merge instantiations by SOURCE LOCATION, not symbol name: llvm-cov embeds
    # the crate-disambiguator hash in each mangled name, so the same source
    # function compiled into different test binaries has different names but the
    # same (file, line, col). Covered if ANY instantiation at that location ran.
    merged = defaultdict(lambda: {"count": 0, "file": None, "line": None})
    for f in funcs:
        files = f.get("filenames") or ["?"]
        file = files[0]
        regions = f.get("regions", [])
        if not regions:
            continue
        start = min(regions, key=lambda r: (r[0], r[1]))
        line, col = start[0], start[1]
        key = (file, line, col)
        m = merged[key]
        m["count"] += f["count"]
        m["file"] = file
        m["line"] = line

    src_cache: dict[str, list[str]] = {}

    def marked(file: str, line: int) -> bool:
        # The marker may sit on the function/closure line or the line above.
        if file not in src_cache:
            try:
                src_cache[file] = open(file, encoding="utf-8").read().splitlines()
            except OSError:
                src_cache[file] = []
        lines = src_cache[file]
        # Check the closure/function line and up to two lines above it, so a
        # short multi-line `// cov:unreachable: …` comment block counts.
        for ln in (line - 2, line - 1, line):
            if 0 <= ln - 1 < len(lines) and MARKER in lines[ln - 1]:
                return True
        return False

    failing, exempted = [], []
    for (file, line, _col), m in merged.items():
        if m["count"] > 0:
            continue
        # Only gate the crate's own LIBRARY source (skip tests/ and deps). main.rs
        # is the Humble-Object CLI shell — exercised by tests/cli.rs but not gated
        # to 100% (Coverage discipline: gate the testable library, not thin glue).
        if "/src/" not in file or "/tests/" in file or file.endswith("/main.rs"):
            continue
        short = file.split("/src/")[-1]
        if marked(file, line):
            exempted.append(f"{short}:{line}")
        else:
            failing.append(f"{short}:{line}")

    if exempted:
        print(f"cov:unreachable exemptions ({len(exempted)}):")
        for e in sorted(exempted):
            print(f"  - {e}")
    if failing:
        print(f"\nUNCOVERED functions without a `// cov:unreachable` marker ({len(failing)}):")
        for x in sorted(failing):
            print(f"  ✗ {x}")
        print("\nFAIL: cover these, or annotate provably-dead arms with `// cov:unreachable: <invariant>`.")
        return 1
    print("\nOK: every uncovered function is an annotated, provably-unreachable arm.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
