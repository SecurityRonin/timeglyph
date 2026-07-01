# 0007 — Clean-room validation: specs + `time_decode` oracle; never decompile DCode

Status: Accepted

## Context

Correctness must be provable against independent references, and the work must stay
publishable — free of intellectual-property taint from proprietary tools.

## Decision

Validate clean-room:

- **Primary specs are the source of truth** — cite them ([MS-DTYP] for FILETIME,
  RFC 5905 for NTP, Apple TN1150 for HFS+, ECMA-335 for .NET, the GPS ICD, etc.).
- **`time_decode` (MIT, Corey Forman) is the open reference and differential
  oracle** — its code may be read and used as a differential oracle; its
  `REFERENCES.md` is a citation starting point.
- **Differential battery**: a fixed table of `(value, format) → expected`
  cross-checked against `time_decode` and each spec's worked example; every
  divergence reconciled (Doer-Checker).

## Consequences

- **Never decompile DCode.** The proprietary .NET binary's EULA near-certainly
  forbids reverse engineering and building a competing product; decompiling and
  reimplementing would taint a publishable work. The format facts are public
  (specs), so it is not needed. At most run it as a black-box sanity check — never
  copy from it.
- Validation methodology is documented in [`../validation.md`](../validation.md).
