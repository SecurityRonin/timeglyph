//! Auto-detection: identify an unknown value by reporting EVERY plausible
//! interpretation, **scored, with stated assumptions** — never "the detected
//! format." A single integer is usually underdetermined: a 64-bit value can be a
//! plausible Unix-s, Java-ms, Chrome-µs, FILETIME, .NET-ticks and Cocoa-s date
//! all at once. Presenting one as *the* answer would fabricate certainty, which a
//! forensic tool must never do (epistemics: "consistent with", not a verdict).
//!
//! Scoring is a named component set (ADR 0005): representable validity,
//! plausibility-window membership, granularity match, magnitude fit, and a
//! sentinel guard are always emitted; byte-width match, endian match,
//! artifact-context hint, and neighbour-monotonicity are emitted when an
//! [`InterpretContext`] supplies their inputs. Every component is surfaced
//! verbatim on the [`Candidate`] — a low component lowers the rank, never hides
//! the reading.

use crate::{
    registry::FORMATS, ChronoError, Format, LeapSemantics, PosixNs, Strategy, TzSemantics, Unit,
};

/// One candidate interpretation of a value. Carries its score *components* and
/// *assumptions*, not just a rank — transparency over false confidence.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Candidate {
    /// Format id (e.g. `"filetime"`).
    pub format_id: &'static str,
    /// Human label.
    pub label: &'static str,
    /// Spec citation for the assumed format.
    pub citation: &'static str,
    /// The decoded instant.
    pub instant: PosixNs,
    /// RFC 3339 rendering, or `None` if outside the civil range.
    pub rendered: Option<String>,
    /// Overall plausibility score in `[0, 1]` (scaffold: window membership).
    pub score: f64,
    /// The individual scored components (named), for auditability.
    pub components: Vec<(&'static str, f64)>,
    /// Assumptions made to produce this reading (e.g. the format + citation).
    pub assumptions: Vec<String>,
    /// True when the raw value is a well-known "magic" sentinel (0/unset, −1,
    /// `i64::MAX`/never) rather than a real instant. Machine-readable so pipelines
    /// can refuse to treat it as authoritative (see also [`Candidate::score`]).
    pub sentinel: bool,
}

/// Byte order of a value observed on disk — supplied via [`InterpretContext`] so
/// the `endian_match` component can reward the order that yields a plausible
/// date over its byte-swapped alternative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    /// Least-significant byte first.
    Little,
    /// Most-significant byte first.
    Big,
}

/// Extra context that sharpens scoring beyond what a bare integer reveals
/// (ADR 0005). Each field, when present, unlocks one additional named
/// component; an all-default context (the [`interpret_int`] path) emits none of
/// them, so the zero-knowledge default is exactly the prior behaviour.
#[derive(Debug, Clone, Default)]
pub struct InterpretContext<'a> {
    /// The observed on-disk storage width in bytes (e.g. the 4 or 8 bytes a hex
    /// input occupied). Unlocks `byte_width_match`.
    pub observed_width_bytes: Option<u8>,
    /// The observed byte order. Unlocks `endian_match` (needs a width too).
    pub endian: Option<Endian>,
    /// A free-text artifact/source hint (e.g. `"chrome history"`, `"ntfs mft"`).
    /// Unlocks `artifact_match`.
    pub artifact: Option<&'a str>,
    /// Sibling values from the same column/sequence. Unlocks
    /// `neighbour_monotonicity` (does this format order the column sanely?).
    pub neighbours: &'a [i64],
}

/// Interpret a raw integer across every integer-decodable format (linear,
/// embedded-millisecond IDs, and packed). Returns ALL readings that render to a
/// civil date, ranked by score (descending), then by id for determinism. The
/// caller MUST present these as candidates, not a single answer.
///
/// ```
/// let candidates = timeglyph::interpret::interpret_int(1_577_836_800);
/// // A raw value is underdetermined — expect several plausible readings.
/// assert!(candidates.len() >= 2);
/// // The top-ranked reading carries its scored components and assumptions.
/// let top = &candidates[0];
/// assert!(top.components.iter().any(|(name, _)| *name == "granularity_match"));
/// assert!(!top.assumptions.is_empty());
/// ```
#[must_use]
pub fn interpret_int(value: i64) -> Vec<Candidate> {
    interpret_int_with_context(value, &InterpretContext::default())
}

/// Like [`interpret_int`], but with an [`InterpretContext`] supplying the
/// on-disk width/byte-order, an artifact hint, and/or sibling column values.
/// Each present context field adds one named component to every candidate; an
/// empty context reproduces [`interpret_int`] exactly. The ranking is otherwise
/// identical: ALL civil-renderable readings, scored, never one verdict.
#[must_use]
#[tracing::instrument(level = "debug", skip(ctx))]
pub fn interpret_int_with_context(value: i64, ctx: &InterpretContext) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    for f in FORMATS {
        // Any integer-decodable strategy is a candidate; float-only and
        // out-of-range readings are skipped inside build_candidate.
        if let Some(c) = build_candidate(f, value, ctx) {
            out.push(c);
        }
    }
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.format_id.cmp(b.format_id))
    });
    out
}

