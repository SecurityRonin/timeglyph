# Security Policy

## Threat model — a parser of attacker-influenced values

`timeglyph` decodes raw timestamp values and byte sequences that, in real
forensic use, originate from **untrusted artifacts**: disk images, memory dumps,
captured network data, files under examination. The security-relevant property is
therefore simple and absolute:

> **No input may cause a panic, a crash, or silently wrong output.**

The engine is built to that standard:

- `#![forbid(unsafe_code)]` across the crate.
- No `unwrap`/`expect`/`panic!` in library or binary code (enforced by
  `clippy::unwrap_used` / `expect_used` set to `deny`); every length, width, and
  arithmetic step is bounds-checked and overflow-checked (`i128` spine,
  `checked_mul`/`checked_add`, `try_from`).
- Out-of-range or malformed input is surfaced as a typed `ChronoError`, never as
  a default value that masks the failure.

### Fuzzing

`fuzz/` holds `cargo-fuzz` targets whose invariant is **no panic on any input**:

- `interpret_int` — arbitrary `i64` values through the full auto-detect path.
- `interpret_hex` — arbitrary byte/UTF-8 input through the hex byte-decoder.

Run locally with a nightly toolchain:

```bash
cargo +nightly fuzz run interpret_hex
```

## Reporting a vulnerability

For an actual security issue — a parser panic on crafted input, a memory-safety
concern, or silently wrong decoding — email **albert@securityronin.com** with
details and a reproducer. Please do not open a public issue for security reports.

We aim to acknowledge within a few business days and to ship a fix promptly,
crediting the reporter unless anonymity is requested.
