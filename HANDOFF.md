# timeglyph — handoff

A handoff for the next Claude Code (or human) session to pick up and build out
`timeglyph`: a **forensic timestamp decipherment** engine. Read this first; it is
the design record, the decision log, and the build-out plan.

---

## 1. Mission, and the honest framing

`timeglyph` decodes/encodes/identifies the many ways systems inscribe time. It is
**not** "the first timestamp converter" — strong ones exist:
[`time_decode`](https://github.com/digitalsleuth/time_decode) (Corey Forman, **MIT**,
Python, comprehensive, already has a "guess" mode) and DCode (Digital Detective,
proprietary, Windows, 69 formats). If someone just needs a working tool, those
suffice. **Do not claim "most powerful in the world"** — one missing format
refutes it (Codex flagged this). The defensible, differentiating claim is:

> the broadest forensic timestamp coverage with **scored, cited, ambiguity-first
> interpretation** — a Rust single-static-binary reference implementation of a
> rigorous model where a reading is *evidence*, not a verdict.

That rigor is the paper's contribution: the cited evidence-metadata registry, the
PosixNs/leap-second correctness, and ambiguity treated as first-class scored
output (never "the detected format").

## 2. The two canon decisions (settled)

- **Tool/crate name: `timeglyph`.** `chrono`-prefixed names bury discoverability
  (people search crates.io for *time*/*timestamp*); `timeglyph` leads with "time"
  and keeps the metaphor: a *glyph* is an inscribed mark whose meaning must be
  deciphered — exactly what a timestamp is. Distinctive, crates.io-free. (Rejected:
  `chronoscope` — namesake `cargo-chronoscope`; `timesleuth` — collides with The
  Sleuth Kit / TSK.)
- **Rename `epoch → source_state_version` (in the PAPER and the issen/state-history
  model — NOT in this crate).** This is separate from `timeglyph`. In the
  state-history forensic model a record's `epoch` (default `'live'`) means *which
  version of system reality an observation was derived from* (live vs VSS/APFS/Time
  Machine snapshot). Calling that `epoch` collides with Unix/FILETIME/NTP epochs.
  Codex's ranked recommendation: **`source_state_version`** (`state_version` in
  code) > `generation` > qualified `state_epoch` > `vintage` > `stratum` (collides
  with NTP stratum!) > `snapshot` (too narrow; live isn't a snapshot). Also: store
  the provider-native snapshot **identity** (VSS GUID, APFS XID, btrfs subvol id,
  Time Machine date), add `source_state_kind = live|snapshot|image|backup`, and do
  NOT bake chronology into an ordinal. (This is a TODO for issen's schema + the
  paper, tracked here for cross-reference.)

## 3. Architecture

- **Canonical spine: `PosixNs(i128)`** = nanoseconds since 1970-01-01, proleptic
  Gregorian, **leap-second-IGNORING (POSIX)**. Deliberately **NOT** called UTC —
  UTC has leap discontinuities POSIX pretends away; calling it UTC is an auditable
  error (Codex). `i128` is load-bearing: FILETIME's 1601 epoch alone is ~1.16e19
  ns, which overflows `i64`.
- **Leap-aware family is partitioned out** (GPS/TAI/NTP). The majority (FILETIME,
  Chrome, Cocoa, HFS+, .NET, OLE, Unix) are POSIX-offset, pure integer math, no
  leap seconds. GPS/TAI/NTP get their **own** instant types via `hifitime` behind a
  feature — do NOT route them through `PosixNs`. Keep GPS, TAI64, and NTP separate
  from each other: NTP is UTC-based with leap indicators + era rollover (not
  "TAI-ish"); GPS has no internal leap but GPS→UTC needs the offset table; TAI64 is
  pure TAI.
- **Reuse, don't reinvent calendar math.** `jiff` = civil time + IANA tz (incl.
  historical) + ISO 8601/RFC 3339/2822/HTTP parse+format. `hifitime` = leap/TAI/GPS.
  The crate writes ZERO calendar code.
- **Registry = evidence metadata** (`src/registry.rs`): each `Format` carries a
  spec `citation`, `tz` semantics, `leap` semantics, and a plausibility window.
  Build it out to also carry: representable range, observed forensic range,
  precision-loss note, and per-format timezone caveats.
- **Interpretation is first-class & scored** (`src/interpret.rs`): `interpret_int`
  returns ALL plausible readings as `Candidate`s with `score`, named `components`,
  and `assumptions`. NEVER return a single "detected format."
- **One crate, lib + thin CLI** (Humble Object). No `-core` split (no lean library
  consumer; the auto-detecting converter is an analyst tool, not a parser
  primitive). Split to `timeglyph-core` ONLY if a fleet *library* ever links the
  primitive — the blazehash precedent, never speculative.

## 4. What's DONE (this scaffold)

- Compiles; `cargo test` green (7 spec-anchored tests); `cargo clippy --all-targets
  -- -D warnings` clean; Paranoid Gatekeeper lints on (no unwrap/expect, forbid
  unsafe).
- Core types: `PosixNs`, `Unit`, `Strategy::{LinearInt,LinearFloat}`, `Format`,
  `TzSemantics`, `LeapSemantics`; `decode_int`/`decode_float`/`encode_int` (all
  bounds-checked, panic-free); `format(id)`.
- Registry: 9 exemplar formats — `unix`, `unix_ms`, `unix_us`, `filetime`,
  `webkit`, `cocoa`, `hfsplus`, `dotnet_ticks`, `ole` — with epoch_ns constants +
  citations. **Anchors validated**: value-0 renders each format's documented epoch;
  the canonical FILETIME 116444736000000000 == 1970; Unix 1577836800 == 2020.
- `interpret_int` (ranked multi-candidate) + `interpret_hex` (LE/BE u32/u64 byte
  decode → numeric interpret).
- CLI: `timeglyph <value>` (auto-detect, ranked), `--from <id>`, `--hex`, `--list`,
  `--json` (stub).

## 4b. Added beyond the scaffold

- **Selectable output timezone** (`RenderZone`, `--tz`): UTC / fixed offset /
  IANA, DST-correct, presentation-only (the instant is unchanged). Reused as the
  *meridian* input for the lunisolar feature.
- **Lunisolar calendar + 干支 four pillars** (`src/lunisolar.rs`, `lunisolar`
  feature). Two engines (reuse, don't reinvent): the **`stem-branch`** solar
  ephemeris (Apache-2.0, h4x0r) supplies the Sun's apparent ecliptic longitude →
  the YEAR pillar (立春=315°) and MONTH pillar (the 12 节, every 30°), which are
  meridian-independent; **`lunar-lite`** (MIT) supplies the lunar (moon) calendar
  DATE its solar-only core can't. Day pillar = Julian-day arithmetic; hour pillar
  = 五鼠遁. **Key design fact** — the conversion is *convention-relative*: a UTC
  instant has no single Chinese date without a reference meridian (China UTC+8,
  Vietnam UTC+7, Korea UTC+9), so `--tz` is REQUIRED; `--longitude` optionally
  corrects the hour pillar to local mean solar time (真太陽時, equation of time not
  applied). Divergences (立春 vs 正月初一) surfaced as assumptions, never hidden.
  Validated against the independent `cnlunar` oracle. NOTE: ΔT uses the
  Espenak–Meeus modern segments (1986–2050); if stem-branch later ports its lunar
  ephemeris, `lunar-lite` could be dropped.

## 5. What's NEXT (build-out, strict TDD — RED then GREEN, separate commits)

### 5a. Format catalog (the bulk) — add to `src/registry.rs`
Each new format: find the **primary spec**, add the entry with a real `citation`,
write a RED anchor test (value-0 = epoch, plus a worked example from the spec),
then GREEN. Validate against the MIT `time_decode` oracle (see §6).

**Landed (oracle-validated):** `active` (AD/LDAP FILETIME), `prtime`, `iostime`
(iOS-11 ns NSDate), `ksuid`, `excel1904`, `mastodon`/`linkedin`/`tiktok` (the
embedded strategy was generalised to carry a `unit`, so seconds-shift IDs like
TikTok work), plus string/packed forms `ulid`, `uuid_v1`, `rfc2822`, `exif`, and
128-bit `SYSTEMTIME`. **Still TODO** — obscure packed-bitfield formats needing
per-format unpackers (exFAT tz byte, bitdate/dttm/logtime/ns40/moto/symantec/dvr,
BCD/GSM semi-octet, Sonyflake's 10ms unit); GPS/NTP/TAI remain in `leap.rs`.

Still needed (Codex's gap list + the long tail):
- **Apple/macOS**: `CFAbsoluteTime` as a **signed double** in plists/NSKeyedArchiver
  (negative = pre-2001); Core Data; APFS nanosecond timestamps; the **HFS+
  local-time caveat** (classic Mac HFS stored LOCAL, not UTC — surface this).
- **Windows**: LDAP/AD (FILETIME but distinct artifact), `SYSTEMTIME` binary struct
  (packed), the **$STANDARD_INFORMATION vs $FILE_NAME FILETIME** distinction
  (forensically significant), OLE 1900-vs-1904 systems.
- **Databases**: SQL Server `datetime` (1900-01-01 base, double days) + `datetime2`
  (100ns); PostgreSQL (2000-01-01 µs); SQLite (Julian-day float / Unix-int / text —
  three encodings); MySQL.
- **Filesystem/archive**: FAT/DOS (1980 packed bits, LOCAL time); **exFAT** (tz
  offset field); **ZIP/MS-DOS** + Info-ZIP tz extension; ext4 (32-bit + high-2-bit
  extender for post-2038); XFS/btrfs ns.
- **IDs with embedded time**: Snowflake/Discord/Twitter, UUIDv1, **UUIDv6/v7**,
  ULID, MongoDB ObjectId, KSUID, Sonyflake.
- **Time scales (leap-aware, separate module)**: GPS, NTP (RFC 5905, era rollover),
  TAI64/TAI64N, Julian Day / Modified Julian Day.
- **String forms**: ASN.1 DER `UTCTime`/`GeneralizedTime`, LDAP GeneralizedTime,
  EXIF datetime (+ optional offset), PDF date strings, ISO week/ordinal dates,
  RFC 2822 / HTTP-date.
- **Strategies to add**: `Strategy::Packed(fn)` (FAT/DOS/SYSTEMTIME/exFAT bit
  layouts), embedded-ms-with-bit-shift (Snowflake/ObjectId/UUIDv7), float-signed
  (CFAbsoluteTime), string parsers.

### 5b. Plausibility scoring — DONE
All components implemented as a named set (emitted verbatim, never just a rank):
`representable`, `in_window`, `granularity_match`, `magnitude_fit`,
`not_sentinel` always; and, behind `interpret::InterpretContext`,
`byte_width_match`, `endian_match`, `artifact_match`, `neighbour_monotonicity`
(each appears only when its context — on-disk width/byte-order, an artifact hint,
or sibling column values — is supplied, so the zero-context default is
unchanged). The hex path feeds width+endian; CSV auto-detect feeds the column as
neighbours; `--artifact` feeds the identify path. Never "looks human"; default
output stays ranked candidates. See `tests/scoring.rs`, `tests/context_scoring.rs`.

### 5c. Epistemics (mandatory)
- A single value is usually **underdetermined** — keep the multi-candidate default.
- Add a **leap-smear disclaimer**: "indistinguishable from a leap-smeared source
  without clock-policy metadata" (Google/AWS/Meta smear can't be inferred from a
  raw value).
- "consistent with", never "is" / "detected" / a verdict.

### 5d. Plumbing
- `serde::Serialize` on `Candidate` + real `--json`.
- `hifitime` feature + the TAI/GPS/NTP instant module.
- Fleet standards before publish: `deny.toml`, `clippy.toml`, fuzz target
  (`fuzz/` — one target per parser strategy; invariant = no panic), `README.md`
  (SecurityRonin standard), `docs/` + MkDocs, `LICENSE` (Apache-2.0), release.yml
  (tag-driven; Homebrew/apt-Cloudsmith/winget — see issen/CLAUDE.md), 100% line
  coverage gate (`cargo llvm-cov --lib`, `// cov:unreachable` for kept guards).
- crates.io: settle the name within 72h of first publish (the rename window).

## 6. Validation — CLEAN-ROOM (legal + canon-critical)

- **Primary specs are the source of truth.** Cite them ([MS-DTYP] for FILETIME,
  RFC 5905 for NTP, Apple TN1150 for HFS+, ECMA-335 for .NET, the GPS ICD, etc.).
- **`time_decode` (MIT) is the open reference + oracle** — you MAY read its code and
  use it as a differential oracle; its `REFERENCES.md` is a citation starting point.
- **Differential battery**: a fixed table of `(value, format) → expected` cross-
  checked against `time_decode` and each spec's worked example. Reconcile every
  divergence (Doer-Checker).
- **NEVER decompile DCode.** The downloaded `~/Downloads/DCode-…zip` is a single
  proprietary .NET `.exe`; its EULA near-certainly forbids RE and "build a competing
  product." Decompiling + reimplementing would taint a publishable work. The format
  facts are public (specs); we don't need it. At most, run it as a peripheral
  black-box sanity check — never copy from it.

## 7. How to pick up

```
cd ~/src/timeglyph
cargo test                                   # 7 green anchors
cargo clippy --all-targets -- -D warnings    # clean
cargo run -- 1577836800                      # ranked candidates demo
cargo run -- --list                          # the registry
```

Pattern to add a format: (1) find the spec + a worked example; (2) RED — add an
anchor test in `tests/anchors.rs` (value-0 = epoch + the worked example); (3) GREEN
— add the `Format` entry (or a new `Strategy` arm) in `src/registry.rs`/`src/lib.rs`;
(4) cross-check vs `time_decode`; (5) commit RED then GREEN separately.

Constraints: panic-free, `forbid(unsafe)`, no unwrap/expect outside tests; reuse
`jiff`/`hifitime` for all calendar/leap math; keep `PosixNs` POSIX (never "UTC").