/// Build a scored, assumption-carrying candidate for one format + integer value,
/// or `None` if the value is not integer-decodable under it or renders outside the
/// civil range. Shared by [`interpret_int`] and the per-format hex decoders.
fn build_candidate(f: &Format, value: i64, ctx: &InterpretContext) -> Option<Candidate> {
    let instant = f.decode_int(value).ok()?;
    let rendered = instant.to_rfc3339()?;
    let components = score_components(f, value, instant, ctx);
    let score = overall_score(&components);
    let mut assumptions = assumptions(f);
    let sentinel = sentinel_reason(value);
    if let Some(reason) = sentinel {
        assumptions.push(format!(
            "value {value} is a likely sentinel ({reason}) — an 'unset'/'never' marker, not necessarily a real instant"
        ));
    }
    Some(Candidate {
        format_id: f.id,
        label: f.label,
        citation: f.citation,
        instant,
        rendered: Some(rendered),
        score,
        components,
        assumptions,
        sentinel: sentinel.is_some(),
    })
}

/// Decode a single named format from an integer value, returning its candidate.
fn decode_one(format_id: &str, value: i64, ctx: &InterpretContext) -> Option<Candidate> {
    build_candidate(crate::format(format_id).ok()?, value, ctx)
}

/// The stated assumptions behind one reading (ADR 0005 epistemics). A reading
/// is evidence, not a verdict: it is framed as *consistent with* a format, never
/// "detected". POSIX-labelled readings additionally carry the leap-smear
/// disclaimer — a raw value cannot reveal whether its source clock smeared leap
/// seconds (Google/AWS/Meta smear is invisible without clock-policy metadata).
fn assumptions(f: &Format) -> Vec<String> {
    let mut out = vec![format!(
        "consistent with {} [{}] — a reading, not a determination",
        f.label, f.citation
    )];
    if matches!(f.leap, LeapSemantics::PosixIgnored) {
        out.push(
            "indistinguishable from a leap-smeared source without clock-policy metadata"
                .to_string(),
        );
    }
    if matches!(f.tz, TzSemantics::LocalNaive) {
        out.push(
            "stored as LOCAL wall-clock time with no offset — the instant is naive, not UTC"
                .to_string(),
        );
    }
    out
}

/// Well-known "magic" sentinel values that denote unset/never/error rather than
/// a real instant (a zero/uninitialized field, an all-ones marker, the Active
/// Directory `accountExpires = 0x7FFFFFFFFFFFFFFF` "never"). Detecting them is the
/// front line against silently rendering a sentinel as a plausible date. NOTE:
/// `0xFFFFFFFF` (u32 max) is deliberately NOT listed — it is the genuine HFS+
/// maximum date, not a sentinel. Public so the CLI can flag a sentinel even on
/// the single-format `decode` path (which does not build a [`Candidate`]).
#[must_use]
pub fn sentinel_reason(value: i64) -> Option<&'static str> {
    match value {
        // Generic value sentinels: suggestive across any format ("possible").
        0 => Some("possible sentinel: zero / unset"),
        -1 => Some("possible sentinel: -1 / all-ones (unset)"),
        // Format-specific magic value with a documented meaning ("known").
        i64::MAX => Some("known sentinel: 0x7FFFFFFFFFFFFFFF (e.g. AD accountExpires 'never')"),
        _ => None,
    }
}

/// The named plausibility components for one reading (ADR 0005). Each is in
/// `[0, 1]` and emitted verbatim on the `Candidate` so a reviewer can audit the
/// rank instead of trusting an opaque number. NEVER a filter — a low component
/// lowers the rank, it does not hide the reading.
fn score_components(
    f: &Format,
    value: i64,
    instant: PosixNs,
    ctx: &InterpretContext,
) -> Vec<(&'static str, f64)> {
    // representable: surfaced only when civil-renderable, so always 1.0 here —
    // emitted explicitly so the component set is complete and self-describing.
    let representable = 1.0;
    let in_window = f64::from(u8::from(
        instant.0 >= f.plausible.0 && instant.0 < f.plausible.1,
    ));
    let granularity = granularity_match(f.strategy, value);
    let magnitude = magnitude_fit(f.strategy, instant);
    let not_sentinel = f64::from(u8::from(sentinel_reason(value).is_none()));
    let mut components = vec![
        ("representable", representable),
        ("in_window", in_window),
        ("granularity_match", granularity),
        ("magnitude_fit", magnitude),
        ("not_sentinel", not_sentinel),
    ];
    // Context-unlocked components (ADR 0005): each appears ONLY when its
    // context is supplied, so the zero-context default is byte-for-byte the old
    // five-component set.
    if let Some(width) = ctx.observed_width_bytes {
        components.push(("byte_width_match", byte_width_match(f, value, width)));
        if ctx.endian.is_some() {
            components.push(("endian_match", endian_match(f, value, width)));
        }
    }
    if let Some(hint) = ctx.artifact {
        components.push(("artifact_match", artifact_match(f, hint)));
    }
    if !ctx.neighbours.is_empty() {
        components.push((
            "neighbour_monotonicity",
            neighbour_monotonicity(f, ctx.neighbours),
        ));
    }
    components
}

/// Number of base-256 (byte) digits needed to store `value` (minimum 1).
fn significant_bytes(value: i64) -> u8 {
    let n = value.unsigned_abs();
    if n == 0 {
        return 1;
    }
    ((64 - n.leading_zeros()).div_ceil(8)) as u8
}

/// Does the observed on-disk width match the format's natural storage width? An
/// exact match is full evidence; a value that would still fit the format's
/// narrower native field (plausibly zero-extended) is a partial fit; a value
/// that cannot fit the native field at all is a mismatch.
fn byte_width_match(f: &Format, value: i64, observed: u8) -> f64 {
    let natural = f.storage_bytes();
    if observed == natural {
        1.0
    } else if significant_bytes(value) <= natural {
        0.5
    } else {
        0.0
    }
}

/// Whether `value` decodes to an in-window instant under `f`.
fn decode_in_window(f: &Format, value: i64) -> bool {
    f.decode_int(value)
        .ok()
        .is_some_and(|inst| inst.0 >= f.plausible.0 && inst.0 < f.plausible.1)
}

