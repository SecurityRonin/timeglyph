# Changelog

All notable changes to `timeglyph` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.1] - 2026-07-11

### Changed

- **Packaging only — no library or CLI behaviour change.** The PyPI page now
  renders a real project description (a Python-focused `README`, wired into
  `pyproject` alongside project URLs, authors, and keywords), and the crate gains
  `homepage`/`documentation` metadata for the crates.io sidebar. Python wheels are
  now stable-ABI (`abi3`, CPython 3.9+) and publish on the same `v*` release tag as
  the crate and binaries.

## [0.4.0] - 2026-07-11

### Changed

- **BREAKING (build): the format catalog is now sourced from `forensicnomicon`.**
  The authoritative 45-format timestamp catalog (ids, labels, epochs, tick units,
  tz/leap semantics, packed-layout tags) moved to
  `forensicnomicon::temporal_formats` (the zero-dependency DFIR knowledge leaf).
  timeglyph is now the *engine* that consumes it: `registry::FORMATS` is built by
  wrapping each catalog entry in an engine `Format { meta, packed }`, where the
  packed calendar codecs stay in timeglyph. `forensicnomicon` is now a **required**
  dependency (was optional behind `artifact-hints`); requires `forensicnomicon >= 1.3`.
- **BREAKING (API):** `Unit`, `TzSemantics`, `LeapSemantics`, and the format record
  are re-exported from `forensicnomicon::temporal_formats`; `Strategy` is renamed
  `Encoding` (with `Packed` now carrying a `PackedLayout` tag rather than function
  pointers). `Format` is now a thin wrapper that `Deref`s to the catalog record
  (`TimeFormat`) and exposes `.meta`; its decode/encode methods are unchanged.
- The `artifact-hints` feature now only toggles the artifact-hint *behaviour*; the
  `forensicnomicon` dependency it used to gate is unconditional.

## [0.3.0] - 2026-06-29

### Added

- **Selectable output timezone** (`--tz`): render dates in `UTC` (default, `Z`),
  a fixed offset (`+08:00`), or an IANA zone (`America/New_York`, DST-correct per
  instant). Threaded through identify/decode/hex/string/csv and `--json`; the
  instant is unchanged, only the displayed offset. An unknown zone errors loudly.
- **Lunisolar calendar + 干支 four pillars** (`lunisolar` feature, CLI
  `lunisolar <datetime> --tz <zone> [--longitude <°E>]`): the Chinese lunar date
  (incl. leap months) plus the sexagenary year/month/day/hour pillars, the Sun's
  apparent ecliptic longitude, and the current solar term. The year (立春) and
  month (the 12 節) pillars are driven by the `stem-branch` solar ephemeris; the
  lunar (moon) date by `lunar-lite`; the day pillar by Julian-day arithmetic and
  the hour pillar by 五鼠遁. The conversion is convention-relative, so a meridian
  (`--tz`) is **required** and an optional longitude corrects the hour pillar to
  local mean solar time; conventions (立春 year, 節 month, 正月初一 lunar date) are
  surfaced as assumptions. Validated against the independent `cnlunar` oracle.
- **Context-aware scoring components** (ADR 0005): `byte_width_match`,
  `endian_match`, `artifact_match`, `neighbour_monotonicity`, each emitted only
  when an `InterpretContext` supplies its input (hex width/endian, `--artifact`
  hint, CSV column neighbours); the zero-context default is unchanged.
- **Format catalog**: PostgreSQL (µs since 2000), Unix nanoseconds, Cocoa
  `CFAbsoluteTime` as a signed double, SQLite Julian-day float, Snowflake IDs
  (Twitter/X and Discord), plus AD/LDAP, Mozilla PRTime, iOS-11 NSDate (ns),
  KSUID, Excel-1904, Mastodon/LinkedIn/TikTok IDs, ULID, UUIDv1, RFC 2822, EXIF,
  and 128-bit SYSTEMTIME — each cross-checked against the MIT `time-decode`
  oracle.
- **Generalised embedded-ID strategy** (`Embedded { epoch_ns, shift_bits, unit }`,
  was `EmbeddedMillis`): supports seconds-shift IDs (TikTok) as well as the
  millisecond snowflake family.
- **`Packed` strategy + FAT/DOS format**: unpacks the 32-bit FAT/DOS date+time
  words as LOCAL naive time, with a no-offset/naive caveat surfaced on the
  reading (FAT's local-time semantics are forensically significant).
- **Leap-aware module** (`src/leap.rs`, behind the `leap` feature): GPS and TAI64
  via `hifitime`'s leap-second table, NTP (RFC 5905) via additive `jiff`; kept
  out of the POSIX spine. CLI `--from gps|tai64|ntp`.
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

### Changed

- `EmbeddedMillis` strategy renamed/generalised to `Embedded { …, unit }`.

### Notes

- Remaining build-out: the obscure packed-bitfield formats
  (exFAT offset byte, bitdate/dttm/logtime/ns40/moto/symantec/dvr, BCD/GSM
  semi-octet, Sonyflake's 10ms unit) and the distribution fan-out
  (Homebrew/apt/winget).
