---
title: "String timestamp forms — ISO 8601, RFC 3339, ASN.1"
description: >-
  Decoding textual timestamps: ISO 8601 / RFC 3339 (with offsets), and ASN.1
  UTCTime / GeneralizedTime as found in X.509 certificates and PKI, including the
  RFC 5280 two-digit-year pivot.
---

# String timestamp forms

Not every timestamp is a number. Many artifacts store **text**: logs, configs,
certificates, and protocol headers. timeglyph parses the common forms via
`timeglyph --as string <text>` (or `interpret_string` in the library).

## ISO 8601 / RFC 3339

The dominant interchange format: `2020-01-01T00:00:00Z`, or with an offset
`2020-01-01T00:00:00+01:00` (normalised to `2019-12-31T23:00:00Z`). Parsing is
delegated to [`jiff`](https://docs.rs/jiff), which handles the offset and
sub-second fields. A string carrying its own offset is **self-describing** — one
of the few unambiguous inputs — so these readings score highest.

## ASN.1 UTCTime (RFC 5280)

X.509 certificates and many PKI/binary structures use ASN.1 time strings. **UTCTime**
has a **two-digit year**: `YYMMDDHHMMSSZ` (e.g. `200101000000Z`).

!!! warning "The two-digit-year pivot"
    [RFC 5280 §4.1.2.5.1](https://www.rfc-editor.org/rfc/rfc5280) fixes the pivot:
    `YY < 50` → `20YY`, otherwise `19YY`. So `20…` is **2020**, `95…` is **1995**.
    A naïve "always 19YY" or "always 20YY" reading is wrong on one side of 2050.

## ASN.1 GeneralizedTime (RFC 5280)

The modern form, with a **four-digit year**: `YYYYMMDDHHMMSSZ` (e.g.
`20200101000000Z`), optionally with a fraction and a `Z`/`±HHMM` offset. Used for
certificate validity dates from 2050 onward (where UTCTime's 2-digit year is
ambiguous).

## Timezone handling

A trailing `Z` (or `±HHMM`) is honoured; a string with **no** designator is read as
UTC, and that assumption is stated. As with all readings, the offset interpretation
is surfaced rather than hidden.

## Examples

```console
$ timeglyph --as string 2020-01-01T00:00:00Z
$ timeglyph --as string 20200101000000Z      # ASN.1 GeneralizedTime
$ timeglyph --as string 950101000000Z        # ASN.1 UTCTime → 1995-01-01
```

## See also

- [Calendars](../concepts/calendars.md) (proleptic Gregorian, the year-0
  convention) · [References](../references.md) · [Input conventions](../concepts/input-conventions.md)