/// The same `value`'s bytes read in the opposite order, at the observed width.
/// `None` for widths other than 4 or 8 (no meaningful swap).
fn byte_swapped(value: i64, width: u8) -> Option<i64> {
    match width {
        4 => u32::try_from(value).ok().map(|v| i64::from(v.swap_bytes())),
        8 => Some((value as u64).swap_bytes() as i64),
        _ => None,
    }
}

/// Does the observed byte order yield a plausible date where the byte-swapped
/// alternative does not? Disambiguated-in-our-favour → 1.0; both orders
/// plausible (genuinely ambiguous) → 0.5; this order out of window → 0.0.
fn endian_match(f: &Format, value: i64, width: u8) -> f64 {
    let this_in = decode_in_window(f, value);
    let flip_in = byte_swapped(value, width).is_some_and(|v| decode_in_window(f, v));
    match (this_in, flip_in) {
        (true, false) => 1.0,
        (true, true) => 0.5,
        (false, _) => 0.0,
    }
}

/// Does an artifact/source hint name this format's family? A keyword (≥3 chars)
/// of the hint appearing in the format's id/family/label is a full match; no
/// overlap is a weak non-match (0.2) — a hint nudges the rank, never a filter.
fn artifact_match(f: &Format, hint: &str) -> f64 {
    let haystack = format!("{} {} {}", f.id, f.family, f.label).to_lowercase();
    let matched = hint
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3)
        .any(|t| haystack.contains(&t.to_lowercase()));
    if matched {
        1.0
    } else {
        0.2
    }
}

/// Across the sibling column values, the fraction of consecutive pairs that this
/// format orders sanely: both decode in-window AND value order matches time
/// order. Linear formats keep order trivially, so this rewards a format under
/// which the WHOLE column lands in plausible range (and penalises one that
/// scatters it). A lone neighbour falls back to its own in-window membership.
fn neighbour_monotonicity(f: &Format, neighbours: &[i64]) -> f64 {
    if neighbours.len() < 2 {
        return f64::from(u8::from(
            neighbours.first().is_some_and(|&v| decode_in_window(f, v)),
        ));
    }
    let mut consistent = 0u32;
    let mut total = 0u32;
    for pair in neighbours.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        total += 1;
        let (ia, ib) = (f.decode_int(a).ok(), f.decode_int(b).ok());
        if let (Some(ta), Some(tb)) = (ia, ib) {
            let in_window = decode_in_window(f, a) && decode_in_window(f, b);
            if in_window && ((b >= a) == (tb.0 >= ta.0)) {
                consistent += 1;
            }
        }
    }
    f64::from(consistent) / f64::from(total)
}

/// Two years in nanoseconds — the ramp over which an embedded-ID timestamp is
/// considered to have a "realistic" distance from its scheme epoch.
const TWO_YEARS_NS: i128 = 730 * 86_400 * 1_000_000_000;

/// Whether the value's magnitude is consistent with the format's encoding. For
/// linear formats the window already governs magnitude (→ `1.0`). For embedded
/// IDs it is diagnostic: a tiny value decodes to an instant essentially AT the
/// scheme epoch (`id >> shift ≈ 0`), which is implausible for a real ID — so the
/// score ramps from `0.0` at the epoch to `1.0` two years past it.
fn magnitude_fit(strategy: Strategy, instant: PosixNs) -> f64 {
    match strategy {
        Strategy::Embedded { epoch_ns, .. } => {
            let past = instant.0 - epoch_ns;
            if past <= 0 {
                0.0
            } else {
                (past as f64 / TWO_YEARS_NS as f64).min(1.0)
            }
        }
        Strategy::LinearInt { .. } | Strategy::LinearFloat { .. } | Strategy::Packed { .. } => 1.0,
    }
}

/// How well the raw value's sub-second resolution fits the format's unit. A
/// whole-second value read as nanoseconds is suspiciously coarse (`0.0`); a
/// value carrying real sub-second digits fits perfectly (`1.0`). Coarse units
/// (seconds/days) never penalise. This is the core seconds-vs-ms-vs-µs-vs-ns
/// disambiguation, expressed structurally rather than by "looks human".
fn granularity_match(strategy: Strategy, value: i64) -> f64 {
    let unit: Unit = match strategy {
        Strategy::LinearInt { unit, .. }
        | Strategy::LinearFloat { unit, .. }
        | Strategy::Embedded { unit, .. } => unit,
        // Packed civil fields have no linear sub-second unit to mismatch against.
        Strategy::Packed { .. } => return 1.0,
    };
    let ssd = unit.sub_second_digits();
    if ssd == 0 {
        return 1.0;
    }
    let tz = trailing_zeros_base10(value).min(ssd);
    1.0 - f64::from(tz) / f64::from(ssd)
}

/// Count of trailing base-10 zeros of `value` (0 for the value `0` itself).
/// Uses `unsigned_abs` so `i64::MIN` cannot panic.
fn trailing_zeros_base10(value: i64) -> u32 {
    let mut n = value.unsigned_abs();
    if n == 0 {
        return 0;
    }
    let mut z = 0;
    while n.is_multiple_of(10) {
        z += 1;
        n /= 10;
    }
    z
}

