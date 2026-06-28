---
title: "Timestamp format reference — epochs, encodings, citations"
description: >-
  A master table of forensic timestamp formats with their epoch, unit, byte width,
  timezone and leap semantics, and primary-source citation. FILETIME, Unix, Cocoa,
  HFS+, .NET, OLE, PostgreSQL, SQLite, Snowflake, GPS, TAI64, NTP and more.
---

# Format reference

Each format below counts from a particular **epoch** in a particular **unit**, with
particular **timezone** and **leap-second** semantics, and a representable range fixed
by its width and signedness. The detail pages give the byte layout, the historical
evolution, a worked example, and the forensic gotchas, each tied to a primary source.

## Master table

| Format | Epoch | Unit | Width | TZ | Family |
|---|---|---|---|---|---|
| [Unix](unix-web-db.md#unix) | 1970-01-01 | seconds | 32/64-bit | UTC | [Unix/web/DB](unix-web-db.md) |
| [Unix ms](unix-web-db.md#unix-subsecond) | 1970-01-01 | milliseconds | 64-bit | UTC | [Unix/web/DB](unix-web-db.md) |
| [Unix µs / ns](unix-web-db.md#unix-subsecond) | 1970-01-01 | µs / ns | 64-bit | UTC | [Unix/web/DB](unix-web-db.md) |
| [FILETIME](windows.md#filetime) | 1601-01-01 | 100 ns | 64-bit | UTC | [Windows](windows.md) |
| [WebKit/Chrome](unix-web-db.md#webkit-chrome) | 1601-01-01 | microseconds | 64-bit | UTC | [Unix/web/DB](unix-web-db.md) |
| [.NET ticks](windows.md#net-ticks) | 0001-01-01 | 100 ns | 64-bit | varies | [Windows](windows.md) |
| [OLE Automation](windows.md#ole-automation-date) | 1899-12-30 | days (float) | f64 | local/UTC | [Windows](windows.md) |
| [FAT/DOS](windows.md#fat-dos) | 1980-01-01 | packed (2 s) | 32-bit | **local** | [Windows](windows.md) |
| [Cocoa / CFAbsoluteTime](apple.md#cocoa-cfabsolutetime) | 2001-01-01 | seconds | f64 (signed) | UTC | [Apple](apple.md) |
| [HFS+](apple.md#hfs) | 1904-01-01 | seconds | u32 | UTC* | [Apple](apple.md) |
| [APFS](apple.md#apfs) | 1970-01-01 | nanoseconds | u64 | UTC | [Apple](apple.md) |
| [PostgreSQL](unix-web-db.md#postgresql) | 2000-01-01 | microseconds | int64 | UTC | [Unix/web/DB](unix-web-db.md) |
| [SQLite Julian](unix-web-db.md#sqlite) | −4713-11-24 noon | days (float) | f64 | UTC | [Unix/web/DB](unix-web-db.md) |
| [Snowflake / Discord](identifiers.md#snowflake) | 2010 / 2015 | ms (shifted) | 64-bit | UTC | [Identifiers](identifiers.md) |
| [UUIDv1 / v6](identifiers.md#uuid) | 1582-10-15 | 100 ns | 60-bit | UTC | [Identifiers](identifiers.md) |
| [UUIDv7 / ULID](identifiers.md#uuid) | 1970-01-01 | milliseconds | 48-bit | UTC | [Identifiers](identifiers.md) |
| [GPS / TAI64 / NTP](#leap-aware-family) | varies | s / s / s | — | leap-aware | leap module |

\* HFS+ stores UTC, but classic HFS stored **local** time, and the HFS+ volume-header
creation date is an explicit local-time exception — see [Apple](apple.md#hfs).

## Detail pages

- **[Microsoft / Windows](windows.md)** — FILETIME, OLE Automation (and the Excel 1900
  leap-year bug), .NET ticks, SYSTEMTIME, Active Directory / LDAP, FAT/DOS, exFAT, the
  NTFS `$STANDARD_INFORMATION` vs `$FILE_NAME` timestomping distinction, and ZIP.
- **[Apple / macOS / iOS](apple.md)** — HFS+ (and the classic-HFS local-time caveat),
  Cocoa / CFAbsoluteTime, APFS, Core Data, and the Safari-vs-Chrome "WebKit" trap.
- **[Unix, web & databases](unix-web-db.md)** — Unix s/ms/µs/ns, the Year 2038 problem,
  WebKit/Chrome, PostgreSQL, SQLite's three encodings, MySQL, Java/JS, and PRTime.
- **[Identifiers](identifiers.md)** — Snowflake (Twitter/X, Discord), UUID v1/v6/v7,
  ULID, MongoDB ObjectId, KSUID, Sonyflake.

## Leap-aware family

GPS, TAI64, and NTP are **not** routed through the POSIX spine — they need true leap
arithmetic or era handling. In `timeglyph` they live behind the `leap` feature and are
decoded via [`hifitime`](https://docs.rs/hifitime) (GPS/TAI) or additive `jiff` (NTP):

```console
$ cargo build --features leap
$ timeglyph decode gps 1261872018      # → 2020-01-01T00:00:00Z (leap-correct UTC)
$ timeglyph decode tai64 4611686020005224741
$ timeglyph decode ntp 3786825600
```

See [time scales](../concepts/time-scales.md) for the GPS−UTC = 18 s and TAI−UTC = 37 s
offsets these apply.
