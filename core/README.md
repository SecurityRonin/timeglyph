# timeglyph-core

The integer half of [timeglyph](https://github.com/SecurityRonin/timeglyph):
turning platform epoch values into Unix nanoseconds.

**Zero dependencies**, and the dependency count is a checked claim rather than a
README sentence — CI asserts the resolved lockfile names exactly one package.

```toml
[dependencies]
timeglyph-core = "0.1"
```

Handles Windows `FILETIME`, WebKit/Chrome, Cocoa and HFS+ timestamps, along with
the not-set sentinel policy that distinguishes "the epoch" from "never set" —
a distinction that matters when the value came out of an artifact rather than a
clock.

This crate is deliberately narrow: it does arithmetic and nothing else. The
scanning, scoring and rendering that decide *which* interpretation of some bytes
is plausible live in `timeglyph`, and are a different kind of claim.

Its MSRV is **1.75**, verified in CI by building the crate standalone.

---

[Privacy](https://securityronin.github.io/timeglyph/privacy/) ·
[Terms](https://securityronin.github.io/timeglyph/terms/) ·
© Security Ronin Ltd