/// Weighted mean of the named components. `in_window` carries double weight (it
/// is the dominant prior on which readings to surface first); the others weigh
/// one. The result is the overall `[0, 1]` rank.
fn overall_score(components: &[(&'static str, f64)]) -> f64 {
    // Double-weighted: the plausibility prior, the magnitude/sentinel guards, and
    // the structural disk-layout/column signals (when present). Everything else
    // (granularity, representable, the softer artifact hint) weighs one.
    let weight = |name: &str| match name {
        "in_window"
        | "magnitude_fit"
        | "not_sentinel"
        | "byte_width_match"
        | "endian_match"
        | "neighbour_monotonicity" => 2.0,
        _ => 1.0,
    };
    let (num, den) = components.iter().fold((0.0, 0.0), |(num, den), (n, v)| {
        let w = weight(n);
        (num + w * v, den + w)
    });
    if den == 0.0 {
        0.0
    } else {
        num / den
    }
}

/// Decode hex bytes as little- and big-endian integers of common widths, then
/// run each through [`interpret_int`]. Returns `(byte-decode assumption,
/// candidates)` per width/endianness — the byte layout is itself an assumption.
pub fn interpret_hex(hex: &str) -> Result<Vec<(String, Vec<Candidate>)>, ChronoError> {
    let clean: String = hex
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != ':')
        .collect();
    let clean = clean.strip_prefix("0x").unwrap_or(&clean);
    let bytes = hex::decode(clean).map_err(|_| ChronoError::OutOfRange {
        what: "hex (not valid hex bytes)",
        value: 0,
    })?;
    let mut out = Vec::new();
    for (label, value, width, endian) in byte_ints(&bytes) {
        // The hex layer KNOWS the on-disk width and byte order — pass them so the
        // byte_width_match + endian_match components are scored.
        let ctx = InterpretContext {
            observed_width_bytes: Some(width),
            endian: Some(endian),
            ..Default::default()
        };
        out.push((label, interpret_int_with_context(value, &ctx)));
    }
    // Packed formats have an ON-DISK byte order distinct from a linear integer,
    // and FAT is doubly ambiguous: the DOS packed convention is date-word then
    // time-word, but a FAT DIRECTORY entry stores time-word then date-word (each
    // little-endian). The same 4 bytes therefore mean two different instants —
    // surface BOTH, clearly labelled, rather than silently swap date and time.
    if let Some(four) = bytes.get(..4).and_then(|s| <[u8; 4]>::try_from(s).ok()) {
        let lo = u16::from_le_bytes([four[0], four[1]]);
        let hi = u16::from_le_bytes([four[2], four[3]]);
        // Packed FAT/DOS is a 4-byte field; its internal word order is surfaced
        // explicitly below, so no endian component (it would double-count).
        let fat_ctx = InterpretContext {
            observed_width_bytes: Some(4),
            ..Default::default()
        };
        // date-word first (DOS packed): date = bytes[0..2], time = bytes[2..4].
        if let Some(c) = decode_one("fat", (i64::from(lo) << 16) | i64::from(hi), &fat_ctx) {
            out.push(("FAT/DOS bytes date|time (LE words)".to_string(), vec![c]));
        }
        // time-word first (FAT directory order): time = bytes[0..2], date = bytes[2..4].
        if let Some(c) = decode_one("fat", (i64::from(hi) << 16) | i64::from(lo), &fat_ctx) {
            out.push((
                "FAT/DOS bytes time|date (LE words, directory order)".to_string(),
                vec![c],
            ));
        }
    }
    // Microsoft 128-bit SYSTEMTIME: 8 little-endian u16 fields
    // (year, month, dayOfWeek, day, hour, minute, second, milliseconds).
    if let Some(sixteen) = bytes.get(..16) {
        if let Some(c) = systemtime_candidate(sixteen) {
            out.push((
                "SYSTEMTIME (16-byte struct, LE u16 fields)".to_string(),
                vec![c],
            ));
        }
    }
    // An all-ones 64-bit value exceeds i64 (so yields no linear reading) but is a
    // common 'unset'/'never' sentinel — surface it explicitly rather than vanish.
    if bytes
        .get(..8)
        .and_then(|s| <[u8; 8]>::try_from(s).ok())
        .is_some_and(|e| u64::from_le_bytes(e) == u64::MAX)
    {
        out.push(("u64 all-ones".to_string(), vec![all_ones_sentinel()]));
    }
    Ok(out)
}

/// Decode a Microsoft `SYSTEMTIME` struct (16 bytes, 8 little-endian `u16`
/// fields) into a self-describing candidate. The `dayOfWeek` field (index 2) is
/// redundant and ignored. `None` if the civil fields are invalid.
fn systemtime_candidate(b: &[u8]) -> Option<Candidate> {
    let field = |i: usize| -> Option<u16> {
        let lo = *b.get(i * 2)?;
        let hi = *b.get(i * 2 + 1)?;
        Some(u16::from_le_bytes([lo, hi]))
    };
    let year = i16::try_from(field(0)?).ok()?;
    let month = i8::try_from(field(1)?).ok()?;
    let day = i8::try_from(field(3)?).ok()?;
    let hour = i8::try_from(field(4)?).ok()?;
    let minute = i8::try_from(field(5)?).ok()?;
    let second = i8::try_from(field(6)?).ok()?;
    let millis = field(7)?;
    // [MS-DTYP] §2.3.13: wMilliseconds is 0..=999. A larger value is not a valid
    // SYSTEMTIME (and would overflow the i32 nanosecond conversion), so reject it
    // rather than fabricate an instant.
    if millis > 999 {
        return None;
    }
    let subsec_nanos = i32::from(millis) * 1_000_000;
    let instant = civil_to_posix(year, month, day, hour, minute, second, subsec_nanos, 0)?;
    Some(string_candidate(
        "systemtime",
        "Microsoft 128-bit SYSTEMTIME",
        "[MS-DTYP] §2.3.13 SYSTEMTIME (8× little-endian WORD fields)",
        instant,
        "decoded as a 16-byte SYSTEMTIME struct (UTC unless the source noted local)",
    ))
}

/// Decode the first 4 and 8 bytes as LE/BE integers (panic-free, bounds-checked).
/// Labels note when only a prefix of a longer input was used, so trailing bytes
/// are never silently dropped.
fn byte_ints(b: &[u8]) -> Vec<(String, i64, u8, Endian)> {
    let total = b.len();
    let suffix = |w: usize| {
        if total > w {
            format!(" (first {w} of {total})")
        } else {
            String::new()
        }
    };
    let mut v = Vec::new();
    if let Some(four) = b.get(..4).and_then(|s| <[u8; 4]>::try_from(s).ok()) {
        v.push((
            format!("u32 LE{}", suffix(4)),
            i64::from(u32::from_le_bytes(four)),
            4,
            Endian::Little,
        ));
        v.push((
            format!("u32 BE{}", suffix(4)),
            i64::from(u32::from_be_bytes(four)),
            4,
            Endian::Big,
        ));
    }
    if let Some(eight) = b.get(..8).and_then(|s| <[u8; 8]>::try_from(s).ok()) {
        if let Ok(n) = i64::try_from(u64::from_le_bytes(eight)) {
            v.push((format!("u64 LE{}", suffix(8)), n, 8, Endian::Little));
        }
        if let Ok(n) = i64::try_from(u64::from_be_bytes(eight)) {
            v.push((format!("u64 BE{}", suffix(8)), n, 8, Endian::Big));
        }
    }
    v
}

/// A sentinel candidate for an all-ones value, which does not fit `i64` and so
/// produces no linear reading — surfaced (never hidden) and flagged.
fn all_ones_sentinel() -> Candidate {
    Candidate {
        format_id: "sentinel",
        label: "all-ones value (0xFFFFFFFFFFFFFFFF)",
        citation: "",
        instant: PosixNs(0),
        rendered: None,
        score: 0.0,
        components: vec![("not_sentinel", 0.0)],
        assumptions: vec![
            "0xFFFFFFFFFFFFFFFF — all-ones; commonly an 'unset'/'never' marker, not a real instant"
                .to_string(),
        ],
        sentinel: true,
    }
}

/// Parse a STRING timestamp form: ISO 8601 / RFC 3339, and ASN.1 UTCTime /
/// GeneralizedTime (ITU-T X.680, RFC 5280) as found in X.509 certificates and
/// PKI structures. Returns every form that parses — a string is usually
/// self-describing, so these readings score high. Empty for unparseable input.
// A flat sequence of independent string-form parse attempts (ISO/RFC/ASN.1/
// ULID/UUID/EXIF/ordinal/week); long by nature, not complex.
#[allow(clippy::too_many_lines)]
/// A string-format parser with a *fixed* interpretation: given the trimmed input
/// it yields the instant iff the input matches its grammar, and its
/// `(id, label, spec, note)` are constant. These live in one data-driven table
/// (`STRING_FORMATS`) instead of a dozen near-identical blocks. Formats whose
/// note is *dynamic* — ISO 8601 (jiff) and the two ASN.1 forms (note depends on
/// whether an explicit offset was present) — get their own push-helpers below.
struct StringFormat {
    parse: fn(&str) -> Option<PosixNs>,
    id: &'static str,
    label: &'static str,
    spec: &'static str,
    note: &'static str,
}

/// The registry of fixed-interpretation string formats, in report order.
const STRING_FORMATS: &[StringFormat] = &[
    StringFormat {
        parse: parse_ulid,
        id: "ulid",
        label: "ULID (first 48 bits = Unix ms)",
        spec: "ULID spec (Crockford base32; 48-bit ms timestamp)",
        note: "parsed as a ULID — the leading 48 bits are milliseconds since the Unix epoch",
    },
    StringFormat {
        parse: parse_uuid_v1,
        id: "uuid_v1",
        label: "UUID version 1 (100ns since 1582-10-15)",
        spec: "RFC 9562 §5.1 (UUIDv1 60-bit Gregorian timestamp)",
        note: "parsed as a UUIDv1 — a 60-bit count of 100ns intervals since 1582-10-15 UTC",
    },
    StringFormat {
        parse: parse_uuid_v6,
        id: "uuid_v6",
        label: "UUID version 6 (reordered 100ns since 1582-10-15)",
        spec: "RFC 9562 §5.6 (UUIDv6 60-bit Gregorian timestamp)",
        note: "parsed as a UUIDv6 — the v1 Gregorian timestamp reordered most-significant-first",
    },
    StringFormat {
        parse: parse_uuid_v7,
        id: "uuid_v7",
        label: "UUID version 7 (Unix ms in the high 48 bits)",
        spec: "RFC 9562 §5.7 (UUIDv7 48-bit Unix-ms timestamp)",
        note: "parsed as a UUIDv7 — the leading 48 bits are milliseconds since the Unix epoch",
    },
    StringFormat {
        parse: parse_objectid,
        id: "objectid",
        label: "MongoDB ObjectId (Unix seconds in the first 4 bytes)",
        spec: "MongoDB ObjectId spec (4-byte big-endian Unix-seconds prefix)",
        note: "parsed as a MongoDB ObjectId — the first 4 bytes are big-endian Unix seconds",
    },
    StringFormat {
        parse: parse_rfc2822,
        id: "rfc2822",
        label: "RFC 2822 / email date",
        spec: "RFC 5322 §3.3 (date-time; via jiff)",
        note: "parsed as an RFC 2822 date-time (offset normalised to UTC)",
    },
    StringFormat {
        parse: parse_exif,
        id: "exif",
        label: "EXIF DateTime (YYYY:MM:DD HH:MM:SS)",
        spec: "CIPA DC-008 (EXIF) DateTime / DateTimeOriginal",
        note: "parsed as an EXIF DateTime; NO offset is stored — assumed UTC, but is usually local time",
    },
    StringFormat {
        parse: parse_iso_ordinal,
        id: "iso_ordinal",
        label: "ISO 8601 ordinal date (YYYY-DDD)",
        spec: "ISO 8601 §5.2.2.1 (ordinal date)",
        note: "parsed as an ISO 8601 ordinal date (day-of-year), midnight UTC assumed",
    },
    StringFormat {
        parse: parse_iso_week,
        id: "iso_week",
        label: "ISO 8601 week date (YYYY-Www-D)",
        spec: "ISO 8601 §5.2.3 (week date)",
        note: "parsed as an ISO 8601 week date, midnight UTC assumed",
    },
];

#[must_use]
#[tracing::instrument(level = "debug", skip(text), fields(len = text.len()))]
pub fn interpret_string(text: &str) -> Vec<Candidate> {
    let s = text.trim();
    let mut out = Vec::new();
    // Dynamic-note formats first (ISO 8601 + ASN.1), then the fixed-note registry.
    push_iso8601(s, &mut out);
    push_asn1(s, &mut out);
    for f in STRING_FORMATS {
        if let Some(instant) = (f.parse)(s) {
            out.push(string_candidate(f.id, f.label, f.spec, instant, f.note));
        }
    }
    out
}

/// RFC 3339 / ISO 8601: jiff parses the offset (or `Z`) and normalises to UTC.
fn push_iso8601(s: &str, out: &mut Vec<Candidate>) {
    if let Ok(ts) = s.parse::<jiff::Timestamp>() {
        out.push(string_candidate(
            "iso8601",
            "ISO 8601 / RFC 3339 string",
            "ISO 8601:2019 / RFC 3339",
            PosixNs(ts.as_nanosecond()),
            "parsed as an ISO 8601 / RFC 3339 string (offset normalised to UTC)",
        ));
    }
}

/// The two ASN.1 string forms, whose assumption note depends on whether the input
/// carried an explicit offset (`had_tz`).
fn push_asn1(s: &str, out: &mut Vec<Candidate>) {
    if let Some((instant, had_tz)) = parse_asn1_generalizedtime(s) {
        out.push(string_candidate(
            "asn1_generalizedtime",
            "ASN.1 GeneralizedTime",
            "ITU-T X.680 / RFC 5280 §4.1.2.5.2",
            instant,
            &asn1_assumption("GeneralizedTime (4-digit year)", had_tz),
        ));
    }
    if let Some((instant, had_tz)) = parse_asn1_utctime(s) {
        out.push(string_candidate(
            "asn1_utctime",
            "ASN.1 UTCTime",
            "ITU-T X.680 / RFC 5280 §4.1.2.5.1",
            instant,
            &asn1_assumption(
                "UTCTime (2-digit year; RFC 5280 pivot: <50 => 20YY, else 19YY)",
                had_tz,
            ),
        ));
    }
}

/// Parse an ISO 8601 ordinal date `YYYY-DDD` (day-of-year) to midnight UTC.
fn parse_iso_ordinal(s: &str) -> Option<PosixNs> {
    let (y, d) = s.split_once('-')?;
    if y.len() != 4 || d.len() != 3 || !d.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year: i16 = y.parse().ok()?;
    let doy: i64 = d.parse().ok()?;
    let date = jiff::civil::Date::new(year, 1, 1)
        .ok()?
        .checked_add(jiff::Span::new().days(doy - 1))
        .ok()?;
    civil_to_posix(date.year(), date.month(), date.day(), 0, 0, 0, 0, 0)
}

/// Parse an ISO 8601 week date `YYYY-Www-D` (D = 1 Mon .. 7 Sun) to midnight UTC.
fn parse_iso_week(s: &str) -> Option<PosixNs> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 {
        return None;
    }
    let year: i16 = parts[0].parse().ok()?;
    let week: i8 = parts[1].strip_prefix('W')?.parse().ok()?;
    let day: i8 = parts[2].parse().ok()?;
    let weekday = jiff::civil::Weekday::from_monday_one_offset(day).ok()?;
    let date = jiff::civil::ISOWeekDate::new(year, week, weekday)
        .ok()?
        .date();
    civil_to_posix(date.year(), date.month(), date.day(), 0, 0, 0, 0, 0)
}

