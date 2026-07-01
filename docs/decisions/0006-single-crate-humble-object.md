# 0006 — One crate, lib + thin CLI (Humble Object) — no `-core` split

Status: Accepted

## Context

Fleet convention sometimes splits a tool into a `-core` library plus a binary
(the blazehash precedent). That split earns its keep only when a separate library
consumer actually links the primitive.

## Decision

Ship **one crate**: a library plus a thin CLI, following the Humble Object pattern
(all decisions in testable library functions, an irreducible shell in `main`). Do
**not** split out `timeglyph-core`.

Rationale: the auto-detecting, scored converter is an *analyst tool*, not a parser
primitive a lean library consumer would embed. There is no such consumer.

## Consequences

- Simpler surface: one published crate, one version, one test suite.
- Revisit only if a fleet *library* ever needs to link the primitive — then split,
  following the blazehash precedent, never speculatively (YAGNI).
