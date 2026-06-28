---
title: "Sentinel values — unset, never, and error markers"
description: >-
  Timestamp fields use magic values (0, all-ones, 0x7FFFFFFFFFFFFFFF) to mean
  unset / never / error rather than a real instant. Decoding them as a date is a
  classic silent-wrong forensic failure; how timeglyph flags and ranks them.
---

# Sentinel values

Not every timestamp field holds a time. Many use **sentinel** ("magic") values to
mean *unset*, *never*, or *error*. Rendering one as a plausible date is a textbook
silent-wrong-output failure — so timeglyph detects them, flags them
(machine-readably), and ranks them so they never look authoritative.

## Common sentinels

| Value | Common meaning | Where seen |
|---|---|---|
| `0` | unset / uninitialised (decodes to the bare epoch) | almost every format |
| `-1` / all-ones | unset / invalid | many C structs |
| `0x7FFFFFFFFFFFFFFF` | "never" (max signed 64-bit) | Active Directory `accountExpires` |
| `pwdLastSet = 0` | "must change password at next logon" | Active Directory |
| FAT date `0` | no date set | FAT/DOS directory entries |

!!! warning "The trap"
    Sentinels are dangerous precisely because they *decode*. `cocoa(0)` =
    `2001-01-01`, which is even inside the plausibility window — so without sentinel
    handling it would look like a confident reading. It is not; it is an unset field.

## How timeglyph handles them

- **A `sentinel` flag on every candidate** — machine-readable, so a pipeline can
  refuse to treat a sentinel reading as authoritative.
- **Possible vs known** — generic value sentinels (`0`, `-1`) are flagged as
  *possible* (suggestive across any format), while a format-specific magic value
  such as `0x7FFFFFFFFFFFFFFF` ("never") is flagged as a *known* sentinel.
- **All-ones bytes** — a `0xFFFFFFFFFFFFFFFF` hex value exceeds `i64` and yields no
  linear reading, so it is surfaced explicitly as an all-ones sentinel rather than
  vanishing.
- **A `not_sentinel` scoring component** (heavily weighted) pulls sentinel readings
  below any genuine value — `cocoa(0)` no longer outranks a real in-window date.
- **An explanatory assumption** — e.g. "value 0 is a likely sentinel (zero / unset)
  — an 'unset'/'never' marker, not necessarily a real instant".
- **A pipeline-safe exit code** — `identify` on a sentinel value exits `2`
  ("review needed"), never a confident `0`.

Sentinels are **flagged, never hidden**: a forensic tool must still show what the
raw bytes were, so the analyst can judge. (See the
[fail-loud / show-the-value discipline](input-conventions.md).)

!!! note "Not a sentinel: `0xFFFFFFFF`"
    `4294967295` (u32 max) is the *genuine* maximum HFS+ date
    (`2040-02-06 06:28:15`, per Apple TN1150), so it is deliberately **not** treated
    as a sentinel — context matters, and a real boundary value is real evidence.

## See also

- [Input conventions](input-conventions.md) · [Methodology](methodology.md) ·
  [Microsoft formats — Active Directory](../formats/windows.md#ad-ldap)
