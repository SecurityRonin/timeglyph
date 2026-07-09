//! Auto-detection: identify an unknown value by reporting EVERY plausible
//! interpretation, **scored, with stated assumptions** — never "the detected
//! format." A single integer is usually underdetermined: a 64-bit value can be a
//! plausible Unix-s, Java-ms, Chrome-µs, FILETIME, .NET-ticks and Cocoa-s date
//! all at once. Presenting one as *the* answer would fabricate certainty, which a
//! forensic tool must never do (epistemics: "consistent with", not a verdict).
//!
//! Scoring is a named component set (ADR 0005): representable validity,
//! plausibility-window membership, granularity match, magnitude fit, an
//! epoch-distance (magnitude/recency) prior, and a sentinel guard are always
//! emitted; byte-width match, endian match,
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

/// Identify a value across every interpretation family — the one-call entry
/// point for library consumers (language bindings, the WASM playground,
/// downstream integrations). Merges integer, float, and self-describing-string
/// readings and returns them ranked by score (descending); empty if nothing
/// decodes. Renderings are UTC — re-render an individual [`Candidate::instant`]
/// via [`PosixNs::render`](crate::PosixNs::render) for another zone. Mirrors the
/// `identify` CLI's auto mode (no artifact hint; use
/// [`interpret_int_with_context`] for that).
#[must_use]
pub fn identify(value: &str) -> Vec<Candidate> {
    let s = value.trim();
    let mut cands = Vec::new();
    if let Ok(v) = s.parse::<i64>() {
        cands.extend(interpret_int(v));
    } else if let Ok(v) = s.parse::<f64>() {
        cands.extend(interpret_float(v));
    }
    cands.extend(interpret_string(s));
    cands.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    cands
}

/// [`identify`] serialized to a JSON array of readings — the JSON-in/JSON-out
/// entry point for the WASM playground, a C ABI, and language bindings that
/// prefer a string boundary over the typed [`Candidate`]. Never fails: an
/// undecodable value yields `"[]"`.
#[must_use]
pub fn identify_json(value: &str) -> String {
    serde_json::to_string(&identify(value))
        // cov:unreachable: Candidate serialization is infallible (no fallible
        // types); the fallback is a kept defensive arm, never taken.
        .unwrap_or_else(|_| "[]".to_owned())
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
            // Break exact score-ties by prevalence (the likelier format first),
            // then by id for determinism.
            .then_with(|| {
                prevalence(b.format_id)
                    .partial_cmp(&prevalence(a.format_id))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.format_id.cmp(b.format_id))
    });
    out
}

/// Auto-identify a floating-point value: report every `LinearFloat`-strategy
/// reading — Cocoa / CFAbsoluteTime, OLE automation date, Julian / Modified-Julian
/// day numbers — scored like [`interpret_int`], ranked, never one verdict. A
/// fractional literal cannot be an integer epoch, so the integer decoders are
/// structurally inapplicable and omitted; the fraction the integer path would
/// truncate is preserved to nanosecond resolution.
#[must_use]
pub fn interpret_float(value: f64) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    for f in FORMATS {
        // Only float strategies decode a double; `decode_float` rejects the rest,
        // and an out-of-civil-range instant is dropped inside build_candidate_float.
        if let Some(c) = build_candidate_float(f, value) {
            out.push(c);
        }
    }
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                prevalence(b.format_id)
                    .partial_cmp(&prevalence(a.format_id))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.format_id.cmp(b.format_id))
    });
    out
}

/// Build a scored candidate for one format + float value, or `None` if the value
/// is not float-decodable under it (non-`LinearFloat` strategy, non-finite, or
/// out of range) or renders outside the civil range.
fn build_candidate_float(f: &Format, value: f64) -> Option<Candidate> {
    let instant = f.decode_float(value).ok()?;
    let rendered = instant.to_rfc3339()?;
    let components = score_components_float(f, instant);
    let score = overall_score(&components);
    let assumptions = assumptions(f);
    Some(Candidate {
        format_id: f.id,
        label: f.label,
        citation: f.citation,
        instant,
        rendered: Some(rendered),
        score,
        components,
        // Sentinels (0 / -1 / i64::MAX) are integer markers; a float carrying a
        // fraction is a real reading, not an 'unset' marker.
        assumptions,
        sentinel: false,
    })
}

