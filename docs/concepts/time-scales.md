---
title: "Time scales & leap seconds — UTC, TAI, GPS, UT1"
description: >-
  How UTC, TAI, GPS time and UT1 relate; what leap seconds are and who decides
  them; why POSIX/Unix time ignores leap seconds; and how Google/AWS/Meta smear
  them. With IERS, BIPM and RFC citations.
---

# Time scales & leap seconds

A timestamp is meaningless until you know which **time scale** it counts on. Most
forensic formats label themselves "UTC", but the family that actually needs leap
arithmetic — GPS, TAI, NTP — is small and separate. This chapter defines the scales,
their relationships, and the leap-second machinery that makes them diverge.

## The four scales you meet in practice

| Scale | What it is | Leap seconds? | Relationship (2017→present) |
|---|---|---|---|
| **TAI** — International Atomic Time | A continuous count of SI seconds from ~450 atomic clocks; the reference for all others | No | the uniform reference |
| **UTC** — Coordinated Universal Time | Civil time: TAI stepped by whole leap seconds to track Earth's rotation | Yes | **TAI − UTC = 37 s** |
| **GPS time** | Continuous; set equal to UTC at the 1980-01-06 GPS epoch and never stepped since | No | **GPS − UTC = 18 s**; TAI − GPS = 19 s (constant) |
| **UT1** | Astronomical time from the Earth's actual rotation angle | n/a | UTC is kept within 0.9 s of UT1 |

The current offsets are authoritative IERS values: the cumulative **TAI − UTC = 37 s**
is published in the [IERS leap-second table](https://hpiers.obspm.fr/iers/bul/bulc/Leap_Second.dat).
GPS is 19 s behind TAI by construction, hence 18 s ahead of UTC after the 2016 leap
second. The GPS navigation message carries the GPS−UTC offset so receivers can output
UTC ([IS-GPS-200, USCG Navcen](https://www.navcen.uscg.gov/sites/default/files/pdf/gps/IS-GPS-200N.pdf)).

!!! warning "Forensic consequence"
    A GPS-sourced timestamp (vehicle telematics, a GNSS logger, a phone's GNSS
    subsystem) is **18 seconds ahead** of the UTC an analyst expects. In a
    high-resolution timeline that is enough to invert event ordering. Always
    establish the source scale before comparing instants across devices.

## Leap seconds

The SI second is fixed; the Earth's rotation is not (it slows irregularly, ~1.5 ms
per day per century). To keep civil time aligned with the sky, UTC is occasionally
nudged by a **leap second** — an extra `23:59:60` inserted (or, in principle, a
second removed) at the end of a UTC month.

- **Who decides:** the [International Earth Rotation and Reference Systems Service
  (IERS)](https://www.iers.org/), announced in **Bulletin C** about six months ahead.
- **When:** preferentially at the end of 30 June or 31 December. Every leap second to
  date has fallen on one of those two dates, and **all have been positive**.
- **How many:** **27 leap seconds** have been inserted since 1972 (TAI − UTC rose from
  10 s on 1972-01-01 to 37 s). The most recent was **2016-12-31 23:59:60 UTC**.

Source: [IERS leap-second data file](https://hpiers.obspm.fr/iers/bul/bulc/Leap_Second.dat).

### The 2035 phase-out

On 18 November 2022 the 27th General Conference on Weights and Measures adopted
**[Resolution 4 (CGPM 2022)](https://www.bipm.org/en/cgpm-2022/resolution-4)**,
directing that the maximum permitted `|UT1 − UTC|` be increased "in, or before, 2035".
In practice this ends the insertion of new leap seconds for at least a century. It
simplifies *future* reasoning but changes nothing about the 27 historical insertions
already baked into any pre-2035 UTC timeline.

## Why POSIX/Unix time is not a count of SI seconds

POSIX defines a day as **exactly 86 400 seconds** and Unix time as a simple count on
that basis ([POSIX.1-2017 §4.16](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap04.html)).
It therefore **does not count leap seconds**. Two consequences matter forensically:

1. **Repeated values.** During a positive leap second a conforming Unix clock runs
   into the next day, then jumps back one second — so two distinct real instants can
   share the **same Unix timestamp**.
2. **Undercounted intervals.** Subtracting two Unix timestamps undercounts the true
   elapsed SI time by the number of leap seconds between them.

Unix time is a *civil-time label*, not an elapsed-physical-time counter. `timeglyph`
makes this explicit: its internal spine is named `PosixNs`, **not** UTC, precisely
because the two differ around leap seconds.

## Leap smearing — and why a raw value can't reveal it

Large operators avoid the awkward `23:59:60` by **smearing** the leap second: they
spread it across a window so no second is repeated or skipped, and clocks read
slightly off true UTC during the window.

| Operator | Smear window | Source |
|---|---|---|
| **Google** | 24 hours, linear, noon-to-noon UTC (centred on the leap) | [Google Public NTP — Leap Smear](https://developers.google.com/time/smear) |
| **AWS** | same 24-hour noon-to-noon smear | [AWS — Look Before You Leap](https://aws.amazon.com/blogs/aws/look-before-you-leap-the-coming-leap-second-and-aws/) |
| **Meta** | ~18 hours, sine-shaped, after 00:00 UTC | [Meta — NTP at scale](https://engineering.fb.com/2020/03/18/production-engineering/ntp-service/) |

!!! danger "The leap-smear disclaimer"
    A raw timestamp **cannot tell you whether its source smeared a leap second.**
    During a smear window a recorded time can deviate from true UTC by up to ~1 s, and
    the deviation depends on the vendor and the position within the window. Correlating
    logs from a smeared cloud (GCP/AWS) against an unsmeared on-prem clock around a leap
    date can produce sub-second skew that is an *artifact*, not evidence. Every POSIX
    reading in `timeglyph` carries this disclaimer in its assumptions.

## See also

- [Calendars](calendars.md) — the proleptic-Gregorian assumption underneath every scale.
- [Epoch rollovers](rollovers.md) — GPS week rollover and the NTP era problem.
- [Methodology](methodology.md) — how these caveats become first-class scored output.
