---
title: "Epoch rollovers — Y2K, Year 2038, NTP 2036, GPS week, Year 2106"
description: >-
  Every fixed-width timestamp eventually overflows. The exact instants for Y2K, the
  Year 2038 problem, the NTP 2036 era rollover, GPS week-number rollover, and Year
  2106 — and the epoch + width + signedness model that predicts them all.
---

# Epoch rollovers and representable range

A fixed-width integer timestamp can only count so far. When it overflows, dates jump —
backwards by decades, or to a wrong era. This chapter gives the exact failure instants
and the simple model that predicts every one of them.

## The model: epoch + width + signedness + unit

A fixed-width integer timestamp's representable window is fully determined by four
parameters:

- **Epoch** — the zero point (Unix 1970-01-01, NTP 1900-01-01, GPS 1980-01-06, …).
- **Width** — bits (32, 48, 64).
- **Signedness** — signed allows pre-epoch dates and halves the forward range; unsigned
  doubles the forward range with no pre-epoch.
- **Unit** — seconds vs milliseconds vs 100-ns ticks shifts both endpoints.

From the *same* Unix epoch in seconds: signed 32-bit overflows in **2038**, unsigned
32-bit in **2106**, and signed 64-bit not for ~292 billion years. To interpret any raw
integer timestamp you must establish all four parameters; guessing the epoch or
signedness produces errors of decades (rollover) to millennia (wrong epoch).

## Y2K — the year 2000 problem {#y2k}

Storing the year as two digits ("YY") cannot distinguish 1900 from 2000; the risk
materialised at **2000-01-01**. Related defects include the leap-year mishandling
(treating 2000 as a non-leap year) and the "Y2K+10" BCD bug (hex `0x10` misread as
decimal 10 in 2010). Source: [Year 2000 problem](https://en.wikipedia.org/wiki/Year_2000_problem).

!!! warning "Pivot windows live on"
    Legacy systems often store a 2-digit year with a *pivot*: e.g. 00–68 → 20xx,
    69–99 → 19xx. The same stored "15" can mean 1915 or 2015 depending on the pivot — a
    real ambiguity in old logs and file formats, and in the [CMOS RTC century
    guess](rtc-hardware.md#the-cmos-real-time-clock-motorola-mc146818).

## Year 2038 — signed 32-bit `time_t` {#y2038}

A signed 32-bit `time_t` maxes at `2³¹ − 1 = 2147483647`, which is:

> **2038-01-19 03:14:07 UTC**

The next second wraps to `−2147483648` = **1901-12-13 20:45:52 UTC** (also the earliest
representable instant). The fix is a signed 64-bit `time_t` (Linux 5.6 for 32-bit ABIs,
2020; NetBSD 6.0; OpenBSD 5.5). Sources:
[Year 2038 problem](https://en.wikipedia.org/wiki/Year_2038_problem),
[Unix time](https://en.wikipedia.org/wiki/Unix_time).

This same instant is **MySQL `TIMESTAMP`'s documented upper bound** — the 32-bit
lineage shows through (see [databases](../formats/unix-web-db.md#mysql)).

!!! tip "A 1901 or 2038 date is a tell"
    When a parsed timestamp lands on **1901-12-13** or jumps to **2038-01-19**, suspect
    a 32-bit overflow/wrap rather than a genuine date — and confirm the storage width of
    the source field.

## NTP 2036 — era rollover {#ntp-2036}

NTP counts **seconds since 1900-01-01** (the "prime epoch", era 0). Its 64-bit timestamp
format has a **32-bit unsigned seconds field**, which wraps at 2³² s:

> **2036-02-07 06:28:16 UTC** — the start of NTP era 1.

[RFC 5905](https://www.rfc-editor.org/rfc/rfc5905.txt) (NTPv4) anticipates this with a
**128-bit date format** whose 64-bit *signed* seconds field is split into a 32-bit **Era
Number** and a 32-bit Era Offset; eras are supplied externally (filesystem/hardware),
not by NTP itself. A raw 32-bit NTP value parsed without era context is ambiguous after
2036 — a near-zero value could be 1900 (era 0) or 2036 (era 1). Many embedded devices
store raw 32-bit NTP seconds. See [`timeglyph`'s NTP handling](../formats/unix-web-db.md).

## GPS week-number rollover {#gps}

GPS broadcasts date as **(week number, time-of-week)**. The legacy navigation message
carries a **10-bit week field**, which wraps every **1024 weeks (~19.6 years)**:

- GPS week 0 began **1980-01-06**.
- **First rollover:** 1999-08-21 (23:59:47 UTC).
- **Second rollover:** 2019-04-06 (23:59:42 UTC).

The modern CNAV message uses a **13-bit** week field (8192 weeks, ~157 years; next
rollover ~2137). A receiver needs the approximate date to resolve the absolute week.
Source: [IS-GPS-200 (USCG Navcen)](https://www.navcen.uscg.gov/sites/default/files/pdf/gps/IS-GPS-200N.pdf),
[GPS week number rollover](https://en.wikipedia.org/wiki/GPS_week_number_rollover).

!!! danger "Implausibly old GNSS dates"
    A GNSS device that mishandles rollover reports a date **1024 weeks (19.6 years) in
    the past** — a 2019 event logged as ~2000. This is a documented real-world failure
    (transit terminals showed wrong dates after the 2019 rollover) and a key check when a
    GNSS-derived timestamp looks too old.

## Year 2106 — unsigned 32-bit Unix seconds {#y2106}

An **unsigned** 32-bit Unix seconds value maxes at `2³² − 1 = 4294967295`:

> **2106-02-07 06:28:15 UTC**, wrapping to 0 one second later.

The time-of-day matches the NTP 2036 wrap (`06:28:16`) because both are 2³² seconds
after their epochs offset by the 1900↔1970 difference. Formats that store *unsigned*
32-bit seconds (some filesystems, network protocols) survive 2038 but fail at 2106 — the
**signedness**, not just the width, sets the cliff. Source:
[Year 2000 problem](https://en.wikipedia.org/wiki/Year_2000_problem).

## Quick reference

| Limit | Field | Instant (UTC) |
|---|---|---|
| Y2K | 2-digit year | 2000-01-01 |
| Year 2038 | signed 32-bit Unix s | 2038-01-19 03:14:07 |
| NTP era 1 | unsigned 32-bit s since 1900 | 2036-02-07 06:28:16 |
| GPS 10-bit week | 1024-week cycle | 1999-08-21, 2019-04-06, … |
| Year 2106 | unsigned 32-bit Unix s | 2106-02-07 06:28:15 |
| UUIDv7 | unsigned 48-bit ms since 1970 | year ~10889 |

## See also

- [Time scales](time-scales.md) — the GPS−UTC offset behind GPS dates.
- [RTC hardware](rtc-hardware.md) — the Y2K century guess and the RFC 868 / 2036 rollover.
- [Methodology](methodology.md) — how representable range becomes a scoring component.
