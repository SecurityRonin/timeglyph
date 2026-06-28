---
title: "Apple timestamp formats — HFS+, CFAbsoluteTime, APFS"
description: >-
  Forensic reference for Apple/macOS/iOS timestamps: HFS+ (1904, and the classic-HFS
  local-time caveat), Cocoa CFAbsoluteTime / NSDate (2001, signed double), APFS
  (nanoseconds since 1970), Core Data, and the Safari-vs-Chrome 'WebKit' trap.
---

# Apple / macOS / iOS timestamp formats

## HFS+ {#hfs}

HFS+ stores dates as **unsigned 32-bit seconds since 1904-01-01 00:00:00 GMT**, defined
in Apple Technical Note **[TN1150 "HFS Plus Volume Format"](https://developer.apple.com/library/archive/technotes/tn/tn1150.html)**.
The spec is explicit that this differs from classic HFS:

> "These dates are stored in unsigned 32-bit integers … the number of seconds since
> midnight, January 1, 1904, GMT. **This is slightly different from HFS, where the value
> represents local time.** The maximum representable date is February 6, 2040 at
> 06:28:15 GMT. The date values do not account for leap seconds."

!!! warning "Two local-time caveats"
    1. **Classic Mac HFS stored LOCAL time** with no offset — you cannot recover absolute
       UTC without knowing the machine's timezone at write time. HFS+ switched to GMT.
    2. **The HFS+ volume-header creation date is itself an exception** — TN1150 states it
       "is NOT stored in GMT; it is stored in local time", so backup tools can use it as
       a stable volume identifier. Do not normalise the volume `createDate` to UTC.

- **Range:** epoch 1904-01-01 to **2040-02-06 06:28:15 GMT** (the 2³² s overflow).
- **Gotcha:** some HFS+ auxiliary structures use a *different* epoch — e.g. the Hot Files
  B-tree time fields are "seconds since Jan 1, 1970 GMT". Verify the epoch per field.

## Cocoa / CFAbsoluteTime / NSDate {#cocoa-cfabsolutetime}

The Core Foundation / "Mac absolute time" scale: **seconds since 2001-01-01 00:00:00
UTC**, stored as a **signed IEEE-754 double**. Apple's open-source CoreFoundation makes
the definition and constants explicit (`CFDate.h` / `CFDate.c`,
[apple-oss-distributions/CF](https://github.com/apple-oss-distributions/CF/blob/main/CFDate.h)):

```c
/* the reference date (epoch) is 00:00:00 1 January 2001. */
const CFTimeInterval kCFAbsoluteTimeIntervalSince1970 = 978307200.0L;
const CFTimeInterval kCFAbsoluteTimeIntervalSince1904 = 3061152000.0L;
```

`NSDate` / Swift `Date` wrap the same scale: `timeIntervalSinceReferenceDate` **is** the
CFAbsoluteTime; `timeIntervalSince1970` = CFAbsoluteTime + 978307200. Human-facing docs:
[CFAbsoluteTime](https://developer.apple.com/documentation/corefoundation/cfabsolutetime).

- **Signed:** negative values are valid and mean **before 2001**; the fraction carries
  sub-second precision.
- **Worked example:** CFAbsoluteTime `700000000.0` → Unix `1678307200` → 2023-03-08.
- **Gotcha:** mis-reading the epoch as Unix-1970 yields a **~31-year** error (the
  978307200 offset is the tell). Small magnitudes are dates near 2001, not "epoch zero".
  On disk it is 8 raw little-endian `double` bytes — a common mistake is reading them as
  an integer.

## APFS {#apfs}

APFS inode timestamps are **`uint64` nanoseconds since 1970-01-01 00:00 UTC**, per the
[Apple File System Reference](https://developer.apple.com/support/downloads/Apple-File-System-Reference.pdf)
(`j_inode_val_t`): `create_time`, `mod_time`, `change_time`, `access_time`, each "the
number of nanoseconds since January 1, 1970 at 0:00 UTC, disregarding leap seconds."

- **Worked example:** `1500000000000000000` ns = 2017-07-14.
- **Gotcha:** divide by 1 000 000 000 to reach Unix seconds. APFS gives a true four-stamp
  (B/M/C/A) set at **nanosecond** resolution — finer than HFS+ (1 s) — so sub-second
  event ordering is meaningful. (`access_time` updates are flag-governed and not reliably
  wall-accurate.)

## Core Data {#core-data}

Core Data persists `Date`/`NSDate` attributes in its SQLite store as the
**CFAbsoluteTime value** (the 2001-epoch signed double, in a `REAL` column). The scale is
the verified CFAbsoluteTime scale above; the specific on-disk column encoding is a strong
forensic convention rather than an Apple-documented guarantee — confirm empirically (read
the raw `REAL`, add 978307200, sanity-check).

!!! note
    The same `.sqlite` file may also hold Unix-epoch integers in non-Core-Data tables.
    Never assume a single epoch across all time columns — detect per column.

## The Safari-vs-Chrome "WebKit" trap {#webkit-trap}

"WebKit" is overloaded, and the two meanings differ by ~395 years *and* a 10⁶ unit scale:

| Source | Scale |
|---|---|
| **Apple Safari** (the WebKit browser engine) — e.g. `History.db` `visit_time` | **CFAbsoluteTime: 2001 epoch, seconds (float)** |
| **Chromium "WebKit time"** (`base::Time`) — Chrome history/cookies | **1601 epoch, microseconds (int64)** — see [Unix/web/DB](unix-web-db.md#webkit-chrome) |

!!! danger
    Identify the browser first. Reading a Safari `visit_time` (2001 epoch seconds) with
    Chrome's 1601-epoch-microsecond rule — or vice versa — produces a wildly wrong date.
    (Apple does not publish a `History.db` schema; the 2001-epoch double convention is
    established forensic practice — verify empirically.)

## Quick reference

| Format | Epoch | Unit | Type | TZ |
|---|---|---|---|---|
| Classic HFS | 1904-01-01 | seconds | UInt32 | **local** |
| HFS+ | 1904-01-01 GMT | seconds | UInt32 | UTC (vol-header createDate = local) |
| CFAbsoluteTime / NSDate | 2001-01-01 UTC | seconds | **signed double** | UTC |
| APFS | 1970-01-01 UTC | **nanoseconds** | uint64 | UTC |

Constants: `kCFAbsoluteTimeIntervalSince1970 = 978307200`;
`kCFAbsoluteTimeIntervalSince1904 = 3061152000`; 1904→1970 = 2 082 844 800 s.

## See also

- [References](../references.md), [Precision](../concepts/precision.md),
  [Time scales](../concepts/time-scales.md).