/// Decode a 26-character Crockford-base32 ULID; its leading 48 bits are
/// milliseconds since the Unix epoch (the trailing 80 bits are random). `None`
/// for any string that is not a well-formed ULID (so it never false-matches).
fn parse_ulid(s: &str) -> Option<PosixNs> {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    if s.len() != 26 {
        return None;
    }
    let mut value: u128 = 0;
    for ch in s.bytes() {
        let up = ch.to_ascii_uppercase();
        let idx = ALPHABET.iter().position(|&a| a == up)?;
        value = value.checked_mul(32)?.checked_add(idx as u128)?;
    }
    let ms = i128::from(u64::try_from(value >> 80).ok()?);
    Some(PosixNs(ms.checked_mul(Unit::Millis.nanos())?))
}

/// 100ns intervals between the UUID Gregorian epoch (1582-10-15) and the Unix
/// epoch, ×100 → nanoseconds: −12_219_292_800 s.
const UUID_V1_EPOCH_NS: i128 = -12_219_292_800 * 1_000_000_000;

/// Decode a UUID **version 1** timestamp: a 60-bit count of 100ns intervals
/// since 1582-10-15 UTC, split across the time_low/mid/hi fields. Returns `None`
/// unless the string is a valid UUID whose version nibble is 1 (a v3/4/5 random
/// or name-based UUID carries no instant and must not be misread as one).
fn parse_uuid_v1(s: &str) -> Option<PosixNs> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let time_low = u64::from_str_radix(hex.get(0..8)?, 16).ok()?;
    let time_mid = u64::from_str_radix(hex.get(8..12)?, 16).ok()?;
    let time_hi_version = u64::from_str_radix(hex.get(12..16)?, 16).ok()?;
    if (time_hi_version >> 12) != 1 {
        return None; // not a version-1 (time-based) UUID
    }
    let ts = ((time_hi_version & 0x0FFF) << 48) | (time_mid << 32) | time_low;
    let ns = i128::from(ts)
        .checked_mul(100)?
        .checked_add(UUID_V1_EPOCH_NS)?;
    Some(PosixNs(ns))
}

