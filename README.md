# timeglyph

[![Crates.io](https://img.shields.io/crates/v/timeglyph.svg)](https://crates.io/crates/timeglyph)
[![Docs.rs](https://img.shields.io/docsrs/timeglyph)](https://docs.rs/timeglyph)
[![CI](https://github.com/SecurityRonin/timeglyph/actions/workflows/ci.yml/badge.svg)](https://github.com/SecurityRonin/timeglyph/actions/workflows/ci.yml)
[![Release](https://github.com/SecurityRonin/timeglyph/actions/workflows/release.yml/badge.svg)](https://github.com/SecurityRonin/timeglyph/releases)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Sponsor](https://img.shields.io/badge/sponsor-h4x0r-ea4aaa?logo=github-sponsors)](https://github.com/sponsors/h4x0r)

**Decipher any timestamp — every plausible reading, ranked and cited, never one fabricated answer.**

You found an 18-digit number in a SQLite cell, a registry value, a browser cookie. Which epoch? Which unit? `timeglyph` doesn't pick one and hope — it surfaces *every* plausible reading, **scored, with its assumptions and a spec citation**, so you can tell Chrome-µs from FILETIME from an iOS nanosecond NSDate at a glance.

```bash
cargo install timeglyph
timeglyph 13390845530064940
```

```console
# readings consistent with 13390845530064940 (ranked; a raw value is usually underdetermined — not a single verdict):
  [0.99] iostime          2001-06-04T23:40:45.53006494Z  (Apple NSDate iOS 11+ (ns since 2001))
  [0.98] webkit           2025-05-04T15:18:50.06494Z     (Chrome / WebKit (µs since 1601))
  [0.74] unix_ns          1970-06-04T23:40:45.53006494Z  (Unix time (nanoseconds))
  [0.73] filetime         1643-06-08T15:55:53.006494Z    (Windows FILETIME (100ns since 1601))
```

**[Full documentation →](https://securityronin.github.io/timeglyph/)**

---

## Install

**macOS**
```bash
brew install securityronin/tap/timeglyph
```

**Debian / Ubuntu / Kali**
```bash
curl -1sLf 'https://dl.cloudsmith.io/public/securityronin/timeglyph/setup.deb.sh' | sudo bash
sudo apt install timeglyph
```

**Cargo**
```bash
cargo install timeglyph                       # core registry
cargo install timeglyph --features leap       # + leap-aware GPS / TAI64 / NTP
cargo install timeglyph --features lunisolar  # + Chinese lunisolar calendar / 干支
```

Prebuilt static binaries for macOS (Apple Silicon + Intel), Linux (x86-64 + ARM64, musl), and Windows are attached to every [release](https://github.com/SecurityRonin/timeglyph/releases/latest). The distributed packages ship the core registry; the `leap` and `lunisolar` families are opt-in via `cargo install --features`.

---

## What you do with it

### Identify an unknown value — the whole point

A raw integer is usually ambiguous, so `timeglyph` ranks *all* readings instead of asserting one. Pass `--artifact "<hint>"` (e.g. `"chrome history"`) to nudge readings toward a known source family, or `--json` for machine-readable output.

```bash
timeglyph 13390845530064940
timeglyph --json 13390845530064940 --artifact "chrome"
```

### Decode or encode one known format

```bash
timeglyph decode filetime 132223104000000000   # -> 2020-01-01T00:00:00Z
timeglyph encode unix 2020-01-01T00:00:00Z      # -> 1577836800
timeglyph list                                  # the registry, with spec citations
```

### Raw bytes, strings, and CSVs

```bash
timeglyph hex aed19dd607bddb01   # LE/BE widths + packed on-disk layouts (FAT, SYSTEMTIME)
timeglyph string 01JTDY1SYGCZWCBPCSEBHV1DW2   # ISO / RFC 2822 / ASN.1 / ULID / UUIDv1 / EXIF
timeglyph csv events.csv         # enrich a CSV: add a human-readable column per timestamp column
```

### Render in any timezone

The instant is absolute; `--tz` changes only how it's displayed (`UTC`, a fixed offset, or an IANA zone, DST-correct per instant).

```console
$ timeglyph 1577836800 --tz Asia/Tokyo
  [1.00] unix             2020-01-01T09:00:00+09:00  (Unix time (seconds))
```

### Chinese lunisolar calendar + 干支 four pillars  *(`--features lunisolar`)*

A timestamp has no single Chinese date without a meridian, so `--tz` is required; `--longitude` optionally corrects the hour pillar to true solar time.

```console
$ timeglyph lunisolar 2020-06-01T00:00:00Z --tz +08:00 --longitude 116.4
2020-06-01T08:00:00+08:00
  lunisolar: 2020年 閏4月 10日
  四柱 pillars: 庚子年 辛巳月 乙亥日 庚辰時
  solar: λ 70.97° (小滿)
```

---

## Why "scored," not "detected"

A single value is *evidence*, not a verdict. Other converters pick a most-likely format; `timeglyph` treats ambiguity as first-class:

- **Ranked candidates, never one answer** — every civil-renderable reading is returned, ordered by a transparent score.
- **Named score components, not an opaque number** — `in_window`, `granularity_match` (the seconds-vs-ms-vs-µs-vs-ns disambiguator), `magnitude_fit`, plus byte-width / endian / artifact-context / neighbour-coherence when that context is available. A low component lowers the rank; it never hides a reading.
- **Stated assumptions + spec citation** on every reading ("consistent with FILETIME [MS-DTYP §2.3.3]", never "detected"), with leap-smear and local-time caveats surfaced.
- **POSIX-correct spine** — `PosixNs(i128)`, deliberately *not* mislabelled UTC; the leap-aware GPS/TAI64/NTP family is kept separate.

## Formats

Unix s/ms/µs/ns · FILETIME (incl. AD/LDAP) · Chrome/WebKit · Cocoa / CFAbsoluteTime (integer, signed-double, iOS-11 ns) · HFS+ · .NET ticks · OLE / Excel-1904 · PostgreSQL · Mozilla PRTime · SQLite Julian day · KSUID · Snowflake-class IDs (Twitter/X, Discord, Mastodon, LinkedIn, TikTok) · FAT/DOS + 128-bit SYSTEMTIME packed forms · ULID · UUIDv1 · RFC 2822 · EXIF · ISO 8601 / RFC 3339 · ASN.1 UTCTime/GeneralizedTime · GPS / TAI64 / NTP (`--features leap`) · Chinese lunisolar + 干支 (`--features lunisolar`).

## Validation

Correctness is checked against an **independent third-party oracle**, not only fixtures we wrote: every linear/embedded/string format is cross-checked against the MIT [`time-decode`](https://github.com/digitalsleuth/time_decode) tool on *its own* published example values, and the lunisolar/干支 output against the `cnlunar` reference. The engine is panic-free (`#![forbid(unsafe_code)]`, no `unwrap`/`expect`/unchecked indexing in production paths) and fuzzed per parser. Methodology: **[docs/validation.md](https://securityronin.github.io/timeglyph/validation/)**.

## Acknowledgements

**Corey Forman** ([digitalsleuth](https://github.com/digitalsleuth)) created [`time_decode`](https://github.com/digitalsleuth/time_decode) (MIT), the comprehensive forensic timestamp tool used here as a differential oracle. Calendar and timezone math is reused from [`jiff`](https://github.com/BurntSushi/jiff); the lunisolar astronomy from [`stem-branch`](https://github.com/h4x0r/stem-branch); leap-second scales from [`hifitime`](https://github.com/nyx-space/hifitime).

---

[Privacy Policy](https://securityronin.github.io/timeglyph/privacy/) · [Terms of Service](https://securityronin.github.io/timeglyph/terms/) · © 2026 Security Ronin Ltd
