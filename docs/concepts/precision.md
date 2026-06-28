---
title: "Precision — seconds, milliseconds, microseconds, nanoseconds"
description: >-
  The sub-second precision ladder and how to disambiguate it by magnitude; why a
  storage unit is not the clock's accuracy; real OS clock granularity on Windows
  and Linux; IEEE-754 double limits; and filesystem timestamp resolution.
---

# Precision: seconds → milliseconds → microseconds → nanoseconds

Two integers can name the *same* instant in wildly different units. Telling them
apart — and knowing how much of the apparent precision is real — is central to
reading a raw timestamp correctly.

## The precision ladder

For a "current-era" value (roughly 2001–2030), the decimal **magnitude** is the
first-order clue to the unit:

| Unit | Per second | Digits (current era) | Typical carrier |
|---|---|---|---|
| seconds | 1 | ~10 (`1577836800` = 2020-01-01) | Unix `time_t`, MongoDB ObjectId |
| milliseconds | 10³ | ~13 | Java `currentTimeMillis`, JS `Date.now()`, Snowflake |
| microseconds | 10⁶ | ~16 | `gettimeofday`, Chrome/WebKit, PostgreSQL, PRTime |
| 100-nanoseconds | 10⁷ | ~17–18 | Windows FILETIME, .NET ticks |
| nanoseconds | 10⁹ | ~19 | `clock_gettime`, APFS, Go `UnixNano` |

This magnitude heuristic is exactly what `timeglyph`'s `granularity_match` score
encodes, and the principle is stated in [RFC 9562 §6.1](https://www.rfc-editor.org/rfc/rfc9562.txt):
"Many levels of precision exist for timestamps: milliseconds, microseconds,
nanoseconds, and beyond." The heuristic is only a first cut — it is ambiguous near
epoch boundaries and for far-past/future values, so corroborate with the carrier
(file format, epoch).

## Resolution vs. accuracy vs. precision — the unit is not the clock

The storage **unit** (the smallest representable increment) is an *upper bound* on
the clock's true **granularity**, not a measure of it. A value can be stored in
100-nanosecond units yet only change every ~15 milliseconds.

[RFC 9562 §6.1](https://www.rfc-editor.org/rfc/rfc9562.txt) again: "system clocks
themselves have an underlying granularity, which is frequently less than the precision
offered by the operating system." POSIX exposes the gap directly through
[`clock_getres()`](https://man7.org/linux/man-pages/man2/clock_gettime.2.html).

!!! tip "Read the trailing digits"
    A column of FILETIME values all ending in the same large round number of 100-ns
    units (e.g. multiples of ~156 250 = 15.625 ms) was produced by a coarse system-tick
    clock, **not** a genuine 100-ns clock. The low digits are quantization, not
    information. Conversely, non-zero low digits imply a high-resolution source.

## Real OS clock granularity

### Windows

- `GetSystemTimeAsFileTime` is tied to the system timer tick. Microsoft documents the
  [default tick interval](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/high-resolution-timers)
  as "typically about 15 milliseconds, and the minimum interval … about 1 millisecond"
  on x86. [`timeGetTime`](https://learn.microsoft.com/en-us/windows/win32/api/timeapi/nf-timeapi-timegettime)
  notes "default precision … can be five milliseconds or more."
- [`GetSystemTimePreciseAsFileTime`](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemtimepreciseasfiletime)
  (Windows 8 / Server 2012+) gives "the highest possible level of precision (<1us)".

So two FILETIME values from the same machine may carry 100-ns *units* but differ in
real resolution by four orders of magnitude depending on which API wrote them.

### Linux / Unix

- [`gettimeofday(2)`](https://man7.org/linux/man-pages/man2/gettimeofday.2.html) carries
  **microsecond** units (`tv_usec`) and is now obsolescent.
- [`clock_gettime(2)`](https://man7.org/linux/man-pages/man2/clock_gettime.2.html) with
  `CLOCK_REALTIME` carries **nanosecond** units (`tv_nsec`); the faster
  `CLOCK_REALTIME_COARSE` variant only resolves to the kernel tick.

!!! note "Spotting a widened value"
    A Unix nanosecond timestamp whose low three digits are always `000` came from a
    microsecond source (`gettimeofday`) widened to nanoseconds. Genuine
    `clock_gettime` output has non-zero nanosecond digits.

## Float precision limits {#float}

An IEEE-754 **double** has a 52-bit fraction (~15–17 significant decimal digits). When
a date is stored as *days* in a double, the integer-day magnitude consumes most of
those bits, degrading sub-second resolution:

- **SQLite Julian-day REAL** — a JD near `2.46e6` leaves only enough fraction for
  ~tens-of-microseconds resolution over a day, and round-tripping can perturb the
  seconds ([SQLite date functions](https://www.sqlite.org/lang_datefunc.html)).
- **OLE Automation date** (`double` days since 1899-12-30) — sub-second precision near
  year-2000 values is limited to ~tens of microseconds and is not exact
  ([DateTime.ToOADate](https://learn.microsoft.com/en-us/dotnet/api/system.datetime.tooadate)).

!!! warning
    Never assert exact sub-second values from a float-backed date column. If both a
    float date and an integer carrier (FILETIME/tick/`unixepoch`) are present, trust
    the integer.

## Filesystem unit vs. recorded resolution

A filesystem's *unit* and its *recorded resolution* often differ:

| Field | Storage unit | Effective resolution |
|---|---|---|
| FAT write/modify time | "seconds ÷ 2" (5 bits) | **2 seconds** |
| FAT creation time | + a separate tenths/10-ms byte | **10 ms** (creation only) |
| FAT last-access | date only | **1 day** |
| exFAT timestamp | 2-second base + 10-ms increment | 2 s, refined to 10 ms |
| NTFS (FILETIME) | 100-ns units | 100-ns unit (clock coarser; see above) |

Layout source: [DosDateTimeToFileTime](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-dosdatetimetofiletime),
[exFAT specification §7.4.8](https://learn.microsoft.com/en-us/windows/win32/fileio/exfat-specification).

!!! tip "Impossible odd seconds"
    Because FAT write times are quantized to *even* seconds, an **odd-second FAT modify
    time is impossible** — if you see one, the value was not produced by FAT, or it has
    been tampered with. See [Microsoft / Windows formats](../formats/windows.md#fat-dos).

## See also

- [RTC hardware](rtc-hardware.md) — the 18.2 Hz PC tick quantizes DOS sub-second times.
- [Formats: Windows](../formats/windows.md) and [Apple](../formats/apple.md) — per-format
  resolution notes.