/// Strip hyphens and validate a 32-hex-digit UUID, returning its 16 bytes' worth
/// of hex. `None` for anything that is not a well-formed UUID.
fn uuid_hex(s: &str) -> Option<String> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    (hex.len() == 32 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hex)
}

/// Decode a UUID **version 6** timestamp: the same 60-bit Gregorian 100ns count
/// as v1, but laid out most-significant-first as time_high(32) time_mid(16)
/// time_low(12). `None` unless the version nibble is 6.
fn parse_uuid_v6(s: &str) -> Option<PosixNs> {
    let hex = uuid_hex(s)?;
    let time_high = u64::from_str_radix(hex.get(0..8)?, 16).ok()?;
    let time_mid = u64::from_str_radix(hex.get(8..12)?, 16).ok()?;
    let time_low_ver = u64::from_str_radix(hex.get(12..16)?, 16).ok()?;
    if (time_low_ver >> 12) != 6 {
        return None;
    }
    let ts = (time_high << 28) | (time_mid << 12) | (time_low_ver & 0x0FFF);
    let ns = i128::from(ts)
        .checked_mul(100)?
        .checked_add(UUID_V1_EPOCH_NS)?;
    Some(PosixNs(ns))
}

/// Decode a UUID **version 7** timestamp: the high 48 bits are milliseconds
/// since the Unix epoch. `None` unless the version nibble is 7.
fn parse_uuid_v7(s: &str) -> Option<PosixNs> {
    let hex = uuid_hex(s)?;
    let high32 = u64::from_str_radix(hex.get(0..8)?, 16).ok()?;
    let mid16 = u64::from_str_radix(hex.get(8..12)?, 16).ok()?;
    let ver = u64::from_str_radix(hex.get(12..16)?, 16).ok()? >> 12;
    if ver != 7 {
        return None;
    }
    let ms = i128::from((high32 << 16) | mid16);
    Some(PosixNs(ms.checked_mul(Unit::Millis.nanos())?))
}

