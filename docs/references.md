---
title: "References — authoritative timestamp specifications"
description: >-
  The primary-source bibliography behind timeglyph: RFCs, Microsoft Open
  Specifications, Apple technotes, POSIX, ECMA, IS-GPS-200, IERS, BIPM, and vendor
  source code for every timestamp format and time scale documented here.
---

# References

Every fact on this site traces to a primary specification or authoritative source.
URLs were verified to resolve to real content during research; where a primary source
could not be reached or does not exist, that is stated plainly.

## Time scales, leap seconds, calendars

- **POSIX.1-2017 / IEEE Std 1003.1, Base Definitions §4.16 "Seconds Since the Epoch"** — [pubs.opengroup.org](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap04.html)
- **IERS leap-second data file** (TAI−UTC table) — [hpiers.obspm.fr](https://hpiers.obspm.fr/iers/bul/bulc/Leap_Second.dat); IERS Bulletin C — [iers.org](https://www.iers.org/)
- **BIPM — Resolution 4 of the 27th CGPM (2022)** (leap-second phase-out by 2035) — [bipm.org](https://www.bipm.org/en/cgpm-2022/resolution-4)
- **US Naval Observatory — Julian Date / calendar notes** — [aa.usno.navy.mil](https://aa.usno.navy.mil/data/JulianDate)
- Leap smearing (vendor primary): **Google Public NTP** — [developers.google.com/time/smear](https://developers.google.com/time/smear); **AWS** — [aws.amazon.com](https://aws.amazon.com/blogs/aws/look-before-you-leap-the-coming-leap-second-and-aws/); **Meta** — [engineering.fb.com](https://engineering.fb.com/2020/03/18/production-engineering/ntp-service/)

## Microsoft / Windows

- **[MS-DTYP] §2.3.3 FILETIME** — [learn.microsoft.com](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-dtyp/2c57429b-fdd4-488f-b5fc-9e4cf020fcdf)
- **Win32 File Times** (NTFS UTC, FAT local) — [learn.microsoft.com](https://learn.microsoft.com/en-us/windows/win32/sysinfo/file-times)
- **OLE Automation DATE** — `VariantTimeToSystemTime` — [learn.microsoft.com](https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-varianttimetosystemtime)
- **Excel 1900 leap-year bug** — [learn.microsoft.com](https://learn.microsoft.com/en-us/office/troubleshoot/excel/wrongly-assumes-1900-is-leap-year)
- **.NET `System.DateTime` / `.Ticks`** — [learn.microsoft.com](https://learn.microsoft.com/en-us/dotnet/api/system.datetime.ticks) (ECMA-335 BCL)
- **SYSTEMTIME** — [learn.microsoft.com](https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-systemtime)
- **Active Directory:** `pwdLastSet` [MS-ADA3 §2.175](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-ada3/9663282b-c880-4061-ba8e-e8509c8aa336); `lastLogonTimestamp` [MS-ADA1 §2.352](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-ada1/530d7194-20f6-4aaa-8d80-9ca6b6350ad6)
- **FAT:** `DosDateTimeToFileTime` — [learn.microsoft.com](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-dosdatetimetofiletime); Microsoft FAT32 specification (fatgen103) — [download.microsoft.com](https://download.microsoft.com/download/1/6/1/161ba512-40e2-4cc9-843a-923143f3456c/fatgen103.doc)
- **exFAT specification §7.4** — [learn.microsoft.com](https://learn.microsoft.com/en-us/windows/win32/fileio/exfat-specification)
- **.ZIP APPNOTE v6.3.10** — [pkware.cachefly.net](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT)
- **C runtime `time`, `_time32`, `_time64`** (Year 2038) — [learn.microsoft.com](https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/time-time32-time64)
- Windows clock granularity: [High-Resolution Timers](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/high-resolution-timers), [GetSystemTimePreciseAsFileTime](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemtimepreciseasfiletime)

!!! note "Non-primary by necessity"
    The on-disk NTFS `$STANDARD_INFORMATION` / `$FILE_NAME` MFT attribute byte layout
    has **no** Microsoft open specification; only the FILETIME unit/epoch and UTC-on-disk
    storage are from Microsoft primaries. The SI-vs-FN timestomping distinction is from
    established forensic literature (The Sleuth Kit; Carrier, *File System Forensic
    Analysis*).

## Apple

- **TN1150 "HFS Plus Volume Format"** — [developer.apple.com/library/archive](https://developer.apple.com/library/archive/technotes/tn/tn1150.html)
- **CFAbsoluteTime / NSDate** — [developer.apple.com](https://developer.apple.com/documentation/corefoundation/cfabsolutetime); constants in Apple CoreFoundation source [CFDate.h](https://github.com/apple-oss-distributions/CF/blob/main/CFDate.h)
- **Apple File System Reference (APFS)** PDF — [developer.apple.com](https://developer.apple.com/support/downloads/Apple-File-System-Reference.pdf)

!!! note "Verification caveat"
    Modern `developer.apple.com/documentation/…` pages are single-page apps that serve no
    machine-readable text to a plain fetch; the CFAbsoluteTime constants here are anchored
    to Apple's published CoreFoundation **source**. Core Data's and Safari `History.db`'s
    on-disk column encodings are established forensic conventions, not Apple-documented
    guarantees — confirm empirically.

## Unix, web & databases

- **WebKit/Chrome `base::Time`** — [chromium.googlesource.com](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/base/time/time.h)
- **PostgreSQL `timestamp.h`** — [github.com/postgres](https://github.com/postgres/postgres/blob/master/src/include/datatype/timestamp.h)
- **SQLite date-and-time functions** — [sqlite.org](https://www.sqlite.org/lang_datefunc.html)
- **MySQL DATE/DATETIME/TIMESTAMP** — [dev.mysql.com](https://dev.mysql.com/doc/refman/8.0/en/datetime.html)
- **ECMA-262 (JavaScript Date) §21.4.1.1** — [tc39.es](https://tc39.es/ecma262/#sec-time-values-and-time-range)
- **Go `time` package** — [pkg.go.dev/time](https://pkg.go.dev/time)
- **Linux man pages:** [clock_gettime(2)](https://man7.org/linux/man-pages/man2/clock_gettime.2.html), [gettimeofday(2)](https://man7.org/linux/man-pages/man2/gettimeofday.2.html), [hwclock(8)](https://man7.org/linux/man-pages/man8/hwclock.8.html)
- **Mozilla NSPR `prtime.h`** — [searchfox.org](https://searchfox.org/mozilla-central/source/nsprpub/pr/include/prtime.h)

## Identifiers

- **RFC 9562 (UUID, obsoletes RFC 4122)** — [rfc-editor.org](https://www.rfc-editor.org/rfc/rfc9562.txt)
- **Twitter Snowflake** — [github.com/twitter-archive/snowflake](https://github.com/twitter-archive/snowflake/blob/snowflake-2010/README.mkd)
- **Discord — Snowflakes** — [discord.com/developers](https://discord.com/developers/docs/reference#snowflakes)
- **ULID spec** — [github.com/ulid/spec](https://github.com/ulid/spec)
- **MongoDB ObjectId** — [mongodb.com](https://www.mongodb.com/docs/manual/reference/method/ObjectId/)
- **KSUID** — [github.com/segmentio/ksuid](https://github.com/segmentio/ksuid)
- **Sonyflake** — [github.com/sony/sonyflake](https://github.com/sony/sonyflake)

## Time scales: GPS, TAI64, NTP

- **IS-GPS-200 (NAVSTAR GPS Interface Specification)** — [navcen.uscg.gov](https://www.navcen.uscg.gov/sites/default/files/pdf/gps/IS-GPS-200N.pdf)
- **RFC 5905 (NTPv4)** — [rfc-editor.org](https://www.rfc-editor.org/rfc/rfc5905.txt)
- **TAI64 (D. J. Bernstein, libtai)** — [cr.yp.to/libtai/tai64.html](https://cr.yp.to/libtai/tai64.html)
- **RFC 868 (Time Protocol)** — [rfc-editor.org](https://www.rfc-editor.org/rfc/rfc868)

## Hardware / older clocks

- **OSDev wiki** (community reference; live host blocks bots, archive snapshots cited): [CMOS](https://web.archive.org/web/20241222182649/https://wiki.osdev.org/CMOS), [RTC](https://web.archive.org/web/20241204080709/https://wiki.osdev.org/RTC), [PIT](https://web.archive.org/web/20241229071829/https://wiki.osdev.org/Programmable_Interval_Timer), [Time And Date](https://web.archive.org/web/20241227171812/https://wiki.osdev.org/Time_And_Date)

## Validation oracle

- **time-decode** (Corey Forman / digitalsleuth, MIT) — the independent
  third-party differential oracle used in [Validation](validation.md) — [github.com/digitalsleuth/time_decode](https://github.com/digitalsleuth/time_decode)

## Libraries timeglyph builds on

- **jiff** (civil time, IANA tz) — [docs.rs/jiff](https://docs.rs/jiff)
- **hifitime** (leap-aware TAI/GPS) — [docs.rs/hifitime](https://docs.rs/hifitime)
