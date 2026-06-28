# timeglyph

**Forensic timestamp decipherment** — decode, encode, and *identify* the many ways
systems inscribe time, with scored, cited, ambiguity-first interpretation.

A timestamp is time inscribed as a symbol — the raw integer or bytes a system
writes to mean an instant. `timeglyph` deciphers those inscriptions. Given an
*unknown* value it reports **every plausible interpretation, ranked and scored,
with its assumptions** — never a single "detected" answer, because a raw value is
usually ambiguous.

```console
$ timeglyph 1577836800
# ranked candidate interpretations of 1577836800 (NOT a single answer):
  [1.00] unix           2020-01-01T00:00:00Z  (Unix time (seconds))
  [0.00] cocoa          2051-01-01T00:00:00Z  (Cocoa / CFAbsoluteTime (s since 2001))
  [0.00] hfsplus        1953-12-31T00:00:00Z  (Apple HFS+ (s since 1904))
  ...

$ timeglyph --hex 0060947C58B2D501     # raw bytes, LE/BE × 32/64-bit
$ timeglyph --from filetime 132223104000000000
$ timeglyph --list                     # the format registry, with spec citations
```

## Status

Early scaffold. The core engine (PosixNs spine, decode/encode/auto-detect/byte
decode) and 9 spec-anchored formats (Unix s/ms/µs, FILETIME, WebKit/Chrome, Cocoa,
HFS+, .NET ticks, OLE) are in and tested. The full ~70-format catalog, the
leap-aware (GPS/TAI/NTP) family, and the scored-plausibility model are the
build-out — see **[HANDOFF.md](HANDOFF.md)** for the design record and plan.

## Why another converter?

Honestly: good ones exist ([`time_decode`](https://github.com/digitalsleuth/time_decode),
MIT; DCode, proprietary). `timeglyph` isn't "the first" — it's a Rust single static
binary and a **rigorous, cited model** where a reading is *evidence*, not a verdict:
a POSIX-correct internal spine (never mislabelled UTC), the leap-second family kept
separate, and **ambiguity as first-class scored output**. Calendar/timezone math is
reused (`jiff` / `hifitime`), never reinvented.

[Privacy Policy](https://securityronin.github.io/timeglyph/privacy/) · [Terms of Service](https://securityronin.github.io/timeglyph/terms/) · © 2026 Security Ronin Ltd