/// Decode a MongoDB ObjectId (24 hex chars): the first 4 bytes are a big-endian
/// Unix-seconds timestamp. `None` for anything that is not a 24-hex ObjectId.
fn parse_objectid(s: &str) -> Option<PosixNs> {
    if s.len() != 24 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let secs = i128::from(u32::from_str_radix(s.get(0..8)?, 16).ok()?);
    Some(PosixNs(secs.checked_mul(Unit::Seconds.nanos())?))
}

/// Parse an RFC 2822 / email date-time (e.g. `Sun, 04 May 2025 15:18:50 +0000`)
/// via jiff, normalising to the POSIX instant. `None` if it does not parse.
fn parse_rfc2822(s: &str) -> Option<PosixNs> {
    jiff::fmt::rfc2822::parse(s)
        .ok()
        .map(|zoned| PosixNs(zoned.timestamp().as_nanosecond()))
}

/// Parse an EXIF DateTime string `YYYY:MM:DD HH:MM:SS` (colon-separated date,
/// the EXIF convention). EXIF stores no offset, so the instant is assumed UTC
/// (surfaced in the assumption). `None` for anything not matching the shape.
fn parse_exif(text: &str) -> Option<PosixNs> {
    let (date, time) = text.trim().split_once(' ')?;
    let date_parts: Vec<&str> = date.split(':').collect();
    let time_parts: Vec<&str> = time.split(':').collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return None;
    }
    let year: i16 = date_parts[0].parse().ok()?;
    let month: i8 = date_parts[1].parse().ok()?;
    let day: i8 = date_parts[2].parse().ok()?;
    let hour: i8 = time_parts[0].parse().ok()?;
    let minute: i8 = time_parts[1].parse().ok()?;
    let second: i8 = time_parts[2].parse().ok()?;
    civil_to_posix(year, month, day, hour, minute, second, 0, 0)
}

/// Build the assumption line for an ASN.1 reading, surfacing the assumed-UTC
/// caveat when the string carried no explicit `Z`/offset (it may be local time).
fn asn1_assumption(kind: &str, had_tz: bool) -> String {
    if had_tz {
        format!("parsed as ASN.1 {kind}")
    } else {
        format!(
            "parsed as ASN.1 {kind}; NO timezone designator — assumed UTC, but may be local time"
        )
    }
}

/// Build a candidate for a self-describing string form. Such inputs are
/// unambiguous once parsed, so they carry a `self_describing` component.
fn string_candidate(
    format_id: &'static str,
    label: &'static str,
    citation: &'static str,
    instant: PosixNs,
    assumption: &str,
) -> Candidate {
    Candidate {
        format_id,
        label,
        citation,
        instant,
        rendered: instant.to_rfc3339(),
        score: 1.0,
        components: vec![
            ("representable", 1.0),
            ("self_describing", 1.0),
            ("not_sentinel", 1.0),
        ],
        assumptions: vec![assumption.to_string()],
        sentinel: false,
    }
}

