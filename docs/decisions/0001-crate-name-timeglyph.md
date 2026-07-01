# 0001 — Name the crate `timeglyph`

Status: Accepted

## Context

The tool decodes/encodes/identifies the many ways systems inscribe time. The name
had to be discoverable on crates.io (people search *time*/*timestamp*), distinct
from prior art, and free.

## Decision

Name the tool and crate **`timeglyph`**. It leads with "time" for discoverability
and keeps a fitting metaphor: a *glyph* is an inscribed mark whose meaning must be
deciphered — exactly what a raw timestamp is.

## Consequences

- Distinctive and crates.io-free at first publish.
- The rename window on crates.io is ~72h after first publish; the name was settled
  before publishing.

## Rejected alternatives

- **`chrono*`-prefixed names** — bury discoverability; searchers look for
  *time*/*timestamp*, not *chrono*.
- **`chronoscope`** — collides with the existing `cargo-chronoscope`.
- **`timesleuth`** — collides with The Sleuth Kit (TSK) in the forensic space.
