# timeglyph

**Forensic timestamp decipherment** — decode, encode, and *identify* the many ways
systems inscribe time, with scored, cited, ambiguity-first interpretation.

A timestamp is time inscribed as a symbol: the raw integer or bytes a system
writes to mean an instant. `timeglyph` deciphers those inscriptions. Given an
*unknown* value it reports **every plausible interpretation, ranked and scored,
with its assumptions** — never a single "detected" answer, because a raw value is
usually ambiguous.

```console
$ timeglyph 1577836800
# readings consistent with 1577836800 (ranked; a raw value is usually underdetermined — not a single verdict):
  [1.00] unix           2020-01-01T00:00:00Z  (Unix time (seconds))
  [0.94] postgres       2000-01-01T00:26:17.8368Z  (PostgreSQL timestamp (µs since 2000))
  [0.67] cocoa          2051-01-01T00:00:00Z  (Cocoa / CFAbsoluteTime (s since 2001))
  ...
```

## The model

- **A reading is evidence, not a verdict.** Every candidate is framed as
  *consistent with* a format and carries its scored components and assumptions —
  including a leap-smear disclaimer for POSIX-labelled readings.
- **Ambiguity is first-class.** The default output is the ranked candidate set.
  Scoring combines named components (representable, in-window, granularity match,
  magnitude fit) so the rank is auditable, never opaque.
- **POSIX-correct spine.** The internal instant is `PosixNs(i128)` — nanoseconds
  since 1970, proleptic Gregorian, leap-second-ignoring. It is deliberately not
  called UTC; the leap-aware scales (GPS/TAI/NTP) are kept separate.
- **Reused calendar math.** Civil-time and timezone conversion is delegated to
  [`jiff`](https://docs.rs/jiff); `timeglyph` writes zero calendar code.

## Usage

```console
$ timeglyph 1577836800                       # identify (auto-detect, ranked)
$ timeglyph --json 1577836800                # machine-readable candidates
$ timeglyph --hex 0060947C58B2D501           # raw bytes, LE/BE × 32/64-bit
$ timeglyph --from filetime 132223104000000000   # decode under one known format
$ timeglyph --list                           # the registry, with spec citations
```

## Format coverage

Unix s/ms/µs/ns, FILETIME, WebKit/Chrome, Cocoa (`CFAbsoluteTime`, integer and
signed-double), HFS+, .NET ticks, OLE Automation, PostgreSQL, SQLite Julian day,
and Snowflake IDs (Twitter/X, Discord) — each carrying a primary-spec citation.
See [HANDOFF.md](https://github.com/SecurityRonin/timeglyph/blob/main/HANDOFF.md)
for the build-out plan and the leap-aware (GPS/TAI/NTP) family.
