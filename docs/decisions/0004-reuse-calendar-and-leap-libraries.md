# 0004 — Reuse `jiff`/`hifitime`; write zero calendar math

Status: Accepted

## Context

Civil-time and leap/TAI/GPS math is a solved, error-prone domain. Hand-rolling
calendar arithmetic (timezones, historical IANA rules, leap seconds) is a classic
source of subtle, hard-to-audit bugs.

## Decision

Reuse mature crates and write **zero** calendar code:

- **`jiff`** — civil time + IANA timezones (including historical) + ISO 8601 /
  RFC 3339 / RFC 2822 / HTTP-date parse and format.
- **`hifitime`** — leap seconds, TAI, GPS (see
  [0003](0003-partition-leap-aware-time-scales.md)).

## Consequences

- Correctness for timezone and leap math rides on audited, maintained libraries.
- The crate owns only the *format registry* and *interpretation*, not time math.
- Lunisolar work reuses the same instinct — see
  [0008](0008-convention-relative-lunisolar.md) (`stem-branch` + `lunar-lite`).
