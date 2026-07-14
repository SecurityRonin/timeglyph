# The forensic calendar (`cal`)

`timeglyph cal` is a calendar for digital-forensics work. Where the Unix `cal`
prints day numbers, `cal` surfaces the temporal detail an examiner reasons
about — timezone offsets and DST fold/gap days, leap-second days, ISO week and
Julian Day numbering, GPS week, timestamp-format epoch boundaries, alternative
calendars, and the moon's phase — for a day, a month, or a whole year.

Every number is computed, not looked up, and carries an independent oracle in the
test suite (`tests/cal.rs`): ISO weeks and day-of-year against `date`, Julian Day
Numbers against USNO, DST transitions against `zdump`, leap seconds against the
IERS table (via hifitime), the moon against JPL/Meeus, and the equinox/solstice
instants against published almanac values.

## Usage

```bash
timeglyph cal                     # the current month, in UTC
timeglyph cal 2026-07             # a specific month
timeglyph cal 2026                # a whole year (with the season timeline)
timeglyph cal 2026-11-01          # one day, in full detail (with a moon disc)
timeglyph cal 2025-02-19T14:30    # a specific instant → the 時柱 for that hour
timeglyph cal 2026-11 --tz America/New_York   # zone-aware: DST folds/gaps flagged
timeglyph cal 2026 --tz Australia/Sydney      # southern-hemisphere seasons (Dec = summer)
timeglyph cal 2026-07 --json      # faithful, one record per day, for pipes
```

Flags: `--tz <zone>` (global; UTC by default), `--week-start monday|sunday`,
`--json`.

## The month grid

```
July 2026                UTC

      Mo  Tu  We  Th  Fr  Sa  Su
W27           1   2   3   4   5
W28   6   7   8   9  10  11  12
W29  13* 14  15  16  17  18  19
W30  20  21  22  23  24  25  26
W31  27  28  29  30  31

  * today   ^ DST gap   v DST fold   + leap second   e epoch   ~ rollover
```

The gutter is the ISO-8601 week number. One marker per day flags the
forensically-salient conditions (highest precedence wins): today, a
spring-forward **gap** day, a fall-back **fold** day, a **leap-second** day, a
timestamp-format **epoch** day, or a fixed-width **rollover**. Markers are plain
ASCII, so a piped or colourless view loses no information.

## The day detail card

A single date prints every field: the week/epoch systems, timezone, leap/GPS, the
moon as a shaded disc, the Chinese / Hebrew / Islamic dates, and the season with
its scene tile.

```
2025-01-15  wednesday
  iso 2025-W03-3   doy 15/365   jdn 2460691   mjd 60690
  unix midnight 1736899200   offset 0s .. 0s   wall day 86400s
  leap 0 (UTC day 86400s)   gps week 2349

      @ @ @ @ @      Full Moon
    @ @ @ @ @ @ @    97% illuminated
   @ @ @ @ @ @ @ @   elongation 199.1deg
   @ @ @ @ @ @ @ @
   @ @ @ @ @ @ @ @
    @ @ @ @ @ @ @
      @ @ @ @ @

  chinese   lunar 十二月十六日 · 小寒
            甲丁甲庚
            辰丑申午
            年月日時
  hebrew    15 Tevet 5785
  islamic   15 Rajab 1446
  season    winter (N. hemisphere; solar longitude 295.6deg)
       _===_
      (.o.o.)
      ( >^< )
     (( : : ))
    *  *  *  *  *
```

The month view closes with an info panel (Chinese year + solar term, season, the
mid-month moon, and the Hebrew / Islamic spans), and the year view opens with the
season timeline of astronomically-exact equinox/solstice dates.

## Definitions and conventions

- **Dates are proleptic Gregorian** everywhere, including the Julian Day Number.
  This matches how every timestamp format defines its epoch (FILETIME's 1601,
  MJD's 1858, SQL Server's 1900), so `cal`'s JDN and a format epoch never disagree
  by the Julian/Gregorian cutover. `cal 1582-10` therefore shows a full October;
  the historically-skipped Oct 5–14 exist proleptically.
- **Wall-day length** is the elapsed real seconds between local midnights: 86 400
  on an ordinary day, 82 800 (23 h) across a spring-forward gap, 90 000 (25 h)
  across a fall-back fold, and other values for sub-hour transitions (e.g. Lord
  Howe's 30-minute change → 88 200 s).
- **Leap-second days** are derived from the change in the cumulative TAI−UTC
  offset across the UTC day (hifitime's IERS table), never a hardcoded date. Such
  a day's UTC length is `86400 + leap`.
- **GPS week** is the continuous week (not the 10-bit rollover value) containing
  the day's 00:00 UTC instant. GPST leads UTC by the current leap count, so a
  Sunday-00:00Z instant already falls in the new week.
- **Season boundaries** are the astronomically exact instants the Sun's apparent
  longitude reaches 0/90/180/270° (the 春分/夏至/秋分/冬至 solar terms), from the
  stem-branch ephemeris — not fixed calendar dates. Which season a boundary opens
  depends on hemisphere: the December solstice opens winter in the north, summer
  in the south. **The hemisphere is derived from the `--tz` zone** — a named zone
  south of the equator (`Australia/Sydney`, `Pacific/Auckland`, `America/Santiago`)
  yields austral seasons automatically, from the tzdb `zone1970.tab` latitudes. UTC
  and fixed offsets carry no latitude, so they default to the northern hemisphere.
- **Moon phase** uses the true geocentric elongation (Meeus ch. 48, including the
  Moon's ecliptic latitude and the real Sun/Moon distances), so the illuminated
  fraction is accurate near the quarters. It is an almanac-grade value, useful for
  corroborating night-time illumination in photo/CCTV/witness evidence.

## Alternative calendars

With the default build, each day also carries the Chinese lunisolar date (in
Chinese, e.g. `正月廿二日`) and the four 干支 pillars — year, month, day, and hour
(年月日時) — via the stem-branch ephemeris at the render-zone meridian. The day
card stacks them as stems over branches over labels. For a bare date the **hour
pillar (時柱) is computed at the day's noon** (午時); pass a time —
`cal 2025-02-19T14:30` — to get the 時柱 for that actual hour (the year/month/day
pillars are unchanged). The solar term is shown as a period phrase — the bare term
on its own day, else `<term>後第<N>日` (e.g. `雨水後第七日`) — so a day well past the
term is not misread as its exact day. Behind the `altcal` feature, the Hebrew and
Islamic (tabular civil) dates come from ICU4X. All values appear in `--json`.
