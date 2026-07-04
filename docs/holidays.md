---
title: "Public holidays — is this date a holiday?"
description: >-
  timeglyph's holiday feature annotates a date with the public holiday (if any)
  for a country — 248 countries, 1980–2100, from python-holidays. Framed as
  consistent-with, in each country's own locale.
---

# Public holidays

Behind the optional **`holiday`** feature, timeglyph can answer: *is this date a
public holiday, and where?* The [timeglyph-spy](spy.md) overlay uses it to
annotate each reading with the holiday for that date in the chosen display zone.

```rust
use timeglyph::holiday;
use jiff::civil::date;

holiday::lookup("US", date(2020, 7, 4));   // Some("Independence Day")
holiday::lookup("CN", date(2020, 1, 25));  // Some("春节")  (Chinese New Year)
holiday::lookup("US", date(2020, 3, 16));  // None          (an ordinary day)
```

## Coverage

- **248 countries** (ISO-3166 alpha-2), **1980–2100**, ~379,000 holiday-dates.
- Generated from the MIT-licensed
  [python-holidays](https://github.com/vacanza/holidays) project and embedded as
  a ~1.5 MB gzipped table (parsed once, lazily). See `data/README.md` for the
  generator and exact version.
- **Coverage varies by country** — python-holidays supports a different year
  range per locale and clips the request, so some countries carry fewer years.

## Epistemics — an annotation, not a verdict

A hit means the date **is consistent with a public holiday** in that country per
the reference data. It is an annotation, not proof the day was observed at a
given place, and a `None` means "no holiday in the covered data", not "provably
an ordinary day". Lunar / astronomically-derived holidays are the reference
project's best estimates.

## Names are in each country's own locale

Holiday names come through exactly as python-holidays emits them, in the
country's default language — HK `一月一日`, CN `元旦`, JP `元日`, TW
`中華民國開國紀念日`, DE `Neujahr`, US/GB `New Year's Day`. So the same calendar
day (e.g. 1 January) carries a different, locally authentic name per country —
including bare-date statutory names like Hong Kong's `一月一日` ("the first day of
January").

## From a timezone to a country

The overlay annotates by the **display zone**, mapping the IANA zone to a country
(`Asia/Shanghai` → `CN`) via the tz database `zone.tab` plus the `backward` Link
file — so legacy aliases resolve too (`Asia/Chongqing` → `Asia/Shanghai` → `CN`,
`US/Eastern` → `America/New_York` → `US`). A zone with no single country (`UTC`, a
fixed offset) yields no annotation.

## Enable it

```bash
cargo add timeglyph --features holiday       # library
cargo install timeglyph --features holiday   # binary (the timeglyph-spy overlay bundles it)
```
