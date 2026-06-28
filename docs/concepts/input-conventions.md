---
title: "Input conventions — encoding, endianness, packing"
description: >-
  The same instant can be presented to a decoder in many input encodings —
  decimal, hex (LE/BE), packed words, strings — and the wrong choice yields a
  plausible-but-wrong date silently. How timeglyph separates format from encoding
  and refuses to look authoritative when the encoding is ambiguous.
---

# Input conventions

The most dangerous failure for a timestamp tool is **silently wrong output**: a
value decoded under the wrong *input encoding* produces a valid-looking date that
is simply incorrect. This chapter maps the encoding ambiguities and how timeglyph
handles them.

## Format is not encoding

A reading needs two independent things:

- **Format (semantics)** — the epoch, unit, calendar, and leap policy (e.g.
  "FILETIME: 100 ns since 1601 UTC").
- **Encoding (presentation)** — how the raw input represents the value(s) the
  format consumes: radix, endianness, width, signedness, and field packing.

The same FILETIME instant can arrive as a decimal QWORD (`132223104000000000`),
as little-endian hex bytes (a registry `.reg` export), or as a big-endian dump.
The format is identical; the encoding differs. Conflating the two is what produces
silent errors.

## The axes of input ambiguity

| Axis | Example trap |
|---|---|
| **Representation** | decimal vs hex vs float vs string for the same value |
| **Endianness** | hex bytes read LE vs BE — different numbers entirely |
| **Width & signedness** | `0xFFFFFFFF` as `-1`, as a sentinel, or as unsigned 2106 |
| **Packing & field order** | which 16-bit word is the date vs time; on-disk byte order vs a packed integer |
| **Sub-second sub-encoding** | OLE's "two ints separated by a dot" vs a real IEEE double |

### The FAT case (a worked example)

FAT/DOS packs a 16-bit date word and a 16-bit time word. timeglyph's `decode fat`
takes a packed integer with the date in the **high** word; on disk the four bytes
are stored as date-word-then-time-word, **each little-endian**. The byte string
`a45a597a` therefore decodes (on disk) as date `0x5AA4`, time `0x7A59` →
`2025-05-04 15:18:50`. Feed those same bytes as a naïve big-endian integer and you
get a *different, wrong, plausible* date. timeglyph's `hex` path decodes the
on-disk packed layout explicitly and labels it, so the analyst sees the right
instant and the assumption behind it.

## Things a naïve design gets wrong

These are real traps (several flagged in an adversarial design review):

- **Width inferred from length.** `00000001` may *intentionally* be a `u32` —
  leading zeros are evidence, not decoration. Never guess width from string length.
- **UUID/GUID textual order ≠ raw byte order**, and UUIDv1 rearranges its
  timestamp fields — decoding the text as raw bytes gives a plausible wrong date.
- **`SYSTEMTIME` is not a timestamp** — it is a struct of calendar fields (LE
  words), and it may be local or UTC depending on the API that wrote it.
- **Composite/split timestamps** — date and time in separate columns; decoding one
  field alone yields plausible nonsense.
- **Hex is a *syntax*, not an *encoding model*** — `--hex`/`hex` says "these are
  bytes", not "this is how the format lays them out". The layout is separate.

## How timeglyph stays safe

- **Ranked candidates, never one verdict** — an ambiguous input surfaces every
  plausible reading, scored, so no single (possibly wrong) answer looks
  authoritative. See [Methodology](methodology.md).
- **Explicit, labelled byte layouts** — the `hex` path tries LE/BE widths *and*
  packed on-disk layouts, each labelled with its assumption.
- **Sentinels are flagged** — magic "unset/never" values do not masquerade as
  instants. See [Sentinel values](sentinel-values.md).
- **Pipeline-safe exit codes** — `identify` exits `2` ("review needed") when the
  top reading is a sentinel or tied for the top score, so a script cannot mistake
  an ambiguous result for a confident one. (`0` = clear winner, `1` = error.)
- **Every reading states its assumptions** — including the timezone caveat for
  local-time formats and the leap-smear caveat for POSIX readings.

## Decimal-packed: allowed, but never authoritative

Analysts often have a decimal pulled from a database column, not raw bytes, so
timeglyph does **not** forbid decimal input for packed formats. But because field
and byte order are not derivable from a bare scalar, such a reading is marked
ambiguous and ranked so it never looks authoritative — prefer the raw `hex` bytes
when you have them.

## See also

- [Sentinel values](sentinel-values.md) · [Precision](precision.md) ·
  [Methodology](methodology.md) · [Format reference](../formats/index.md)
