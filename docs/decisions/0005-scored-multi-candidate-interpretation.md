# 0005 — Interpretation is scored and multi-candidate — never one "detected format"

Status: Accepted

## Context

A single raw timestamp value is usually **underdetermined**: the same integer is a
plausible reading under many formats. Returning one "detected format" would present
a guess as a verdict — the opposite of forensic rigor.

## Decision

Interpretation is **first-class and scored**. `interpret_int` returns *all*
plausible readings as `Candidate`s, each with a `score`, named `components`, and
`assumptions`. It never returns a single detected format.

- Scoring components are emitted verbatim (a named set), never collapsed to a bare
  rank: `representable`, `in_window`, `granularity_match`, `magnitude_fit`,
  `not_sentinel` always; and, when context is supplied, `byte_width_match`,
  `endian_match`, `artifact_match`, `neighbour_monotonicity`.
- **Epistemic stance:** a reading is *evidence*, not a verdict. Output says
  "consistent with", never "is" / "detected". A leap-smear disclaimer is surfaced
  where relevant (a smeared source is indistinguishable from a raw value without
  clock-policy metadata).

## Consequences

- The default output stays ranked candidates; the zero-context result is unchanged
  when no width/endian/artifact/neighbour context is available.
- The registry must carry the evidence metadata scoring depends on (citations, tz
  and leap semantics, plausibility windows).
