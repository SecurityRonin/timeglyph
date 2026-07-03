# Architecture Decision Records

Each ADR records one decision — the context, the choice, and its consequences —
so the *why* survives after the plan that produced it is gone. Conclusions, not
plans: these are permanent. Superseding a decision means adding a new ADR that
marks the old one `Superseded`, never editing the record of what was decided.

These were reverse-written on 2026-07-01 from the project's original design
record; the decisions themselves were settled during initial design (2026-06)
and are reflected in the shipped v0.3.0 code.

| ADR | Decision |
|---|---|
| [0001](0001-crate-name-timeglyph.md) | Name the crate `timeglyph` |
| [0002](0002-posixns-i128-canonical-spine.md) | Canonical spine is `PosixNs(i128)` — POSIX, not UTC |
| [0003](0003-partition-leap-aware-time-scales.md) | Partition leap-aware time scales (GPS/TAI/NTP) out of `PosixNs` |
| [0004](0004-reuse-calendar-and-leap-libraries.md) | Reuse `jiff`/`hifitime`; write zero calendar math |
| [0005](0005-scored-multi-candidate-interpretation.md) | Interpretation is scored and multi-candidate — never one "detected format" |
| [0006](0006-single-crate-humble-object.md) | One crate, lib + thin CLI (Humble Object) — no `-core` split |
| [0007](0007-clean-room-validation.md) | Clean-room validation: specs + `time_decode` oracle; never decompile DCode |
| [0008](0008-convention-relative-lunisolar.md) | Lunisolar conversion is convention-relative — `--tz` required |
| [0009](0009-spy-zone-display-conventions.md) | timeglyph-spy zone & time display conventions (amber chip, no ⚠, `Local (…)`, `etc.`) |
