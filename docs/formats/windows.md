---
title: "Microsoft / Windows timestamp formats — FILETIME, OLE, .NET, FAT"
description: >-
  Forensic reference for Windows timestamps: FILETIME (1601, 100 ns), OLE Automation
  dates and the Excel 1900 leap-year bug, .NET ticks, SYSTEMTIME, Active Directory /
  LDAP, FAT/DOS, exFAT, NTFS $STANDARD_INFORMATION vs $FILE_NAME, and ZIP.
---

# Microsoft / Windows timestamp formats

## FILETIME {#filetime}

A 64-bit count of **100-nanosecond intervals since 1601-01-01 00:00:00 UTC**, defined in
**[MS-DTYP §2.3.3](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-dtyp/2c57429b-fdd4-488f-b5fc-9e4cf020fcdf)**.
On disk it is two little-endian 32-bit halves; recombine as
`(dwHighDateTime << 32) | dwLowDateTime`. UTC by definition; leap seconds are not
counted. It underlies NTFS file times, the registry, the Windows Event Log, and Active
Directory `Integer8` timestamps.

- **Worked example:** `116444736000000000` = 1970-01-01 (the FILETIME→Unix anchor);
  `132223104000000000` = 2020-01-01.
- **Gotcha:** the 1601 epoch and 100-ns unit make values ~18 digits — easily confused
  with Unix nanoseconds (1970 epoch). The 1601→1970 gap is 11 644 473 600 s.

## OLE Automation date {#ole-automation-date}

An 8-byte IEEE-754 **double**: the integer part is days, the fraction is the time of
day, counted from the **1899-12-30** epoch. The authoritative description is in the
Win32 conversion API
[`VariantTimeToSystemTime`](https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-varianttimetosystemtime):
"2.0 represents January 1, 1900 … 2.5 represents noon on January 1, 1900".

### Why 1899-12-30, and the Excel 1900 leap-year bug

