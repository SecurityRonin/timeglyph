---
title: "Unix, web & database timestamps — Unix time, WebKit, PostgreSQL, SQLite"
description: >-
  Forensic reference for Unix time (seconds/ms/µs/ns) and the Year 2038 problem,
  Chrome/WebKit time, PostgreSQL, SQLite's three encodings, MySQL DATETIME vs
  TIMESTAMP, Java/JavaScript milliseconds, and Mozilla PRTime — with citations.
---

# Unix, web & database timestamps

## Unix time {#unix}

POSIX defines **seconds since 1970-01-01 00:00:00 UTC**, with every day counted as
**exactly 86 400 seconds** — i.e. **leap-second-ignoring**
([POSIX.1-2017 §4.16, "Seconds Since the Epoch"](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap04.html)).
POSIX does not mandate width or signedness; the C `time_t` was historically signed 32-bit
and is now widely signed 64-bit.

- **Worked example:** `1577836800` = 2020-01-01.
- **Gotcha:** because leap seconds are ignored, a Unix timestamp is *not* a faithful UTC
  count across leap boundaries (see [time scales](../concepts/time-scales.md)).

### The Year 2038 problem {#y2038}

A signed 32-bit `time_t` overflows at **2038-01-19 03:14:07 UTC**, wrapping to
1901-12-13. The fix is 64-bit `time_t`. Full detail and the wider rollover family on the
[Epoch rollovers](../concepts/rollovers.md#y2038) page.

### Sub-second Unix variants {#unix-subsecond}

The same 1970 epoch appears in finer units — distinguish them by
[magnitude](../concepts/precision.md):

| Unit | Carrier | Source |
|---|---|---|
| milliseconds | Java `System.currentTimeMillis`, JavaScript `Date` | [ECMA-262 §21.4.1.1](https://tc39.es/ecma262/#sec-time-values-and-time-range) |
| microseconds | `gettimeofday`, PRTime | [gettimeofday(2)](https://man7.org/linux/man-pages/man2/gettimeofday.2.html) |
| nanoseconds | Go `time.UnixNano`, APFS | [Go `time`](https://pkg.go.dev/time) |

ECMA-262 is explicit that JavaScript time is leap-ignoring ("every day … exactly 86,400
seconds") and its `Date` range is ±8.64 × 10¹⁵ ms. Go's `UnixNano` is "undefined …
before the year 1678 or after 2262" (int64 ns). The classic misparse is **13-digit ms
read as 10-digit seconds** (or vice versa) — off by ~1000×.

## WebKit / Chrome (base::Time) {#webkit-chrome}

Chromium's `base::Time` counts **microseconds since 1601-01-01 UTC** (the Windows/FILETIME
epoch), stored as int64. From [`base/time/time.h`](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/base/time/time.h):

```cpp
// microseconds (s/1,000,000) since the Windows epoch (1601-01-01 00:00:00 UTC)
static constexpr int64_t kTimeTToMicrosecondsOffset = INT64_C(11644473600000000);
```

Used in Chromium History (`visits.visit_time`), Cookies (`creation_utc`), and autofill
SQLite stores. Convert: `unix_seconds = (value − 11644473600000000) / 1000000`.

!!! danger "Not Safari"
    This is **Chrome's** "WebKit time" (1601, microseconds). **Apple Safari** uses
    CFAbsoluteTime (2001, seconds float) — see the
    [Safari-vs-Chrome trap](apple.md#webkit-trap). They differ by ~395 years and a 10⁶
    unit scale.

## PostgreSQL {#postgresql}

`timestamp`/`timestamptz` are stored as **int64 microseconds since 2000-01-01**, per
[`src/include/datatype/timestamp.h`](https://github.com/postgres/postgres/blob/master/src/include/datatype/timestamp.h):

```c
#define UNIX_EPOCH_JDATE     2440588 /* date2j(1970, 1, 1) */
#define POSTGRES_EPOCH_JDATE 2451545 /* date2j(2000, 1, 1) */
```

The two Julian-day anchors differ by 10 957 days = 946 684 800 s — the Unix-seconds value
of 2000-01-01. `timestamptz` stores the same int64 (UTC internally) and applies the
session zone on I/O; `timestamp` carries no zone. Very old builds used a `float8`
(seconds) representation; int64 microseconds is the only representation from PostgreSQL 10
onward.

- **Worked example:** `0` = 2000-01-01; `631152000000000` = 2020-01-01.
- **Gotcha:** the **2000 epoch** is the dominant trap — decoding a PG `timestamp` against
  the Unix epoch lands 30 years early.

## SQLite {#sqlite}

SQLite has no dedicated date type; the
[date-and-time functions](https://www.sqlite.org/lang_datefunc.html) define **three**
interchangeable encodings:

1. **TEXT** — ISO-8601 strings (e.g. `2025-05-29 14:16:00`).
2. **REAL** — a **Julian day number**: "days … since -4713-11-24 12:00:00" (see
   [calendars](../concepts/calendars.md)). JD `2440587.5` = the Unix epoch.
3. **INTEGER / REAL** — a Unix timestamp via the `unixepoch` modifier.

- **Worked example:** JD `2451545.0` = 2000-01-01 12:00; `1748528160` (`unixepoch`) =
  2025-05-29 14:16.
- **Gotcha:** a bare numeric column is ambiguous — the same value can be a Julian day,
  Unix seconds, or fractional seconds. SQLite's own `'auto'` heuristic even warns that
  "Unix timestamps for the first 63 days of 1970 will be interpreted as julian day
  numbers." Julian-day REALs also lose [sub-second precision](../concepts/precision.md#float).

## MySQL {#mysql}

[MySQL](https://dev.mysql.com/doc/refman/8.0/en/datetime.html) has two distinct types:

- **`TIMESTAMP`** — Unix-based, range **1970-01-01 00:00:01 to 2038-01-19 03:14:07 UTC**
  (the [Year 2038](../concepts/rollovers.md#y2038) ceiling), TZ-converted from the
  session zone to UTC for storage.
- **`DATETIME`** — a packed calendar literal, range 1000–9999, **no** TZ conversion.

Both support microsecond fractional seconds (MySQL 5.6.4+).

- **Gotcha:** a `TIMESTAMP` read back in a different session zone displays a different
  wall-clock time than a `DATETIME`; reconstructing the original needs the connection's
  `time_zone` at write time.

## Mozilla PRTime {#prtime}

Firefox's `places.sqlite` (`moz_places.last_visit_date`, `moz_historyvisits.visit_date`,
…) uses **PRTime**: signed 64-bit **microseconds since 1970-01-01 UTC**
([NSPR `prtime.h`](https://searchfox.org/mozilla-central/source/nsprpub/pr/include/prtime.h)).

- **Worked example:** `1577836800000000` = 2020-01-01.
- **Gotcha:** same epoch as Unix but in **microseconds** (~16 digits) — the inverse of
  the WebKit trap (right epoch, wrong unit if read as ms/s). PR_Now is non-monotonic
  (NTP corrections), so adjacent events can appear out of order.

## See also

- [References](../references.md), [Precision](../concepts/precision.md),
  [Rollovers](../concepts/rollovers.md), [Calendars](../concepts/calendars.md).
