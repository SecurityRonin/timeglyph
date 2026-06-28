---
title: "Methodology & epistemics — a reading is evidence, not a verdict"
description: >-
  How timeglyph scores and presents candidate timestamp interpretations: the
  PosixNs spine, the named plausibility components, and the epistemic framing that
  keeps a reading 'consistent with' a format rather than a verdict.
---

# Methodology and epistemics

`timeglyph` treats a timestamp reading as **forensic evidence, not a verdict**. A
single integer is usually underdetermined — the same 64-bit value can be a plausible
Unix-seconds, Java-milliseconds, Chrome-microseconds, FILETIME, .NET-ticks, and
Cocoa-seconds date at once. Presenting one as *the* answer would fabricate certainty.
This page explains how the engine avoids that.

## The PosixNs spine

Internally every POSIX-family instant is `PosixNs(i128)` — **nanoseconds since
1970-01-01, proleptic Gregorian, leap-second-ignoring**. Two deliberate choices:

- **`i128`, not `i64`.** FILETIME's 1601 epoch alone is ~1.16 × 10¹⁹ ns from 1970,
  which overflows a signed 64-bit nanosecond counter. The wide spine is load-bearing.
- **Named `PosixNs`, not UTC.** UTC has leap-second discontinuities that POSIX pretends
  away; calling the spine "UTC" would be an auditable error (see
  [time scales](time-scales.md)). The genuinely leap-aware scales — GPS, TAI64, NTP —
  are kept **out** of this spine and handled separately.

## Scored candidates, not a single answer

Auto-detection returns **every plausible reading**, ranked by a score in `[0, 1]` that
is the weighted mean of named, individually-emitted components. Because the components
are visible, the rank is auditable — you can see *why* a reading scored as it did.

| Component | What it measures |
|---|---|
| `representable` | the instant renders to a civil date (in range) |
| `in_window` | the instant falls in the configured plausibility window |
| `granularity_match` | does the value's sub-second resolution fit the unit? (the [seconds-vs-ms-vs-µs-vs-ns](precision.md) disambiguation, via trailing-zero structure) |
| `magnitude_fit` | is the value's magnitude consistent with the encoding? (sinks epoch-hugging false reads of [embedded-ID](../formats/identifiers.md) schemes) |

A low component **lowers the rank; it never hides the reading.** Forensics is about
surfacing possibilities with their weight, not silently discarding them.

## Epistemic framing

The language of every reading is chosen to stay on the right side of the
expert-witness line:

- Readings are described as **"consistent with"** a format — never "detected", "is",
  or any verdict.
- **POSIX readings carry a leap-smear disclaimer**: "indistinguishable from a
  leap-smeared source without clock-policy metadata" (see
  [leap smearing](time-scales.md#leap-smearing-and-why-a-raw-value-cant-reveal-it)).
- **Local-time formats carry a no-offset caveat**: e.g. a FAT reading states the
  instant is naive wall-clock time, not UTC (see [FAT/DOS](../formats/windows.md#fat-dos)).

## Clean-room provenance

Every format's epoch and encoding is taken from a **primary specification**, listed on
the [References](../references.md) page and cited on each format entry. Worked examples are
cross-checked against an independent oracle (a different language or tool) rather than
against `timeglyph`'s own output, so a test cannot pass merely because the fixture was
encoded to match a bug.

## See also

- [Format overview](../formats/index.md) — the registry these readings draw on.
- [Time scales](time-scales.md) and [Calendars](calendars.md) — the assumptions behind
  "consistent with".
