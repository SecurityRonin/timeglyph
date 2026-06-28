---
title: "Calendars — Julian, Gregorian, proleptic, Julian Day"
description: >-
  The 1582 Gregorian reform and its 10-day jump, staggered national adoption,
  Julian vs Gregorian leap-year rules, the proleptic Gregorian calendar,
  astronomical year numbering, and Julian Day / Modified Julian Day.
---

# Calendars

Every "days since an epoch" calculation rests on a calendar. Software almost always
uses one specific idealisation — the **proleptic Gregorian** calendar with
**astronomical year numbering** — which silently disagrees with the historical record
for old dates. This chapter explains the reforms and conventions that matter.

## Julian vs Gregorian, and the 1582 reform

The Julian calendar (Julius Caesar, 45 BC) assumes a year of exactly 365.25 days —
a leap year every 4 years. That is ~11 minutes too long, and by the 16th century the
spring equinox had drifted ten days, disrupting the date of Easter.

Pope Gregory XIII corrected it with the papal bull **_Inter gravissimas_ (24 February
1582)**. In the Catholic countries that adopted it immediately, **Thursday 1582-10-04
(Julian) was followed by Friday 1582-10-15 (Gregorian)** — ten calendar days simply
vanished.

Sources: [Gregorian calendar](https://en.wikipedia.org/wiki/Gregorian_calendar),
[Julian calendar](https://en.wikipedia.org/wiki/Julian_calendar).

### Adoption was staggered by centuries

There was no civil authority beyond the Church, so adoption varied:

- **1582** — Spain, Portugal, the Papal States, much of Catholic Europe.
- **1752** — Britain and its colonies (including what became the USA), by which point
  **11 days** had to be dropped.
- Later still for Russia (1918), Greece (1923), and others.

!!! warning "Forensic consequence"
    A pre-adoption date in a historical record may be "Old Style" (Julian). Software
    using the proleptic Gregorian calendar (below) will shift it by 10–11 days, and
    the size of the shift depends on the *jurisdiction* and *century*.

### The leap-year rules differ

The two calendars share month names and lengths; they differ **only** in the
leap-year rule ([US Naval Observatory](https://aa.usno.navy.mil/data/JulianDate)):

| Calendar | Leap-year rule | Mean year |
|---|---|---|
| Julian | every year divisible by 4 | 365.25 days |
| Gregorian | divisible by 4, **except** centuries not divisible by 400 | 365.2425 days |

The Gregorian rule, applied in order:

1. divisible by 400 → leap (2000 is a leap year)
2. else divisible by 100 → **not** leap (1700, 1800, **1900** are not)
3. else divisible by 4 → leap
4. else → common year

!!! note "Why 1900 ≠ leap but 2000 = leap"
    1900 is divisible by 100 but not 400, so it is **not** a leap year. 2000 is
    divisible by 400, so it **is**. The naive "divisible by 4" rule happens to give the
    right answer for 2000 — which is exactly why buggy date code (and the
    [Excel 1900 leap-year bug](../formats/windows.md#ole-automation-date)) went
    unnoticed for so long.

## Proleptic Gregorian calendar

The **proleptic** Gregorian calendar extends the Gregorian rules *backward* before
1582, as if they had always applied. This is what
[ISO 8601](https://en.wikipedia.org/wiki/Proleptic_Gregorian_calendar), POSIX, and
nearly every date library (including `jiff`, which `timeglyph` uses) compute on.
It is internally consistent and convenient — and it disagrees with the
historically-recorded Julian date for any pre-1582 (or pre-adoption) event.

## Astronomical year numbering

ISO 8601 and most software include a **year 0**:

- year 0 = 1 BC
- year −1 = 2 BC, and so on.

The historical (Anno Domini) convention has **no year zero**: 1 BC is immediately
followed by AD 1. The two conventions therefore differ by one year for BCE dates.
`timeglyph` renders years in the astronomical convention (via `jiff`), so a BCE
instant printed here is offset by one from a historian's "BC" label. Source:
[Astronomical year numbering](https://en.wikipedia.org/wiki/Astronomical_year_numbering).

## Julian Day (JD) and Modified Julian Day (MJD)

Astronomers count time as a continuous **Julian Day number** — days (and fractions)
since a very old epoch, with no months or years to complicate arithmetic.

- **JD epoch:** noon Universal Time, **1 January 4713 BC** in the proleptic *Julian*
  calendar (= astronomical year −4712). Days start at **noon**, so the fraction `.5`
  is midnight. ([US Naval Observatory](https://aa.usno.navy.mil/data/JulianDate))
- **Unix epoch in JD:** **JD 2440587.5** = 1970-01-01 00:00:00 UTC (the `.5` because
  the day boundary is noon, not midnight).
- **Modified Julian Day:** **MJD = JD − 2400000.5**, epoch **1858-11-17 00:00:00**.
  MJD days start at midnight, removing the half-day offset.

JD and MJD appear in astronomy, satellite, RINEX/GNSS, and scientific-instrument logs,
and — importantly for forensics — as the **REAL** storage option in SQLite (see
[Unix, web & databases](../formats/unix-web-db.md#sqlite)).

!!! warning "The half-day trap"
    Because a Julian Day starts at *noon*, converting JD to civil midnight requires the
    0.5 offset. Forgetting it puts every converted instant 12 hours out. And because a
    large JD stored as an IEEE-754 `double` spends most of its bits on the integer day
    count, sub-second precision is degraded — see [Precision](precision.md#float).

## See also

- [Time scales](time-scales.md) — the leap-second layer above the calendar.
- [Formats: identifiers](../formats/identifiers.md) — UUIDv1/v6 count 100-ns intervals
  from the **1582-10-15 Gregorian reform date** itself.