The epoch is 1899-12-30, not 1900-01-01, to stay aligned with **Excel's** day numbering
— and Excel's numbering contains a deliberate error. Excel
[treats 1900 as a leap year](https://learn.microsoft.com/en-us/office/troubleshoot/excel/wrongly-assumes-1900-is-leap-year)
(it is not — 1900 is divisible by 100 but not 400; see [calendars](../concepts/calendars.md)).
The phantom `1900-02-29` exists as serial 60. Excel inherited this from **Lotus 1-2-3**
for compatibility and has never fixed it. Offsetting the OLE epoch back to 1899-12-30
makes OLE `2.0` line up with Excel serial 2 = 1900-01-01 for all dates from 1900-03-01
onward.

### The 1900 vs 1904 date system

Excel for the original Macintosh used a **1904 date system** (epoch 1904-01-01) to dodge
the 1900 issue; Windows Excel defaults to 1900. The two differ by **1462 days**, so a
workbook moved between systems shifts every date by that amount.

- **Gotcha:** as a `double`, sub-second precision is limited (see [precision](../concepts/precision.md#float)).
  Always determine which date system a workbook uses before trusting a raw serial.

## .NET DateTime.Ticks {#net-ticks}

A 64-bit signed count of **100-nanosecond ticks since 0001-01-01 00:00:00**, proleptic
Gregorian (`DateTime.MinValue`), per the .NET BCL (ECMA-335). 10 000 ticks/ms;
10 000 000 ticks/s. Interpretation depends on the value's `Kind` (`Utc`, `Local`,
`Unspecified`); leap seconds are not included.

- **Worked example:** `630822816000000000` = 2000-01-01.
- **Gotcha:** different epoch (year 1) from FILETIME (1601) — the offset is
  `504911232000000000` ticks. A `Kind=Unspecified` value carries no zone.

## SYSTEMTIME {#systemtime}

A packed 16-byte struct of eight `WORD` fields (year, month, day-of-week, day, hour,
minute, second, milliseconds), defined in
[minwinbase.h](https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-systemtime).
It is **not** an offset count, carries **no zone tag** (UTC via `GetSystemTime`, local
via `GetLocalTime`), and does not validate the date — invalid dates can be stored.

## Active Directory / LDAP {#ad-ldap}

AD stores `pwdLastSet`, `lastLogonTimestamp`, `lastLogon`, `accountExpires`,
`badPasswordTime` as **`Integer8`** (signed 64-bit) values that are **FILETIME**
counts — "100 nanosecond intervals since January 1, 1601 (UTC)" per
[MS-ADA3 §2.175 (pwdLastSet)](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-ada3/9663282b-c880-4061-ba8e-e8509c8aa336).
The *same* `Interval` syntax (`2.5.5.16`) also encodes **durations** (e.g. `maxPwdAge`)
as negative values — so read the attribute's semantics before assuming a value is an
absolute time. Other attributes (`whenCreated`, `whenChanged`) use **GeneralizedTime**
strings (`YYYYMMDDHHMMSS.0Z`, UTC).

!!! warning "lastLogonTimestamp lags reality"
    Per [MS-ADA1 §2.352](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-ada1/530d7194-20f6-4aaa-8d80-9ca6b6350ad6),
    `lastLogonTimestamp` is replicated lazily — its update window is 14 days minus a
    random fraction of 5 days. It can lag the real last logon by up to ~14 days; it is
    **not** an accurate "last logon" time. The per-DC `lastLogon` is precise but not
    replicated. Sentinels: `pwdLastSet = 0` means "change at next logon";
    `accountExpires` of 0 or `0x7FFFFFFFFFFFFFFF` means "never".

## FAT / DOS {#fat-dos}

A 32-bit packed value: the high 16 bits are the date word, the low 16 bits the time
word, in **LOCAL time** with no offset, at **2-second** resolution. Epoch **1980**.

| Word | Bits 15–9 | Bits 8–5 | Bits 4–0 |
|---|---|---|---|
| Date (high 16) | year − 1980 (0–127 → 1980–2107) | month (1–12) | day (1–31) |
| Time (low 16) | hour (bits 15–11) | minute (bits 10–5) | second ÷ 2 (bits 4–0) |

Layout: [DosDateTimeToFileTime](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-dosdatetimetofiletime),
[Microsoft FAT32 specification](https://download.microsoft.com/download/1/6/1/161ba512-40e2-4cc9-843a-923143f3456c/fatgen103.doc);
local-time storage is confirmed by the [File Times](https://learn.microsoft.com/en-us/windows/win32/sysinfo/file-times) doc.

- **Worked example:** `2162688` (`0x00210000`) = 1980-01-01 (the FAT epoch / minimum);
  `1391422645` (`0x52EF6CB5`) = 2021-07-15 13:37:42.
- **Gotcha:** write times are quantized to **even** seconds — an odd-second FAT modify
  time is impossible. FAT *creation* time carries a separate 10-ms field; last-access is
  date-only. The local-time storage means timeline work needs the machine's then-current
  timezone/DST. See also [RTC hardware](../concepts/rtc-hardware.md) — 1980-01-01 is the
  classic reset-clock artifact.

## exFAT {#exfat}

exFAT uses the **same packed timestamp layout as FAT** (epoch 1980, 2-second base, plus
a 10-ms increment field), but **adds a `UtcOffset` field** (`OffsetFromUtc`, 15-minute
increments, with an `OffsetValid` bit) that FAT entirely lacks. When `OffsetValid = 1`
the timestamp can be normalised to UTC; otherwise it is local time like FAT. Spec:
[exFAT specification §7.4.5–7.4.10](https://learn.microsoft.com/en-us/windows/win32/fileio/exfat-specification).

## NTFS — $STANDARD_INFORMATION vs $FILE_NAME {#ntfs}

NTFS stores file times **in UTC** on disk as FILETIME values
([File Times](https://learn.microsoft.com/en-us/windows/win32/sysinfo/file-times)). Each
MFT record holds **two** independent MACB quartets (Modified, Accessed, Created,
MFT-changed):

- **`$STANDARD_INFORMATION` (SI)** — the times Explorer shows; **writable from user
  space** via `SetFileTime`.
- **`$FILE_NAME` (FN)** — set by the kernel on create/rename/move; not updatable through
  normal Win32 file-time APIs.

!!! note "Timestomping indicator"
    Anti-forensic tools backdate **SI** times trivially, but **FN** times usually retain
    the true creation/move time. **SI earlier than FN**, or SI sub-seconds of exactly
    `.0000000` while FN is precise, is a classic timestomping tell. (The on-disk MFT
    attribute byte layout has no Microsoft open specification; the FILETIME unit/epoch
    and UTC storage are from MS-DTYP and the File Times doc, while the SI/FN forensic
    distinction is established forensic practice.)

## ZIP / MS-DOS time {#zip}

A ZIP entry's mandatory timestamp uses the **exact FAT/DOS packed format** (1980 epoch,
2-second, local time) — its only required time field. The
[.ZIP APPNOTE v6.3.10](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT)
defines optional extra fields with better time: the **Info-ZIP extended timestamp**
(`0x5455`) carries 32-bit Unix `time_t` values in UTC (1-second), and the **NTFS extra
field** (`0x000A`) carries FILETIME values (100-ns, UTC). A single entry may carry all
three at differing precision and zone.

- **Gotcha:** the mandatory DOS time is local with no zone — compare against the
  `0x5455` / `0x000A` extra fields when present; the local and central-directory copies
  can disagree; and the Unix extra field faces the [Year 2038 problem](../concepts/rollovers.md#y2038).

## See also

- [References](../references.md) — all specifications linked above.
- [Precision](../concepts/precision.md), [RTC hardware](../concepts/rtc-hardware.md),
  [Rollovers](../concepts/rollovers.md).
