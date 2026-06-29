# Changelog

All notable changes to `timeglyph` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Selectable output timezone** (`--tz`): render dates in `UTC` (default, `Z`),
  a fixed offset (`+08:00`), or an IANA zone (`America/New_York`, DST-correct per
  instant). Threaded through identify/decode/hex/string/csv and `--json`; the
  instant is unchanged, only the displayed offset. An unknown zone errors loudly.
- **Lunisolar calendar + 干支 four pillars** (`lunisolar` feature, CLI
  `lunisolar <datetime> --tz <zone> [--longitude <°E>]`): the Chinese lunar date
  (incl. leap months) plus the sexagenary year/month/day/hour pillars, the Sun's
  apparent ecliptic longitude, and the current solar term. The year (立春) and
  month (the 12 节) pillars are driven by the `stem-branch` solar ephemeris; the
  lunar (moon) date by `lunar-lite`; the day pillar by Julian-day arithmetic and
  the hour pillar by 五鼠遁. The conversion is convention-relative, so a meridian
  (`--tz`) is **required** and an optional longitude corrects the hour pillar to
  local mean solar time; conventions (立春 year, 节 month, 正月初一 lunar date) are
  surfaced as assumptions. Validated against the independent `cnlunar` oracle.
- **Context-aware scoring components** (HANDOFF §5b): `byte_width_match`,
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

- Remaining build-out in `HANDOFF.md`: the obscure packed-bitfield formats
  (exFAT offset byte, bitdate/dttm/logtime/ns40/moto/symantec/dvr, BCD/GSM
  semi-octet, Sonyflake's 10ms unit) and the distribution fan-out
  (Homebrew/apt/winget).
