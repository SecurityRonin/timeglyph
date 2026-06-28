---
title: "timeglyph — forensic timestamp decipherment"
description: >-
  The authoritative, cited reference for decoding timestamps in digital forensics:
  every epoch, encoding, calendar, leap second, and rollover, with primary-source
  citations. FILETIME, Unix, Cocoa, Snowflake, GPS, TAI64, NTP and more.
---

# timeglyph

**Forensic timestamp decipherment** — decode, encode, and *identify* the many ways
systems inscribe time, with scored, cited, ambiguity-first interpretation.

A timestamp is time inscribed as a symbol: the raw integer or bytes a system writes
to mean an instant. `timeglyph` deciphers those inscriptions. Given an *unknown*
value it reports **every plausible interpretation, ranked and scored, with its
assumptions** — never a single "detected" answer, because a raw value is usually
ambiguous.

```console
$ timeglyph 1577836800
# readings consistent with 1577836800 (ranked; a raw value is usually underdetermined — not a single verdict):
  [1.00] unix           2020-01-01T00:00:00Z  (Unix time (seconds))
  [0.94] postgres       2000-01-01T00:26:17.8368Z  (PostgreSQL timestamp (µs since 2000))
  [0.67] cocoa          2051-01-01T00:00:00Z  (Cocoa / CFAbsoluteTime (s since 2001))
  ...
```

## Why this site exists

Good timestamp converters already exist. What is scarce is a single **authoritative,
cited reference** that explains not just *what* a format is but *why* it is shaped
that way — the epoch it counts from, the calendar it assumes, the leap-second policy
it ignores or honours, and the rollover that eventually breaks it. This site is that
reference. Every fact links to a primary source (an RFC, a vendor specification, a
standards body), collected on the [References](references.md) page.

## Start here

<div class="grid cards" markdown>

- **[Time scales & leap seconds](concepts/time-scales.md)** — UTC, TAI, GPS, UT1,
  leap seconds, leap smearing, and why POSIX time is not a count of SI seconds.
- **[Calendars](concepts/calendars.md)** — Julian vs Gregorian, the 1582 reform,
  proleptic Gregorian, astronomical year numbering, leap-year rules, Julian Day.
- **[Precision](concepts/precision.md)** — seconds → milliseconds → microseconds →
  nanoseconds, and why the storage *unit* is not the clock's *accuracy*.
- **[RTC hardware](concepts/rtc-hardware.md)** — the MC146818 CMOS clock, the 18.2 Hz
  PC timer tick, the 1980 DOS epoch, and what a reset clock looks like.
- **[Epoch rollovers](concepts/rollovers.md)** — Y2K, Year 2038, NTP 2036, GPS week
  rollover, Year 2106, and the epoch + width + signedness model behind them all.
- **[Format reference](formats/index.md)** — every supported format with its epoch,
  layout, citation, evolution, and forensic gotchas.
- **[Validation](validation.md)** — the tier-1 differential battery: every format
  cross-checked against an independent third-party oracle, with provenance.

</div>

## The model

- **A reading is evidence, not a verdict.** Every candidate is framed as *consistent
  with* a format and carries its scored components and assumptions — including a
  leap-smear disclaimer for POSIX-labelled readings. See [Methodology](concepts/methodology.md).
- **Ambiguity is first-class.** The default output is the ranked candidate set;
  scoring combines named components (representable, in-window, granularity match,
  magnitude fit) so the rank is auditable, never opaque.
- **POSIX-correct spine.** The internal instant is `PosixNs(i128)` — nanoseconds
  since 1970, proleptic Gregorian, leap-second-ignoring. It is deliberately *not*
  called UTC; the leap-aware scales (GPS/TAI/NTP) are kept separate.
- **Reused calendar math.** Civil-time conversion is delegated to
  [`jiff`](https://docs.rs/jiff); leap-aware scales use
  [`hifitime`](https://docs.rs/hifitime). `timeglyph` writes zero calendar code.

## Usage

```console
$ timeglyph 1577836800                       # identify (auto-detect, ranked)
$ timeglyph --json 1577836800                # machine-readable candidates
$ timeglyph --hex 0060947C58B2D501           # raw bytes, LE/BE × 32/64-bit
$ timeglyph --from filetime 132223104000000000   # decode under one known format
$ timeglyph --from gps 1261872018            # leap-aware (cargo build --features leap)
$ timeglyph --list                           # the registry, with spec citations
```
