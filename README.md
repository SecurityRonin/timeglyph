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

```console
$ timeglyph 1577836800
# readings consistent with 1577836800 (ranked; a raw value is usually underdetermined — not a single verdict):
  [1.00] unix           2020-01-01T00:00:00Z  (Unix time (seconds))
  [0.94] postgres       2000-01-01T00:26:17.8368Z  (PostgreSQL timestamp (µs since 2000))
  [0.67] cocoa          2051-01-01T00:00:00Z  (Cocoa / CFAbsoluteTime (s since 2001))
  [0.67] hfsplus        1953-12-31T00:00:00Z  (Apple HFS+ (s since 1904))
  ...

$ timeglyph --json 1577836800          # machine-readable ranked candidates
$ timeglyph --hex 0060947C58B2D501     # raw bytes, LE/BE × 32/64-bit
$ timeglyph --from filetime 132223104000000000
$ timeglyph --list                     # the format registry, with spec citations
```

## Status

Working engine. The `PosixNs(i128)` spine, decode/encode/auto-detect/byte-decode,
component-based plausibility scoring, and epistemic framing (consistent-with +
leap-smear) are in and tested against primary-spec anchors. The registry covers
Unix s/ms/µs/ns, FILETIME, WebKit/Chrome, Cocoa (integer and signed-double),
HFS+, .NET ticks, OLE, PostgreSQL, SQLite Julian day, and Snowflake IDs
(Twitter/X, Discord). The leap-aware (GPS/TAI/NTP) family and the `Packed`
(FAT/DOS) strategy are the remaining build-out — see **[HANDOFF.md](HANDOFF.md)**
for the design record and plan.

## Why another converter?

Honestly: good ones exist ([`time_decode`](https://github.com/digitalsleuth/time_decode),
MIT; DCode, proprietary). `timeglyph` isn't "the first" — it's a Rust single static
binary and a **rigorous, cited model** where a reading is *evidence*, not a verdict:
a POSIX-correct internal spine (never mislabelled UTC), the leap-second family kept
separate, and **ambiguity as first-class scored output**. Calendar/timezone math is
reused (`jiff`), never reinvented; the leap-aware family will delegate to `hifitime`.

[Privacy Policy](https://securityronin.github.io/timeglyph/privacy/) · [Terms of Service](https://securityronin.github.io/timeglyph/terms/) · © 2026 Security Ronin Ltd
