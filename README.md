<p align="center">
  <img src="assets/logo.png" alt="timeglyph" width="180" />
</p>

# timeglyph

[![Crates.io](https://img.shields.io/crates/v/timeglyph.svg)](https://crates.io/crates/timeglyph)
[![Docs.rs](https://img.shields.io/docsrs/timeglyph)](https://docs.rs/timeglyph)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![CI](https://github.com/SecurityRonin/timeglyph/actions/workflows/ci.yml/badge.svg)](https://github.com/SecurityRonin/timeglyph/actions/workflows/ci.yml)
[![Release](https://github.com/SecurityRonin/timeglyph/actions/workflows/release.yml/badge.svg)](https://github.com/SecurityRonin/timeglyph/releases)
[![Sponsor](https://img.shields.io/badge/sponsor-h4x0r-ea4aaa?logo=github-sponsors)](https://github.com/sponsors/h4x0r)

**Decode any timestamp. Identify the unknown ones.**

You pulled a raw `133801920000000000` out of an artifact and don't know what it
means. `timeglyph` reads it every way a system might have written it and reports
the results **ranked, scored, and cited** — honest about the ambiguity instead of
guessing one answer. One static Rust binary, plus a live overlay that decodes
whatever is under your cursor.

**[Full documentation →](https://securityronin.github.io/timeglyph/)**

```console
$ timeglyph 1577836800
# readings consistent with 1577836800 (ranked; a raw value is usually underdetermined — not a single verdict):
  [1.00] unix           2020-01-01T00:00:00Z  (Unix time (seconds))
  [0.94] postgres       2000-01-01T00:26:17.8368Z  (PostgreSQL timestamp (µs since 2000))
  [0.67] cocoa          2051-01-01T00:00:00Z  (Cocoa / CFAbsoluteTime (s since 2001))
  [0.67] hfsplus        1953-12-31T00:00:00Z  (Apple HFS+ (s since 1904))
  ...
```

---

## Install

**macOS**
```bash
brew install securityronin/tap/timeglyph
```

**Debian / Ubuntu**
```bash
curl -1sLf 'https://dl.cloudsmith.io/public/securityronin/timeglyph/setup.deb.sh' | sudo -E bash
sudo apt install timeglyph
```

**Windows**
```powershell
winget install SecurityRonin.timeglyph
```

**Cargo**
```bash
cargo install timeglyph
```

On macOS and Windows this also installs the
[`timeglyph-lens`](#timeglyph-lens--hover-anything-read-the-time) overlay.

---

## What you do with it

### Identify an unknown value

```bash
timeglyph 1577836800                    # ranked, scored readings across every format
timeglyph identify --json 1577836800    # same, machine-readable
timeglyph hex 0060947C58B2D501          # raw bytes: little/big-endian + packed on-disk
timeglyph string 20200101000000Z        # ISO / RFC 2822 / ASN.1 string forms
```

Exit codes are pipeline-safe: `0` clear top reading, `2` ambiguous or a sentinel
(review needed), `1` error. Render in any timezone with `--tz` (`UTC`, a fixed
offset, or a DST-correct IANA name); nudge readings toward a source family with
`--artifact "<hint>"`.

### Decode or encode a known format

```bash
timeglyph decode filetime 132223104000000000
timeglyph encode unix 2020-01-01T00:00:00Z
timeglyph list                          # the format registry, with spec citations
```

### Mine artifacts at scale

```bash
timeglyph scan app.log                  # find & decode every timestamp in text (or stdin)
timeglyph csv events.csv                # enrich a CSV with human-readable timestamp columns
```

[CSV enrichment →](docs/csv.md)

---

## timeglyph-lens — hover anything, read the time

An always-on-top overlay that follows your cursor and shows timeglyph's ranked
readings for any number in the UI element under the pointer — no copy-paste. Each
row carries its confidence, the weekday, and the public holiday for that date in
the chosen zone. Pick any display timezone from the footer.

<p align="center">
  <img src="assets/lens-in-action.png" alt="timeglyph-lens decoding a SQLite timestamp column live over DB Browser for SQLite" width="640" />
</p>

It installs with the CLI on macOS and Windows and reads the element under the
cursor through the platform accessibility layer — the Accessibility API on macOS,
UI Automation on Windows. (Linux support is in progress.)

[Overlay guide →](docs/lens.md)

---

## Formats

`timeglyph` decodes and auto-identifies:

- **Epoch integers** — Unix (s/ms/µs/ns), FILETIME (incl. Active Directory / LDAP),
  WebKit/Chrome, Cocoa / CFAbsoluteTime (integer, signed double, iOS-11 ns),
  Apple HFS+, .NET ticks, OLE automation, Excel-1904, PostgreSQL, Mozilla PRTime,
  SQLite Julian day
- **Embedded IDs** — KSUID, ULID, UUIDv1 / v6 / v7, MongoDB ObjectId, and
  Snowflake-class IDs (Twitter/X, Discord, Mastodon, LinkedIn, TikTok)
- **Packed on-disk** — FAT/DOS date-time words and 128-bit SYSTEMTIME structs
- **Strings** — ISO 8601 / RFC 3339, RFC 2822 email dates, EXIF, ASN.1
  GeneralizedTime & UTCTime

Every reading names the spec it assumes and is scored on window membership,
granularity, magnitude, byte-width, endianness, artifact context, and neighbour
monotonicity. Correctness is checked against primary-spec worked examples and the
MIT [`time_decode`](https://github.com/digitalsleuth/time_decode) oracle — see
[validation](docs/validation.md).

---

## Optional feature flags

Off by default, so the common build stays lean.

- **`leap`** — the leap-aware GPS / TAI64 / NTP family (`decode gps|tai64|ntp`),
  leap-correct UTC via `hifitime`
- **`lunisolar`** — the Chinese lunisolar calendar + 干支 four pillars
  (`lunisolar <datetime> --tz <zone>`), via the `stem-branch` ephemeris
- **`holiday`** — whole-world public-holiday lookup (248 countries, 1980–2100,
  from the MIT `python-holidays`)

---

## Why another converter?

Good ones exist ([`time_decode`](https://github.com/digitalsleuth/time_decode),
MIT; DCode, proprietary). `timeglyph` is a single static Rust binary built on a
**rigorous, cited model** where a reading is *evidence, not a verdict*: a
POSIX-correct internal spine (never mislabelled UTC), the leap-second family kept
separate, and **ambiguity as first-class, scored output**. Calendar and timezone
math is reused (`jiff`), never reinvented. See
[the design decisions](docs/decisions/).

---

[Privacy Policy](https://securityronin.github.io/timeglyph/privacy/) · [Terms of Service](https://securityronin.github.io/timeglyph/terms/) · © 2026 Security Ronin Ltd