/// The base plausibility components for a float reading, mirroring the
/// zero-context set of [`score_components`]. A finite fractional value uses the
/// unit's full sub-second precision, so `granularity_match` is 1.0; the on-disk
/// width/endian and neighbour components have no analogue for a decimal literal.
fn score_components_float(f: &Format, instant: PosixNs) -> Vec<(&'static str, f64)> {
    let in_window = f64::from(u8::from(
        instant.0 >= f.plausible.0 && instant.0 < f.plausible.1,
    ));
    vec![
        ("representable", 1.0),
        ("in_window", in_window),
        ("granularity_match", 1.0),
        ("magnitude_fit", magnitude_fit(f.strategy, instant)),
        ("epoch_distance", epoch_distance(f.strategy, instant)),
        ("prevalence", prevalence(f.id)),
        ("not_sentinel", 1.0),
    ]
}

/// Editorial PREVALENCE prior (ADR-0005 successor), used ONLY as a score-neutral
/// tie-break: how commonly this format is the TRUE source of a timestamp in real
/// evidence — a *documented prior, NOT a measurement*. It demotes ONLY the
/// genuinely-rare long tail (AD `active`, legacy Mac `excel1904`, obscure packed
/// hardware-clock/regional formats) so that when such a value ALSO reads as a
/// far more common format sharing its window, the common reading wins the tie
/// (e.g. filetime over `active`, OLE over `excel1904`). Everything mainstream —
/// Unix/FILETIME/WebKit/Cocoa, DBs, .NET, browsers, social IDs — stays at 1.0,
/// so the prior NEVER pushes a mainstream true reading out of the top-3, and
/// genuinely-ambiguous ties among common formats (Twitter vs Discord, SQL Server
/// vs PostgreSQL) stay tied — the tool reports underdetermination honestly. As a
/// weight-0, always-emitted component it is visible and auditable but does not
/// distort the score; it only orders exact score-ties, and never hides a reading.
fn prevalence(id: &str) -> f64 {
    match id {
        "active" | "excel1904" | "sony" | "dttm" | "bitdate" | "bitdec" | "bcd" | "moto"
        | "symantec" | "dvr" | "ns40" | "ns40le" | "logtime" | "semioctet" | "gsm" | "nokiale"
        | "mjd" | "dhcp6" | "hfs" | "gmsgid" => 0.5,
        _ => 1.0,
    }
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
    // A Windows FILETIME whose sub-second 100 ns field is exactly zero (instant
    // lands on a clean UTC second) is consistent with a SetFileTime-style
    // programmatic set: the file API produces whole-second precision when called
    // directly, whereas a naturally-recorded file time almost never falls on an
    // exact second boundary. A soft forensic signal only — framed "consistent
    // with", never a verdict (ADR 0005). Scoped to `filetime` ONLY: AD `active`
    // shares the encoding but has many legitimately whole-second attributes, so
    // annotating it would be a false positive.
    if f.id == "filetime" && instant.0 % 1_000_000_000 == 0 {
        assumptions.push(
            "sub-second field is exactly zero — consistent with a SetFileTime-style \
             manipulation, not a naturally-recorded instant"
                .to_string(),
        );
    }
    // A value at/near a 32-bit field boundary is evidence-relevant for a whole-
    // seconds field (the time_t class): 2^31 is the signed max, 2^32 the unsigned
    // max. Derived from the value + field width, so it holds for any seconds-unit
    // LinearInt regardless of epoch — the Unix Y2038 boundary is just its most
    // famous instance. Framed as a possibility (ADR 0005), never a verdict.
    if matches!(
        f.strategy,
        Strategy::LinearInt {
            unit: Unit::Seconds,
            ..
        }
    ) {
        const SIGNED_MAX: i64 = i32::MAX as i64; // 2_147_483_647
        const UNSIGNED_MAX: i64 = u32::MAX as i64; // 4_294_967_295
        const NEAR: i64 = 63_072_000; // ~2 years of seconds
        if (SIGNED_MAX + 1..=UNSIGNED_MAX).contains(&value) {
            assumptions.push(
                "stored value exceeds the signed 32-bit range (2^31) but fits an unsigned \
                 32-bit field — consistent with an unsigned 32-bit time field, or a value \
                 past a signed field's rollover"
                    .to_string(),
            );
        } else if (SIGNED_MAX - NEAR..=SIGNED_MAX).contains(&value) {
            assumptions.push(
                "stored value is within ~2 years of the signed 32-bit maximum (2^31-1) — \
                 consistent with approaching the representable limit of a signed 32-bit field \
                 (the Unix Y2038 boundary for a 1970-epoch field)"
                    .to_string(),
            );
        }
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
    let epoch_dist = epoch_distance(f.strategy, instant);
    let not_sentinel = f64::from(u8::from(sentinel_reason(value).is_none()));
    let mut components = vec![
        ("representable", representable),
        ("in_window", in_window),
        ("granularity_match", granularity),
        ("magnitude_fit", magnitude),
        ("epoch_distance", epoch_dist),
        ("prevalence", prevalence(f.id)),
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

/// MAGNITUDE/RECENCY prior: how far the decoded instant sits *past the format's
/// own epoch*, relative to the two-year "well into its era" ramp — `0.0` at the
/// epoch, `1.0` two years or more past it.
///
/// Forensic justification (independent of any corpus): a real timestamp's
/// magnitude places the decoded instant WELL INTO the format's plausible era,
/// not hugging the format's epoch. A value that lands microseconds/minutes/hours
/// after the format's own epoch is orders of magnitude too small for that unit
/// to have been produced by a modern clock — weak evidence for that format. The
/// canonical case: a 13-digit Unix-millisecond value is ALSO in-window as
/// `iostime` (nanoseconds since 2001), where it decodes to 2001-01-01 plus a few
/// minutes; a genuine ns-since-2001 value for a modern date is ~17-19 digits, so
/// the tiny reading is implausible for that unit and this prior demotes it.
///
/// This subsumes [`magnitude_fit`]'s epoch ramp for `Embedded` IDs and extends
/// the same idea to the linear (`LinearInt`/`LinearFloat`) formats it previously
/// left at `1.0`. It is a LOW-WEIGHT prior (weight 1, like `granularity_match`):
/// it nudges the rank while the double-weighted `in_window`/`magnitude_fit`/
/// `not_sentinel` guards stay dominant. Tradeoff: it is NOT a filter — a genuine
/// early-epoch timestamp (e.g. a real instant an hour after a format's epoch)
/// still appears as a candidate, just ranked lower; the reading is never hidden.
/// `Packed` civil-field formats carry no linear epoch offset, so they score
/// `1.0` (the prior does not apply), exactly as [`magnitude_fit`] treats them.
fn epoch_distance(strategy: Strategy, instant: PosixNs) -> f64 {
    let epoch_ns = match strategy {
        Strategy::LinearInt { epoch_ns, .. }
        | Strategy::LinearFloat { epoch_ns, .. }
        | Strategy::Embedded { epoch_ns, .. } => epoch_ns,
        Strategy::Packed { .. } => return 1.0,
    };
    let past = instant.0 - epoch_ns;
    if past <= 0 {
        0.0
    } else {
        (past as f64 / TWO_YEARS_NS as f64).min(1.0)
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
        // Prevalence is a VISIBLE, auditable prior but score-NEUTRAL (weight 0):
        // it breaks exact score-ties in the sort, never distorts the score. As a
        // weighted component it demoted niche formats out of the top-3 (a net
        // loss); as a pure tie-break it lifts the likelier format to #1 with no
        // top-3 cost.
        "prevalence" => 0.0,
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
    // Cap on decoded byte length (untrusted-input guard; enforced after decode).
    const MAX_HEX_BYTES: usize = 64 * 1024;
    let clean: String = hex
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != ':')
        .collect();
    let clean = clean
        .strip_prefix("0x")
        .or_else(|| clean.strip_prefix("0X"))
        .unwrap_or(&clean);
    let bytes = hex::decode(clean).map_err(|_| ChronoError::OutOfRange {
        what: "hex (not valid hex bytes)",
        value: 0,
    })?;
    // Cap untrusted hex so a pathological blob can't drive an unbounded byte vec
    // + per-window candidate lists. Fail loud, naming the offending size.
    if bytes.len() > MAX_HEX_BYTES {
        return Err(ChronoError::OutOfRange {
            what: "hex input exceeds 64 KiB",
            value: bytes.len() as i128,
        });
    }
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
        parse: parse_google_ei,
        id: "google_ei",
        label: "Google ei= URL parameter (Unix seconds in the first 4 bytes)",
        spec: "Google ei URL param (urlsafe base64; first 4 bytes little-endian Unix seconds)",
        note: "parsed as a Google ei= URL parameter — the leading 4 bytes are little-endian Unix seconds",
    },
    StringFormat {
        parse: parse_clf,
        id: "clf",
        label: "Apache/nginx common-log-format date",
        spec: "Apache mod_log_config (CLF): dd/Mon/YYYY:HH:MM:SS ±HHMM",
        note: "parsed as an Apache/nginx CLF date-time (offset normalised to UTC)",
    },
    StringFormat {
        parse: parse_pdf_date,
        id: "pdf_date",
        label: "PDF metadata date (D:YYYYMMDDHHmmSS)",
        spec: "ISO 32000-1 §7.9.4 (PDF date string)",
        note: "parsed as a PDF metadata date (offset normalised to UTC)",
    },
    StringFormat {
        parse: parse_dmtf_cim,
        id: "dmtf_cim",
        label: "DMTF/WMI CIM_DATETIME",
        spec: "DMTF DSP0004 (CIM_DATETIME): yyyymmddHHMMSS.mmmmmm±UUU",
        note: "parsed as a DMTF/WMI CIM datetime — UUU is the offset in minutes east of UTC",
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

/// Google's `ei=` URL parameter (urlsafe base64): its leading 4 decoded bytes are
/// a little-endian Unix-seconds count. Decoded ONLY when the `ei=` marker is
/// present (the format *is* a named URL parameter) — a bare base64-looking token
/// carries no structural signature, so requiring the marker keeps auto-detect
/// quiet instead of reading a timestamp out of any 6-char word. `None` if no
/// `ei=` marker, or the value is under 6 chars / not urlsafe base64.
fn parse_google_ei(s: &str) -> Option<PosixNs> {
    // The value after the `ei=` marker, up to the next query delimiter.
    let val = s.split("ei=").nth(1)?.split(['&', '#']).next()?;
    // 6 urlsafe-base64 chars = 36 bits; the first 4 bytes are the top 32.
    let mut acc: u64 = 0;
    for ch in val.get(..6)?.bytes() {
        acc = (acc << 6) | u64::from(urlsafe_b64_val(ch)?);
    }
    let bytes = ((acc >> 4) as u32).to_be_bytes();
    let secs = i128::from(u32::from_le_bytes(bytes));
    Some(PosixNs(secs.checked_mul(Unit::Seconds.nanos())?))
}

/// One urlsafe-base64 character (`A–Z a–z 0–9 - _`) to its 6-bit value; `None`
/// for padding or any other byte.
fn urlsafe_b64_val(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
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

/// Three-letter English month abbreviation (`Jan`..`Dec`, case-insensitive) to
/// its 1-based number. `None` for anything else.
fn month_abbr(m: &str) -> Option<i8> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let m = m.to_ascii_lowercase();
    MONTHS
        .iter()
        .position(|x| *x == m)
        .map(|i| i8::try_from(i + 1).unwrap_or(1))
}

/// Parse a `±HHMM` numeric offset (e.g. `-0700`) into seconds east of UTC.
fn numeric_offset_secs(tz: &str) -> Option<i64> {
    let sign = match tz.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits = &tz[1..];
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let h: i64 = digits.get(0..2)?.parse().ok()?;
    let mi: i64 = digits.get(2..4)?.parse().ok()?;
    Some(sign * (h * 3600 + mi * 60))
}

/// Apache/nginx common-log-format date: `dd/Mon/YYYY:HH:MM:SS ±HHMM`, optionally
/// wrapped in `[...]` as it appears in a log line. `None` unless the whole shape
/// (including the numeric offset) matches.
fn parse_clf(s: &str) -> Option<PosixNs> {
    let s = s
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    let (date_time, tz) = s.rsplit_once(' ')?;
    let (date, time) = date_time.split_once(':')?;
    let mut d = date.split('/');
    let day: i8 = d.next()?.parse().ok()?;
    let mon = month_abbr(d.next()?)?;
    let year: i16 = d.next()?.parse().ok()?;
    if d.next().is_some() {
        return None;
    }
    let mut t = time.split(':');
    let (h, mi, sec) = (
        t.next()?.parse().ok()?,
        t.next()?.parse().ok()?,
        t.next()?.parse().ok()?,
    );
    if t.next().is_some() {
        return None;
    }
    civil_to_posix(year, mon, day, h, mi, sec, 0, numeric_offset_secs(tz)?)
}

/// PDF metadata date (ISO 32000-1 §7.9.4): `D:YYYYMMDDHHmmSS` with an optional
/// `±HH'mm'`/`Z` offset. Trailing civil fields may be omitted (default to the
/// start of the period). `None` unless the `D:` marker and a 4-digit year are
/// present.
fn parse_pdf_date(s: &str) -> Option<PosixNs> {
    let body = s.trim().strip_prefix("D:")?;
    let digits: String = body.chars().take_while(char::is_ascii_digit).collect();
    let year: i16 = digits.get(0..4)?.parse().ok()?;
    let f = |r: std::ops::Range<usize>, dflt: i8| -> i8 {
        digits.get(r).and_then(|x| x.parse().ok()).unwrap_or(dflt)
    };
    let (mo, d) = (f(4..6, 1), f(6..8, 1));
    let (h, mi, sec) = (f(8..10, 0), f(10..12, 0), f(12..14, 0));
    // Offset: the remainder after the digits — `Z`, empty, or `±HH'mm'`.
    let rest = &body[digits.len()..];
    let offset = if rest.is_empty() || rest.starts_with('Z') {
        0
    } else {
        let cleaned: String = rest.chars().filter(|c| *c != '\'').take(5).collect();
        numeric_offset_secs(&cleaned)?
    };
    civil_to_posix(year, mo, d, h, mi, sec, 0, offset)
}

/// DMTF/WMI CIM_DATETIME (DSP0004): `yyyymmddHHMMSS.mmmmmm±UUU`, where `UUU` is
/// the offset in whole minutes east of UTC (or `***` for "unknown", treated as
/// UTC). `None` unless this exact shape matches — distinctive enough to avoid
/// colliding with bare civil integers.
fn parse_dmtf_cim(s: &str) -> Option<PosixNs> {
    let s = s.trim();
    let (main, tz) = s.split_once(['+', '-'])?;
    let sign: i64 = if s.as_bytes()[main.len()] == b'-' {
        -1
    } else {
        1
    };
    let (date, frac) = main.split_once('.')?;
    if date.len() != 14 || !date.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if frac.len() != 6 || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let g = |r: std::ops::Range<usize>| -> Option<i64> { date.get(r)?.parse().ok() };
    let year = i16::try_from(g(0..4)?).ok()?;
    let offset = if tz == "***" {
        0
    } else {
        if tz.len() != 3 || !tz.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        sign * tz.parse::<i64>().ok()? * 60
    };
    civil_to_posix(
        year,
        i8::try_from(g(4..6)?).ok()?,
        i8::try_from(g(6..8)?).ok()?,
        i8::try_from(g(8..10)?).ok()?,
        i8::try_from(g(10..12)?).ok()?,
        i8::try_from(g(12..14)?).ok()?,
        frac_to_nanos(frac),
        offset,
    )
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
