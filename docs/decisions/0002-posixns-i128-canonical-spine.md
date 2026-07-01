# 0002 — Canonical spine is `PosixNs(i128)` — POSIX, not UTC

Status: Accepted

## Context

Timestamp formats need a single internal instant type to convert through. The
choice of scale and width is load-bearing and auditable: getting it wrong silently
mislabels leap semantics or overflows on legitimate values.

## Decision

The canonical spine is **`PosixNs(i128)`** — nanoseconds since 1970-01-01,
proleptic Gregorian, **leap-second-ignoring (POSIX)**.

- It is deliberately **not** named UTC. UTC has leap-second discontinuities that
  POSIX pretends away; calling a POSIX count "UTC" is an auditable error.
- The width is **`i128`, not `i64`**: FILETIME's 1601 epoch alone is ~1.16 × 10¹⁹
  ns, which overflows `i64`.

## Consequences

- All POSIX-offset formats (FILETIME, Chrome/WebKit, Cocoa, HFS+, .NET, OLE, Unix)
  are pure bounds-checked integer math through `PosixNs`, no leap handling.
- Leap-aware scales must **not** route through `PosixNs` — see
  [0003](0003-partition-leap-aware-time-scales.md).
- Naming discipline: keep `PosixNs` POSIX in code and docs; never relabel it UTC.
