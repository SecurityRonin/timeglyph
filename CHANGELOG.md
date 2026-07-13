# Changelog

All notable changes to `timeglyph` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0] - 2026-07-13

### Added

- **`cal` — a forensics-grade calendar.** `timeglyph cal` renders a day, month, or
  year with the temporal detail an examiner reasons about: per-day UTC offset and
  DST fold/gap days (flagged, with wall-day length), leap-second days and GPS week
  (from hifitime's IERS table), ISO week / day-of-year / Julian Day / Modified JD /
  Unix midnight, timestamp-format epoch and rollover markers (derived from the
  registry), the Chinese lunisolar date + 干支, Hebrew and Islamic dates (behind the
  new `altcal` feature, via ICU4X), and the moon's phase as a shaded ASCII disc.
  The year view carries an astronomically-exact season timeline (equinox/solstice
  instants from the stem-branch ephemeris, hemisphere-aware via `--south`). A
  faithful `--json` record carries every field per day. Every computed value is
  validated against an independent oracle (`date`, `zdump`, USNO, IERS, JPL/Meeus).
  Requires `stem-branch >= 0.8` (for the new `moon_phase` API).

### Added

- **Correctness wave (DST fold/gap, Y2038, GPS rollover).** A `LocalNaive`
  reading under a named `--tz` now flags a DST fall-back **fold** (both instants)
  or a spring-forward **gap** (nonexistent wall time) — `resolve_local`,
  tier-1-validated against IANA tzdb. A 4-byte value with the high bit set surfaces
  the **signed i32 (wrapped `time_t`)** reading alongside the unsigned one. New
  `leap::gps_rollover_eras` emits the plausible 1024-week GPS eras (with an optional
  case-date anchor); `leap::within_leap_smear_window` adds a cloud-smear note when a
  reading falls within ±12h of a leap second (from hifitime's IERS table).
- **`explain` — registry-generated spec cards.** `timeglyph explain <format>` prints
  the epoch, tick unit, tz/leap semantics, valid range, known sentinels, and citation
  (`interpret::explain`).
- **`identify_bytes` + bounded carve.** `interpret::identify_bytes(&[u8])` (the byte
  sweep behind `interpret_hex`) and `carve::carve` (find timestamps at every offset of
  a bounded blob, window + score-thresholded), with JSONL, ImHex-bookmark, and
  Timesketch-JSONL exports; `timeglyph carve <hex>` CLI.
- **`mcp` — Model Context Protocol server.** `timeglyph mcp` speaks JSON-RPC over
  stdio, exposing `identify`/`decode`/`explain` as tools for LLM-driven DFIR.
- **Format wave 2.** Oracle 7-byte DATE, ISO 9660 (ECMA-119), ext4 extended
  timestamp, CP56Time2a (IEC 60870-5), UDF (ECMA-167), and IEEE 1588 PTP — each
  tier-1-validated against its spec worked example; the first five decode via the
  CLI (`decode <fmt> <hex|secs,extra>`).
- `sentinel_reason` now also flags `i64::MIN` (unset/overflow).

### Changed

- **Validation is enforced, not just claimed.** `docs/validation-tiers.tsv` +
  `tests/validation_tiers.rs` gate that every format is audited, none is tier-3-only,
  and each tier claim is bound to the oracle file that references it. Reliability is
  reported **per family** (36 families), never one global number that hides
  same-instant-collision families.

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
