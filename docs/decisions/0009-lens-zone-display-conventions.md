# 0009 — timeglyph-lens zone & time display conventions

Status: Accepted

## Context

The `timeglyph-lens` overlay renders a decoded instant in a chosen display zone.
Two recurring hazards shape how the zone frame is presented:

- **Misreading the frame** — the classic forensic error of reading a local-time
  value as if it were UTC. The display must make the active frame unmistakable.
- **IANA quirks that confuse** — the POSIX `Etc/GMT±N` ids invert the sign
  (`Etc/GMT-8` is UTC+08:00); `Etc/*` is a bag of duplicate offset aliases, not a
  geographic region; many zones report a numeric pseudo-abbreviation (`-05`) that
  merely repeats the offset; and `Local` does not say which zone it resolved to.

## Decision

Footer zone chip (`zone::zone_summary`):

- **Always highlighted amber**, including UTC, so the active frame is unmistakable
  at a glance — replacing the earlier calm-vs-loud styling and the `⚠` caution
  sign, which the amber highlight plus the explicit offset make redundant.
- Form `Label (ABBR = UTC±HH:MM)`, e.g. `Asia/Shanghai (CST = UTC+08:00)`; when a
  zone has no letter code, `Name (UTC+05:30)`; `UTC` and fixed / `Etc` offset
  labels stand alone.
- **`Local` surfaces the resolved system zone**:
  `Local (Asia/Shanghai (HKT) = UTC+08:00)`.

Zone identifiers:

- Numeric pseudo-abbreviations (`-05`) are suppressed (`tzinfo::stamp`) — they only
  repeat the offset.
- `Etc/GMT±N` ids are relabeled to their true, sign-corrected offset
  (`zone::clean_label`: `Etc/GMT-8` → `UTC+08:00`); all `Etc/*` UTC aliases → `UTC`.
- The `Etc` region stays in the picker but is shown as **`etc.`**
  (`zone::continent_label`), and every region's submenu is deduplicated and
  offset-sorted (`zone::menu_entries`), so `Etc` reads `UTC-12:00 → UTC+14:00` with
  a single `UTC` entry instead of the lexical `Etc/GMT+1, +10, +11…` mess.
  `SystemV` (aliases `clean_label` does not tidy) is excluded.

Readings (`overlay::datetime_cell`):

- A UTC-anchored reading shows a trailing **`UTC`** designator (even though its
  value already ends in `Z`), so every zone — UTC included — occupies the same
  zone-designator column that a named zone's abbreviation does.
- DST is marked with a ☀ sun glyph.

All chrome glyphs are monochrome (egui's GL backend cannot rasterize colour emoji)
and are guarded by `lens/tests/fonts.rs`.

## Consequences

- The frame is legible without a warning symbol; UTC is treated like any other
  zone for consistency, at the cost of a deliberate `Z` / `UTC` duplication.
- Pure offsets stay reachable (the map, and `Etc`) but never surface a
  sign-inverted or duplicate id.
- These are presentation conventions in `lens/src/zone.rs` and
  `lens/src/overlay.rs`; the engine's `PosixNs` spine and rendering are unchanged.