/// Split a trailing timezone designator (`Z`, `±HHMM`, or none → assume UTC) off
/// a numeric ASN.1 time string, returning the digit core and the offset seconds.
/// Returns `None` when a present offset is malformed or out of range (e.g.
/// `+1260`) — such input must not be silently normalised into a fabricated
/// instant. A `had_tz` flag distinguishes an explicit `Z`/offset from an
/// assumed-UTC fallback so the caller can surface that assumption.
fn split_tz(s: &str) -> Option<(String, i64, bool)> {
    if let Some(core) = s.strip_suffix('Z').or_else(|| s.strip_suffix('z')) {
        return Some((core.to_string(), 0, true));
    }
    // A numeric tz suffix (+HHMM / -HHMM) is 5 ASCII bytes. Guard the byte-index
    // split against a multi-byte UTF-8 char straddling that boundary (split_at
    // panics on a non-char-boundary; a non-ASCII tail is not a valid offset).
    if s.len() >= 5 && s.is_char_boundary(s.len() - 5) {
        let (core, suf) = s.split_at(s.len() - 5);
        let b = suf.as_bytes();
        if (b[0] == b'+' || b[0] == b'-') && suf[1..].bytes().all(|c| c.is_ascii_digit()) {
            let hh: i64 = suf[1..3].parse().ok()?;
            let mm: i64 = suf[3..5].parse().ok()?;
            if hh > 23 || mm > 59 {
                return None; // out-of-range offset — reject, do not fabricate
            }
            let mag = hh * 3600 + mm * 60;
            return Some((
                core.to_string(),
                if b[0] == b'-' { -mag } else { mag },
                true,
            ));
        }
    }
    Some((s.to_string(), 0, false))
}

/// Build an instant from civil fields at a fixed UTC offset (panic-free).
// A flat civil-fields helper: the broken-out arguments mirror the parsed digit
// groups one-to-one, which is clearer here than an intermediate struct.
#[allow(clippy::too_many_arguments)]
fn civil_to_posix(
    y: i16,
    mo: i8,
    d: i8,
    h: i8,
    mi: i8,
    s: i8,
    subsec_nanos: i32,
    offset_secs: i64,
) -> Option<PosixNs> {
    let dt = jiff::civil::DateTime::new(y, mo, d, h, mi, s, subsec_nanos).ok()?;
    let off = jiff::tz::Offset::from_seconds(i32::try_from(offset_secs).ok()?).ok()?;
    let zoned = dt.to_zoned(jiff::tz::TimeZone::fixed(off)).ok()?;
    Some(PosixNs(zoned.timestamp().as_nanosecond()))
}

/// Convert an ASN.1 fractional-second digit string to nanoseconds (the first 9
/// digits, right-padded; further digits truncated).
fn frac_to_nanos(frac: &str) -> i32 {
    let mut t: String = frac.chars().take(9).collect();
    while t.len() < 9 {
        t.push('0');
    }
    t.parse().unwrap_or(0)
}

/// Shared ASN.1 time parser (ITU-T X.680). `year_digits` is 4 (GeneralizedTime)
/// or 2 (UTCTime, RFC 5280 pivot). Accepts omitted minutes/seconds and, when
/// seconds are present, a fractional second (`.fff` / `,fff`).
fn parse_asn1(s: &str, year_digits: usize) -> Option<(PosixNs, bool)> {
    let (core, off, had_tz) = split_tz(s)?;
    let (digits, frac) = match core.split_once(['.', ',']) {
        Some((d, f)) => (d.to_string(), Some(f.to_string())),
        None => (core, None),
    };
    if !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let year = if year_digits == 4 {
        digits.get(0..4)?.parse().ok()?
    } else {
        let yy: i16 = digits.get(0..2)?.parse().ok()?;
        if yy < 50 {
            2000 + yy
        } else {
            1900 + yy
        }
    };
    let base = year_digits;
    let len = digits.len();
    // Required: month, day, hour. Optional: minute, second.
    let mo = digits.get(base..base + 2)?.parse().ok()?;
    let d = digits.get(base + 2..base + 4)?.parse().ok()?;
    let h = digits.get(base + 4..base + 6)?.parse().ok()?;
    let sec_present = len == base + 10;
    let min_present = sec_present || len == base + 8;
    if len != base + 6 && len != base + 8 && len != base + 10 {
        return None;
    }
    let mi = if min_present {
        digits.get(base + 6..base + 8)?.parse().ok()?
    } else {
        0
    };
    let s = if sec_present {
        digits.get(base + 8..base + 10)?.parse().ok()?
    } else {
        0
    };
    // A fraction is only meaningful when seconds are present.
    let subsec = match frac {
        Some(f) if sec_present && !f.is_empty() && f.bytes().all(|c| c.is_ascii_digit()) => {
            frac_to_nanos(&f)
        }
        Some(_) => return None,
        None => 0,
    };
    let instant = civil_to_posix(year, mo, d, h, mi, s, subsec, off)?;
    Some((instant, had_tz))
}

/// ASN.1 GeneralizedTime (4-digit year).
fn parse_asn1_generalizedtime(s: &str) -> Option<(PosixNs, bool)> {
    parse_asn1(s, 4)
}

/// ASN.1 UTCTime (2-digit year; RFC 5280 pivot).
fn parse_asn1_utctime(s: &str) -> Option<(PosixNs, bool)> {
    parse_asn1(s, 2)
}
