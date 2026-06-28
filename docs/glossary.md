---
title: "Glossary — timestamp & time-scale terms"
description: >-
  Definitions of the core terms in forensic timestamp analysis: epoch, tick, leap
  second, proleptic Gregorian, MACB, BCD, era, leap smear, sentinel, and more.
---

# Glossary

**Epoch** — the zero point a timestamp counts from (Unix 1970-01-01, FILETIME
1601-01-01, Cocoa 2001-01-01, …). See [format reference](formats/index.md).

**Tick / unit** — the smallest increment a format counts in: seconds,
milliseconds, microseconds, 100-nanoseconds (FILETIME/.NET), or nanoseconds. See
[Precision](concepts/precision.md).

**POSIX time** — seconds since 1970 counting every day as exactly 86 400 s, i.e.
**leap-second-ignoring**. Not the same as UTC around a leap second. timeglyph's
internal spine is `PosixNs` for this reason.

**UTC / TAI / GPS / UT1** — the [time scales](concepts/time-scales.md). UTC is
civil time (with leap seconds); TAI is continuous atomic time; GPS is continuous
from 1980; UT1 tracks Earth's rotation.

**Leap second** — a ±1 s adjustment to UTC (a real `23:59:60`) to track Earth's
rotation, decided by the IERS. 27 inserted since 1972; being phased out by ~2035.

**Leap smear** — spreading a leap second over a window (Google/AWS/Meta) so no
second repeats; invisible in a raw timestamp.

**Julian / Gregorian** — the [calendars](concepts/calendars.md). The 1582 Gregorian
reform dropped 10 days; the leap-year rules differ at century boundaries.

**Proleptic Gregorian** — the Gregorian calendar extended backward before 1582;
what ISO 8601 and most software (incl. `jiff`) compute on.

**Astronomical year numbering** — includes a year 0 (= 1 BC); used by ISO 8601.

**Julian Day (JD)** — a continuous day count from noon, 24 Nov 4714 BC; JD
2440587.5 = the Unix epoch. **MJD** = JD − 2400000.5.

**MACB** — the four NTFS/forensic timestamps: **M**odified, **A**ccessed,
**C**reated (born), MFT-changed. See [NTFS SI vs FN](formats/windows.md#ntfs).

**FILETIME** — Windows 64-bit count of 100-ns intervals since 1601 UTC.

**Snowflake** — a 64-bit ID with a millisecond timestamp in its high bits
(Twitter/X, Discord). See [identifiers](formats/identifiers.md).

**BCD (binary-coded decimal)** — each decimal digit stored in 4 bits; used by the
[MC146818 RTC](concepts/rtc-hardware.md). `0x59` = 59, not 89.

**Era** — a rollover generation: NTP's 32-bit seconds wrap into era 1 in 2036;
GPS's 10-bit week wraps every 1024 weeks. See [rollovers](concepts/rollovers.md).

**Sentinel** — a magic value (`0`, all-ones, `0x7FFFFFFFFFFFFFFF`) meaning
unset/never/error, not a real instant. See [sentinel values](concepts/sentinel-values.md).

**Encoding vs format** — *format* is the semantics (epoch/unit); *encoding* is the
presentation (radix/endianness/width/packing). See [input conventions](concepts/input-conventions.md).

**Tier-1 validation** — a check where an independent third party authored both the
input and the answer (a spec worked example or an independent oracle tool). See
[Validation](validation.md).
