#!/usr/bin/env sh
# Stamp the Python bindings with the crate version — single source of truth is
# the root Cargo.toml [package] version. release-plz bumps ONLY the root crate on
# a release; this propagates that one bump into bindings/python/{Cargo,pyproject}.toml
# so the published wheel version always equals the crate version. Drift is made
# impossible by construction rather than merely detected after the fact (the old
# equality guard could never pass on a release-plz PR, which bumps only the crate).
# Portable POSIX sh + awk so it runs identically on the ubuntu/macOS/windows wheel
# matrix. Run from anywhere: paths are resolved relative to this script.
set -eu

here=$(dirname -- "$0")
here=$(cd -- "$here" && pwd)
ver=$(grep -m1 '^version' "$here/../../Cargo.toml" | cut -d'"' -f2)
[ -n "$ver" ] || { echo "sync-version: could not read crate version" >&2; exit 1; }

for f in Cargo.toml pyproject.toml; do
  p="$here/$f"
  awk -v v="$ver" '!d && /^version = "/ { sub(/"[^"]*"/, "\"" v "\""); d=1 } 1' "$p" > "$p.tmp"
  mv "$p.tmp" "$p"
done

echo "sync-version: bindings/python stamped to crate version $ver"
