---
title: "Validation — tier-1 differential battery & provenance"
description: >-
  How every timeglyph format is validated: the evidence tiers, the independent
  third-party oracles (time-decode, CPython, the format specs), the per-format
  differential battery with agreed answers, and the input-convention caveats.
---

# Validation

A forensic tool's output is only worth the rigour behind it. This page documents
exactly how each timeglyph format is checked, against whom, and at what confidence
tier — so the standing rests on measurements an independent party can re-run, not
on self-assessment.

## Evidence tiers

timeglyph classifies every validation by **who confirms it** (the fleet standard):

| Tier | Definition | Example here |
|---|---|---|
| **Tier 1** | An independent third party authored **both** the input value **and** the answer, or it is real-world data. | Discord's docs give id `175928847299117063` → `2016-04-30 11:18:25.796`; Apple TN1150 states HFS+ max = `2040-02-06 06:28:15`. |
| **Tier 2** | Real output checked against an independent oracle, but **we** chose the scenario. | A value we picked, confirmed by an independent implementation (e.g. CPython). |
| **Tier 3** | We authored both the fixture and the expected answer, nothing independent vouches. | (Avoided — these are the self-deception trap.) |

The goal is tier 1 wherever reachable; tier-2 rows are labelled honestly.

## Independent oracles

