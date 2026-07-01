# 0003 — Partition leap-aware time scales (GPS/TAI/NTP) out of `PosixNs`

Status: Accepted

## Context

GPS, TAI, and NTP are not POSIX-offset. Forcing them through the POSIX spine
([0002](0002-posixns-i128-canonical-spine.md)) would corrupt their leap semantics,
and they differ from *each other* too.

## Decision

Keep the leap-aware family in its **own module** (`leap.rs`) with its **own
instant types** via `hifitime` behind a feature — do not route them through
`PosixNs`. Keep GPS, TAI64, and NTP distinct from each other:

- **NTP** is UTC-based with leap indicators and era rollover (RFC 5905) — not
  "TAI-ish".
- **GPS** has no internal leap seconds, but GPS→UTC needs the offset table.
- **TAI64/TAI64N** is pure TAI.

## Consequences

- The common, majority case (POSIX-offset formats) stays simple integer math.
- Leap correctness for GPS/TAI/NTP is delegated to `hifitime`, not hand-rolled.
- The leap-aware feature is optional, so pure POSIX users don't pay for it.
