---
title: "timeglyph — forensic timestamp decipherment"
description: >-
  The authoritative, cited reference for decoding timestamps in digital forensics:
  every epoch, encoding, calendar, leap second, and rollover, with primary-source
  citations. FILETIME, Unix, Cocoa, Snowflake, GPS, TAI64, NTP — plus a forensic
  calendar with DST, leap seconds, GPS week, format epochs, and moon phase.
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
- **[Forensic calendar](cal.md)** — `cal` lays a day, month, or year out with DST
  fold/gap days, leap seconds, GPS week, ISO/Julian-Day numbering, format epochs,
  alternative calendars, and the moon's phase — every value oracle-validated.
- **[Validation](validation.md)** — the tier-1 differential battery: every format
  cross-checked against an independent third-party oracle, with provenance.

</div>

## The model

- **A reading is evidence, not a verdict.** Every candidate is framed as *consistent
  with* a format and carries its scored components and assumptions — including a
  leap-smear disclaimer for POSIX-labelled readings. See [Methodology](concepts/methodology.md).
- **Ambiguity is first-class.** The default output is the ranked candidate set;
  scoring combines named components (representable, in-window, granularity match,
  magnitude fit, epoch distance) so the rank is auditable, never opaque.
- **POSIX-correct spine.** The internal instant is `PosixNs(i128)` — nanoseconds
  since 1970, proleptic Gregorian, leap-second-ignoring. It is deliberately *not*
  called UTC; the leap-aware scales (GPS/TAI/NTP) are kept separate.
- **Reused calendar math.** Civil-time conversion is delegated to
  [`jiff`](https://docs.rs/jiff); leap-aware scales use
  [`hifitime`](https://docs.rs/hifitime). `timeglyph` writes zero calendar code.

## Usage

```console
$ timeglyph 1577836800                        # identify (auto-detect, ranked)
$ timeglyph identify --json 1577836800        # machine-readable candidates
$ timeglyph decode filetime 132223104000000000  # decode under one known format
$ timeglyph encode unix 2020-01-01T00:00:00Z  # encode a datetime → a format
$ timeglyph --as hex 0060947C58B2D501        # raw bytes only (LE/BE + packed on-disk)
$ timeglyph --as string 20200101000000Z      # string forms only (ASN.1 / ISO / RFC)
$ timeglyph scan app.log                     # find & decode every timestamp in text/stdin
$ timeglyph cal 2026-11 --tz America/New_York  # forensic calendar (DST/leap/epoch markers + moon)
$ timeglyph decode gps 1261872018            # leap-aware (cargo build --features leap)
$ timeglyph lunisolar 2020-01-25 --tz Asia/Shanghai  # Chinese calendar + 干支 (--features lunisolar)
$ timeglyph csv events.csv                   # enrich a CSV (human-readable timestamp columns)
$ timeglyph list                             # the registry, with spec citations
```

Exit codes are pipeline-safe: `0` a clear top reading, `2` ambiguous or a
[sentinel](concepts/sentinel-values.md) (review needed), `1` error.

## Beyond decoding

Optional, feature-gated layers extend the engine past raw instant↔instant mapping:

- **Leap-aware scales** (`--features leap`) — GPS / TAI64 / NTP with leap-correct
  UTC via `hifitime`, kept separate from the POSIX spine.
- **Chinese lunisolar + 干支** (`--features lunisolar`) — the lunisolar date and
  Heavenly-Stem / Earthly-Branch four pillars for an instant at a chosen meridian,
  via the `stem-branch` ephemeris. A convention-relative reading, not a verdict.
- **Whole-world public holidays** (`--features holiday`) — is a date a public
  holiday in a given country? 248 countries, 1980–2100, generated from the MIT
  `python-holidays` project (names in each country's own locale). Framed as
  *consistent with a public holiday*, an annotation rather than proof.
- **Forensic calendar** (`cal`) — a day, month, or year rendered with the temporal
  detail an examiner reasons about: per-day UTC offset and DST fold/gap days, leap
  seconds and GPS week, ISO/Julian-Day numbering, timestamp-format epoch markers,
  seven alternative calendars (Chinese lunisolar + 干支, plus ROC / Japanese /
  Buddhist / Hebrew / Islamic / Persian), and the moon's phase. See [the calendar](cal.md).

## timeglyph-lens — the cursor overlay

`timeglyph-lens` is the interactive front-end: an always-on-top window that follows
your cursor and shows timeglyph's ranked readings for any number in the element
under the pointer — with the weekday, the public holiday for that date in the
chosen zone, and the opt-in 干支 pillars colored by 五行. macOS and Windows (the
picker uses the Accessibility API / UI Automation respectively); see the
[README](https://github.com/SecurityRonin/timeglyph#timeglyph-lens--hover-anything-read-the-time)
to install.
