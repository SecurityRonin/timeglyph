# Changelog

All notable changes to `timeglyph` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Format catalog**: PostgreSQL (µs since 2000), Unix nanoseconds, Cocoa
  `CFAbsoluteTime` as a signed double, SQLite Julian-day float, and Snowflake IDs
  (Twitter/X and Discord) — each with a primary-spec citation and spec-anchored
  tests.
- **`EmbeddedMillis` strategy** for ID schemes that embed a millisecond timestamp
  in their high bits (`value >> shift_bits`).
- **Component-based plausibility scoring** (`representable`, `in_window`,
  `granularity_match`, `magnitude_fit`) emitted on every candidate so the rank is
  auditable. `granularity_match` resolves the seconds-vs-ms-vs-µs-vs-ns ambiguity
  via trailing-zero analysis; `magnitude_fit` sinks epoch-hugging false ID reads.
- **Epistemic framing**: candidates are described as *consistent with* a format
  (never "detected"), and POSIX readings carry a leap-smear disclaimer.
- **Real `--json`** output via `serde::Serialize` on `Candidate`/`PosixNs`.
- **Fleet standards**: Apache-2.0 `LICENSE`, `SECURITY.md`, `deny.toml`,
  `clippy.toml`, `cargo-fuzz` targets (no-panic invariant), MkDocs site with
  Privacy/Terms, and GitHub Actions CI (test/clippy/fmt/coverage/deny/freshness),
  Docs, and a tag-driven Release workflow.

### Notes

- The leap-aware (GPS/TAI/NTP) family via `hifitime`, the `Packed` strategy
  (FAT/DOS/SYSTEMTIME), and the distribution fan-out (Homebrew/apt/winget) remain
  on the build-out plan in `HANDOFF.md`.