| Oracle | Author / license | Role | Verified |
|---|---|---|---|
| **time-decode** v10.4.0 | Corey Forman (digitalsleuth), **MIT** — [github](https://github.com/digitalsleuth/time_decode) | A separate implementation of 73 timestamp formats; the primary differential oracle (the reference named in ADR 0007). | run in this battery |
| **CPython `datetime`** | Python Software Foundation, PSF-2.0 — [docs](https://docs.python.org/3/library/datetime.html) | Independent implementation for formats time-decode lacks a flag for (PostgreSQL, Unix-ns). | run |
| **unfurl** (`dfir-unfurl`) | Ryan Benson, **Apache-2.0** — [github](https://github.com/obsidianforensics/unfurl) | A *second* independent oracle for the embedded-ID family (Twitter/Discord snowflakes); raises those from single- to dual-oracle. Env-gated (`tests/unfurl_oracle.rs`). | run when installed |
| **Format specifications** | the spec authors (Discord, Microsoft, Apple, RFCs, …) | Spec worked examples: the author states value → answer directly (tier-1 gold). | see [References](references.md) |

Differential validation between two independent implementations is strong but not
infallible — a shared misreading of a spec could make both agree on a wrong
answer. The tier-1 **spec worked examples** below anchor the trickiest epochs
independently of any tool, which is why both kinds of check are used together.

## Format coverage

timeglyph implements **44 numeric/packed formats** (`src/registry.rs`), plus the
self-describing string forms in `interpret.rs` (ISO-8601/RFC-3339, RFC-2822,
HTTP-date, EXIF, ASN.1, ULID, UUIDv1, ObjectId, Google `ei=`, Apache CLF, PDF date, DMTF/WMI CIM). `tests/docs_sync.rs` fails the
build if any registry format drifts out of this list, so coverage cannot fall
silently behind:

- **Linear epochs** (seconds → nanoseconds since a fixed point): `unix`,
  `unix_ms`, `unix_us`, `unix_ns`, `unix_float`, `filetime`, `webkit`, `cocoa`, `cocoa_float`,
  `hfsplus`, `hfs`, `dotnet_ticks`, `active`, `prtime`, `iostime`, `postgres`,
  `dhcp6`, `ole`, `excel1904`, `mjd`, `sqlite_julian`, `ksuid`, `nokiale`.
- **Embedded-ID** (epoch + bit-shift within a larger ID): `snowflake`,
  `discord`, `mastodon`, `linkedin`, `tiktok`, `sony`.
- **Packed bit-field / civil**: `fat`, `exfat`, `dttm`, `bitdate`, `bitdec`,
  `bcd`, `moto`, `symantec`, `dvr`, `ns40`, `ns40le`, `logtime`, `semioctet`,
  `gsm`, `sqlserver`.

The differential battery below validates a subset against `time-decode`;
formats without a matching oracle flag are tier-2 (spec worked example) — see the
note after the table.

## The differential battery

Every input below is **time-decode's own published example value** for that
format (`time-decode --formats <fmt>`), so the input is authored by the
independent third party — not chosen by us — and the expected answer is its
tool's output. timeglyph agrees with all of them to the second. The battery is
encoded as an env-gated test (`tests/oracle.rs`).

| timeglyph format | oracle flag | third-party input | agreed answer (UTC) | tier |
|---|---|---|---|---|
| `unix` | `--unixsec` | `1746371930` | 2025-05-04 15:18:50 | 1 |
| `unix_ms` | `--unixmilli` | `1746371930064` | 2025-05-04 15:18:50.064 | 1 |
| `unix_us` | `--prtime` | `1746371930064939` | 2025-05-04 15:18:50.064939 | 1 |
| `filetime` | `--active` | `133908455300649390` | 2025-05-04 15:18:50.064939 | 1 |
| `webkit` | `--chrome` | `13390845530064940` | 2025-05-04 15:18:50.064940 | 1 |
| `hfsplus` | `--hfsdec` | `3829216730` | 2025-05-04 15:18:50 | 1 |
| `hfsplus` (max) | `--hfsdec` | `4294967295` | 2040-02-06 06:28:15 | 1 (spec + oracle) |
| `dotnet_ticks` | `--dotnet` | `638819687300649472` | 2025-05-04 15:18:50.064947 | 1 |
| `cocoa_float` | `--mac` | `768064730.064939` | 2025-05-04 15:18:50.064939 | 1 |
| `sqlite_julian` | `--juliandec` | `2460800.1380787035` | 2025-05-04 15:18:49.999986 | 1 |
| `ole` | `--oleauto` | `45781.638079455312` | 2025-05-04 15:18:50.064939 | 1 |
| `discord` | `--discord` | `1102608904745127937` | 2023-05-01 14:54:08.374 | 1 |
| `snowflake` | `--twitter` | `1189581422684274688` | 2019-10-30 16:34:47.179 | 1 |
| `fat` | `--fat` | `a45a597a` | 2025-05-04 15:18:50 (local) | 1 |
| `gps` | `--gps` | `1430407111` | 2025-05-04 15:18:13 | 1 |
| `ntp` | `--ntp` | `3981841662.020607` | 2026-03-07 03:07:42 | 1 |
| `tai64` | `--tai` | `1599755800` | 2020-09-10 16:36:03 | 1 |

!!! note "Formats time-decode has no flag for"
    `postgres` (µs since 2000) and `unix_ns` are validated against **CPython
    `datetime`** instead, with values we chose — **tier 2**:
    `postgres 631152000000000` → `2020-01-01 00:00:00` and
    `unix_ns 1577836800000000000` → `2020-01-01 00:00:00`. To reach tier 1, source
    a third-party-published worked example for each.

## Tier-1 spec worked examples

These come straight from the format's own specification — the author states the
value and the answer, so no tool is involved:

- **Discord** — the developer docs decode `175928847299117063` → `2016-04-30 11:18:25.796 UTC` ([Discord docs](https://discord.com/developers/docs/reference#snowflakes)).
- **OLE Automation** — Microsoft states `2.0` = 1900-01-01 and `2.5` = noon 1900-01-01 ([VariantTimeToSystemTime](https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-varianttimetosystemtime)).
- **HFS+** — Apple TN1150 states the maximum date is `2040-02-06 06:28:15 GMT` (the u32 overflow), matched by `hfsplus 4294967295` ([TN1150](https://developer.apple.com/library/archive/technotes/tn/tn1150.html)).

These are encoded in `tests/anchors.rs` and run on every build (no oracle needed).

## Input conventions (reconciled divergences)

The same instant can be presented to a decoder in different encodings. Where
timeglyph and the oracle take different encodings, the battery converts between
them — these are **convention** differences, not disagreements about the time:

- **FAT** — time-decode reads the 4 on-disk bytes `a4 5a 59 7a` as
  `date = LE(a4,5a) = 0x5AA4`, `time = LE(59,7a) = 0x7A59`. timeglyph takes those
  two 16-bit words packed into one integer (date in the high word) =
  `0x5AA47A59`. Both decode to the same instant.
- **TAI64** — the external label is `2^62 + s` (s = TAI seconds since 1970 TAI),
  while time-decode's `--tai` takes `s` directly; the battery passes `label − 2^62`.
- **NTP** — time-decode's example carries a fractional part (`…​.020607`); timeglyph
  decodes the integer seconds field. They agree to the second.

The broader question of robustly handling endianness, hex-vs-decimal, and packed
layouts is a design topic tracked separately.

## How to run

```bash
# 1. Install the independent oracle (MIT).
pip install time-decode

# 2. Run the env-gated differential battery (skips cleanly if absent).
cargo test --features leap --test oracle

# 3. The tier-1 spec anchors run with the normal suite.
cargo test --all-features
```

## Appendix — epoch offsets (seconds to the Unix epoch)

A cross-check reference; positive = after 1970, negative = before.

| Epoch | Used by | Offset to 1970 |
|---|---|---|
| 0001-01-01 | .NET ticks | −62 135 596 800 s |
| 1582-10-15 | UUIDv1/v6 | −12 219 292 800 s (×10⁷ as 100-ns) |
| 1601-01-01 | FILETIME, WebKit | −11 644 473 600 s |
| 1899-12-30 | OLE Automation | −2 209 161 600 s |
| 1900-01-01 | NTP | −2 208 988 800 s |
| 1904-01-01 | HFS+ | −2 082 844 800 s |
| 1970-01-01 | Unix | 0 |
| 1980-01-01 | FAT/DOS | +315 532 800 s |
| 1980-01-06 | GPS | +315 964 800 s |
| 2000-01-01 | PostgreSQL | +946 684 800 s |
| 2001-01-01 | Cocoa / CFAbsoluteTime | +978 307 200 s |

Julian Day `2440587.5` = the Unix epoch. Leap offsets (2017→): TAI − UTC = 37 s,
GPS − UTC = 18 s. See [Time scales](concepts/time-scales.md) and
[Calendars](concepts/calendars.md).
