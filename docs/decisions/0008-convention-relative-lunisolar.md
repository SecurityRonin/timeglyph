# 0008 — Lunisolar conversion is convention-relative — `--tz` required

Status: Accepted

## Context

Converting an instant to a Chinese lunisolar date and 干支 four pillars depends on a
reference meridian: China (UTC+8), Vietnam (UTC+7), and Korea (UTC+9) disagree on
the civil date and the pillars for the same instant. A UTC instant has no single
Chinese date on its own.

## Decision

The lunisolar feature (`src/lunisolar.rs`, `lunisolar` feature) treats the
conversion as **convention-relative**:

- **`--tz` is required** — it fixes the reference meridian.
- `--longitude` optionally corrects the hour pillar to local mean solar time
  (真太陽時; the equation of time is not applied).
- Divergences (e.g. 立春 vs 正月初一 as the year boundary) are surfaced as
  assumptions, never hidden.

Two engines are reused rather than reinvented (see
[0004](0004-reuse-calendar-and-leap-libraries.md)):

- **`stem-branch`** (Apache-2.0) — solar ephemeris → Sun's apparent ecliptic
  longitude → the year pillar (立春 = 315°) and the 12 month 節 (every 30°), both
  meridian-independent.
- **`lunar-lite`** (MIT) — the lunar (moon) calendar date its solar-only core
  can't supply. Day pillar = Julian-day arithmetic; hour pillar = 五鼠遁.

## Consequences

- Validated against the independent `cnlunar` oracle.
- ΔT uses the Espenak–Meeus modern segments (1986–2050). If `stem-branch` later
  ports its lunar ephemeris, `lunar-lite` could be dropped.
