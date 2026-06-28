---
title: "RTC hardware & older clocks — MC146818, the 18.2 Hz tick, the 1980 epoch"
description: >-
  How older computers kept time: the MC146818 CMOS real-time clock and its 2-digit
  year, the 18.2 Hz PC timer tick, the MS-DOS 1980 epoch, CMOS-battery clock resets,
  drift without NTP, and the Linux hardware-vs-system clock split.
---

# RTC hardware and the limits of older clocks

Timestamps are only as good as the clock that produced them. On older machines that
clock was a small battery-backed chip with a 2-digit year, a coarse periodic tick, and
no network discipline. Understanding it explains a whole class of forensic artifacts —
implausible floor dates, quantized sub-second times, and timezone-blind local stamps.

## The CMOS real-time clock — Motorola MC146818

The IBM PC/AT (1984) introduced a battery-backed real-time clock plus low-power CMOS
RAM on one chip, the **Motorola MC146818A** (and compatibles), accessed via I/O ports
`0x70` (index) / `0x71` (data). Its registers hold the wall clock to **1-second**
resolution:

| Offset | Field | Range |
|---|---|---|
| `0x00` | Seconds | 0–59 |
| `0x02` | Minutes | 0–59 |
| `0x04` | Hours | 0–23 (or 1–12 + PM bit) |
| `0x07` | Day of month | 1–31 |
| `0x08` | Month | 1–12 |
| `0x09` | Year | **0–99 (two digits)** |
| `0x32` | Century (if present) | non-standard |
| `0x0A` | Status A | bit 7 = update-in-progress |
| `0x0B` | Status B | bit 1 = 24h, bit 2 = binary (else BCD) |

Values are BCD unless Status B bit 2 is set; the oscillator is the canonical 32.768 kHz
crystal, updating once per second. Source: [OSDev CMOS](https://web.archive.org/web/20241222182649/https://wiki.osdev.org/CMOS)
and [OSDev RTC](https://web.archive.org/web/20241204080709/https://wiki.osdev.org/RTC)
(the live wiki blocks automated fetches; these are verified archive snapshots).

!!! warning "The 2-digit year (a structural Y2K problem)"
    The on-chip year is **two digits**, and originally there was **no century
    register**. Manufacturers later added one (commonly at `0x32`) but, in the words of
    the OSDev reference, "there was no official standard … different manufacturers used
    different registers"; ACPI's FADT later added a pointer naming which register holds
    the century (0 = none). So a CMOS-sourced date's century is, on pre-ACPI hardware, a
    BIOS/OS **guess** (typically "year ≥ 90 → 19YY, else 20YY"). See [Y2K](rollovers.md#y2k).

There is also an **update-in-progress** hazard: reading the registers mid-update can
return inconsistent values (e.g. `8:60`), so software must poll Status A bit 7 first.
The weekday register is, per the same reference, "entirely unreliable."

## The 18.2 Hz PC timer tick

MS-DOS did not read the RTC for time-of-day; it counted ticks from the **8253/8254
Programmable Interval Timer (PIT)**. The PIT runs at ~**1.193182 MHz** (the 14.31818 MHz
master oscillator ÷ 12). With the BIOS default reload value of 65 536, channel 0 fires
IRQ0 at:

```text
1193182 / 65536 = 18.2065 Hz   →   54.9254 ms per tick
```

Source: [OSDev PIT](https://web.archive.org/web/20241229071829/https://wiki.osdev.org/Programmable_Interval_Timer).

!!! note "DOS sub-second times are quantized"
    MS-DOS wall-clock time read from the BIOS tick counter is quantized to ~54.925 ms —
    a DOS-era fractional second can take only ~18 discrete values. Any finer "precision"
    in a DOS timestamp is not real. (This is why FAT settled on a coarse
    [2-second write granularity](precision.md#filesystem-unit-vs-recorded-resolution).)

## The MS-DOS 1980 epoch

FAT/DOS dates encode the year as a 7-bit "offset from **1980**" (0–127 → 1980–2107),
so the **minimum representable FAT date is 1980-01-01** and any earlier date is
structurally impossible. Source: [DosDateTimeToFileTime](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-dosdatetimetofiletime),
[exFAT specification](https://learn.microsoft.com/en-us/windows/win32/fileio/exfat-specification).
See the [FAT/DOS format reference](../formats/windows.md#fat-dos).

## Clock resets, drift, and the absence of NTP

A dead CMOS battery loses the clock; on next boot the firmware re-seeds from an invalid
or floor value. Common "reset" sinks seen in evidence:

- **1980-01-01** — the FAT/DOS floor (most common on DOS/FAT systems).
- **1970-01-01** — the Unix epoch floor.
- a hardcoded **BIOS default / manufacture date** (often the BIOS build year).

!!! danger "A cluster of floor dates means a reset clock, not activity"
    A group of files all stamped at a single implausible floor date — especially with
    identical times — is a strong indicator of a **dead CMOS battery or cleared NVRAM**,
    not of genuine user activity. Treat such timestamps as "clock-was-reset" markers.

Older machines also had **no NTP**: the RTC ran free, drifting fast or slow, corrected
only by manual setting. Pre-NTP network protocols were crude — the
[TIME protocol (RFC 868)](https://www.rfc-editor.org/rfc/rfc868) returns a 32-bit count
of seconds since 1900 and rolls over in 2036. And **DOS/FAT store local wall-clock time
with no timezone or DST tag**, so those timestamps cannot be converted to UTC without
independent knowledge of the machine's then-current locale. Dual-booting two OSes that
both DST-adjust the RTC can corrupt it further. Source:
[OSDev Time And Date](https://web.archive.org/web/20241227171812/https://wiki.osdev.org/Time_And_Date).

## Modern Unix: hardware clock vs. system clock

Linux separates the battery-backed **hardware clock** (RTC) from the kernel's
**system clock**. [`hwclock(8)`](https://man7.org/linux/man-pages/man8/hwclock.8.html)
reads or sets the RTC (`--hctosys`, `--systohc`), applies a stored drift factor from
`/etc/adjtime` (`--adjust`), and notes the kernel's **"11-minute mode"**: when NTP
discipline is active the kernel silently copies system time back into the RTC roughly
every 11 minutes.

!!! tip "Forensic leverage"
    On an NTP-disciplined Linux box the RTC reflects *recently disciplined* time, not the
    last manual set. Meanwhile `/etc/adjtime` records the historical drift factor and the
    last-set timestamp — which can corroborate or contradict a claim about how the clock
    behaved.

## See also

- [Precision](precision.md) — sub-second resolution in general.
- [Epoch rollovers](rollovers.md) — Y2K, the 2036 (NTP/TIME) rollover, and Year 2106.
- [Formats: Windows — FAT/DOS](../formats/windows.md#fat-dos).
