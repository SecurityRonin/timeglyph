# HANDOFF — Release timeglyph 0.4.0

**Status:** `main` is at **0.4.0**, fixed and CI-green, but **0.4.0 is NOT published** (crates.io latest is `0.3.0`). This handoff is the remaining work to ship it.

## Context (what already happened)
The fleet completed the **knowledge/engine split** for timestamps:
- `forensicnomicon 1.3.0` now owns the timestamp-format catalog (`temporal_formats::TIME_FORMATS`, 45 formats) — **published to crates.io**.
- timeglyph is now the **decoder** that consumes it: `registry.rs` sources `FORMATS` from `forensicnomicon::temporal_formats`; the 16 packed calendar codecs stay engine-side (`packed_codec(PackedLayout)`). The independent MIT `time-decode` oracle re-passed for all 45 constants (incl. negative pre-1970 epochs); the registry digest is byte-identical (behavior-preserving).
- `main` was hot-fixed: the temporary `[patch.crates-io] forensicnomicon = { path = "../forensicnomicon" }` was **removed** (commit `9ef8b6d`); timeglyph now resolves `forensicnomicon = "1.3"` from crates.io. `cargo check --all-features` passes; CI is green.
- **Breaking change (0.3 → 0.4):** field reads moved to `Format.meta.*` (via `Deref`); `Strategy` → `Encoding` (re-exported from forensicnomicon); `FORMATS` is now `LazyLock<Vec<Format>>`. CHANGELOG updated.

## Do this to release 0.4.0

### 1. Fix the non-blocking `freshness` advisory first
The `freshness` CI job runs `cargo update --locked` and currently **fails** — it's a dependency-freshness gate (fails when the lockfile is behind the latest versions allowed by `Cargo.toml`). It does **not** block the required checks (overall CI is green), but clean it up before tagging:

```bash
export GITSIGN_CREDENTIAL_CACHE="$HOME/Library/Caches/sigstore/gitsign/cache.sock"
cd ~/src/timeglyph
git checkout main && git pull
cargo update                     # refresh Cargo.lock to latest-allowed
cargo test --all-features        # confirm still green after the bump
git commit -S -am "chore(deps): cargo update — satisfy freshness gate"
git push
```
Confirm the `freshness` job goes green on the resulting CI run.

### 2. Release via the tag-driven pipeline (NOT release-plz, NOT hand-publish)
timeglyph releases on a **`v*` tag** — `.github/workflows/release.yml` (trigger `push: tags: ["v*"]`) is the only thing that publishes the crate + builds binaries; `python-wheel-release.yml` builds the wheels. Do **not** `cargo publish` by hand.

```bash
# Confirm the version is 0.4.0 and CHANGELOG has a 0.4.0 entry, then:
git tag -s v0.4.0 -m "timeglyph 0.4.0 — consume the forensicnomicon timestamp catalog (knowledge/engine split)"
git push origin v0.4.0
```
The tag **must be signed** (`-s`, gitsign) per fleet policy.

### 3. Verify the release actually shipped (don't assume)
```bash
gh run watch -R SecurityRonin/timeglyph   # watch the release run to completion
```
- The build matrix is fail-fast: one failed target skips the whole `release` job → a tag with no binaries. Watch it.
- Confirm **crates.io shows 0.4.0**: `curl -sS https://index.crates.io/ti/me/timeglyph | tail -1` should include `"vers":"0.4.0"`.
- Confirm the GitHub Release has binaries + wheels.

## Prerequisites (already satisfied)
- ✅ `forensicnomicon 1.3.0` is on crates.io (the `version = "1.3"` dep resolves).
- ✅ `main` has no `[patch.crates-io]` block.
- ✅ `main` CI green; commits signed.
- ✅ crates.io token configured (`~/.cargo/credentials.toml` `[registry]`), so the release job / any publish is credentialed.

## Gotchas
- If the release job errors with `E0463: can't find crate for core`, the cross-build toolchain in `release.yml` doesn't match the `rust-toolchain.toml` pin — align them.
- Do not re-publish an existing version (0.4.0 must be new on crates.io — it is).
- Related merged PRs for reference: forensicnomicon #10 (catalog) and its release-plz release (1.3.0), timeglyph #1 (consume the catalog), plus the `9ef8b6d` patch-drop hotfix on `main`.

_Delete this file once 0.4.0 is published and verified._
