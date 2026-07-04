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

**Forensic timestamp decipherment** — decode, encode, and *identify* the many ways
systems inscribe time, with scored, cited, ambiguity-first interpretation.

A timestamp is time inscribed as a symbol — the raw integer or bytes a system
writes to mean an instant. `timeglyph` deciphers those inscriptions. Given an
*unknown* value it reports **every plausible interpretation, ranked and scored,
with its assumptions** — never a single "detected" answer, because a raw value is
usually ambiguous.

**[Full documentation →](https://securityronin.github.io/timeglyph/)**

```console
$ timeglyph 1577836800
# readings consistent with 1577836800 (ranked; a raw value is usually underdetermined — not a single verdict):
  [1.00] unix           2020-01-01T00:00:00Z  (Unix time (seconds))
  [0.94] postgres       2000-01-01T00:26:17.8368Z  (PostgreSQL timestamp (µs since 2000))
  [0.67] cocoa          2051-01-01T00:00:00Z  (Cocoa / CFAbsoluteTime (s since 2001))
  [0.67] hfsplus        1953-12-31T00:00:00Z  (Apple HFS+ (s since 1904))
  ...

$ timeglyph identify --json 1577836800   # machine-readable ranked candidates
$ timeglyph decode filetime 132223104000000000
$ timeglyph encode unix 2020-01-01T00:00:00Z
$ timeglyph hex 0060947C58B2D501       # raw bytes (LE/BE + packed on-disk)
$ timeglyph string 20200101000000Z     # ISO / RFC / ASN.1 string forms
$ timeglyph scan app.log               # find & decode every timestamp in text (or stdin)
$ timeglyph csv events.csv             # enrich a CSV: human-readable timestamp columns
$ timeglyph list                       # the format registry, with spec citations
```

Exit codes are pipeline-safe: `0` clear top reading, `2` ambiguous or a sentinel
(review needed), `1` error.

Render in any timezone with `--tz` (`UTC`, a fixed offset, or an IANA name,
DST-correct), and pass `--artifact "<hint>"` to nudge readings toward a known
source family.

## Install

**Windows**
```powershell
winget install SecurityRonin.timeglyph
```

**macOS**
```bash
brew install securityronin/tap/timeglyph
```

**Debian / Ubuntu**
```bash
curl -1sLf 'https://dl.cloudsmith.io/public/securityronin/timeglyph/setup.deb.sh' | sudo -E bash
sudo apt install timeglyph
```

On Windows and macOS this also installs the [`timeglyph-lens`](#timeglyph-lens--hover-anything-read-the-time) overlay.

## timeglyph-lens — hover anything, read the time

A companion GUI: an always-on-top overlay that follows your cursor and shows
timeglyph's ranked readings for any number in the UI element under the pointer —
no copy-paste. Each row carries its confidence, the weekday, and the public
holiday for that date in the chosen zone. Pick any display timezone from the
footer.

<p align="center">
  <img src="assets/lens-in-action.png" alt="timeglyph-lens decoding a SQLite timestamp column live over DB Browser for SQLite" width="640" />
</p>

It installs with the CLI (`winget install SecurityRonin.timeglyph` on Windows,
`brew install securityronin/tap/timeglyph` on macOS) and reads the element under
the cursor through the platform accessibility layer — the Accessibility API on
macOS, UI Automation on Windows. (Linux support is in progress.)

**First launch on macOS — grant Accessibility.** macOS gates the Accessibility
API behind an explicit grant, so the overlay shows no readings until you allow it:

1. Open **System Settings → Privacy & Security → Accessibility**.
2. Turn on **timeglyph-lens** (click **+** and add it if it isn't listed).
3. Quit and relaunch timeglyph-lens.

**Windows** needs no special permission. To inspect an *elevated*
(Run-as-administrator) window, launch timeglyph-lens elevated too — Windows blocks
UI access across privilege levels.

## Status

Working engine. The `PosixNs(i128)` spine, decode/encode/auto-detect/byte-decode,
the full named-component plausibility scoring (window, granularity, magnitude,
byte-width, endian, artifact-context, neighbour-monotonicity), and epistemic
framing (consistent-with + leap-smear) are in and tested against primary-spec
anchors and the MIT `time-decode` oracle. The registry covers Unix s/ms/µs/ns,
FILETIME (incl. AD/LDAP), WebKit/Chrome, Cocoa (integer / signed-double / iOS-11
ns), HFS+, .NET ticks, OLE, Excel-1904, PostgreSQL, Mozilla PRTime, SQLite Julian
day, KSUID, Snowflake-class IDs (Twitter/X, Discord, Mastodon, LinkedIn, TikTok),
FAT/DOS + SYSTEMTIME packed forms, and the ULID / UUIDv1 / RFC 2822 / EXIF / ISO /
ASN.1 string forms. The leap-aware GPS/TAI64/NTP family is behind the `leap`
feature (`decode gps|tai64|ntp`, leap-correct UTC via `hifitime`); the Chinese
lunisolar calendar + 干支 four pillars are behind the `lunisolar` feature
(`lunisolar <datetime> --tz <zone>`, via the `stem-branch` ephemeris); a
whole-world public-holiday lookup (248 countries, 1980–2100, generated from the
MIT `python-holidays`) is behind the `holiday` feature; and arbitrary text is
mined for timestamps with `scan`. The `timeglyph-lens` overlay (above) is the
interactive front-end for all of it. The library carries 100% function-coverage
tests. See **[the ADRs](docs/decisions/)** for the design decisions.

## Why another converter?

Honestly: good ones exist ([`time_decode`](https://github.com/digitalsleuth/time_decode),
MIT; DCode, proprietary). `timeglyph` isn't "the first" — it's a Rust single static
binary and a **rigorous, cited model** where a reading is *evidence*, not a verdict:
a POSIX-correct internal spine (never mislabelled UTC), the leap-second family kept
separate, and **ambiguity as first-class scored output**. Calendar/timezone math is
reused (`jiff`), never reinvented; the leap-aware family will delegate to `hifitime`.

[Privacy Policy](https://securityronin.github.io/timeglyph/privacy/) · [Terms of Service](https://securityronin.github.io/timeglyph/terms/) · © 2026 Security Ronin Ltd
