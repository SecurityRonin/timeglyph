//! `timeglyph` CLI — a thin Humble-Object shell over the library engine.
//!
//! Subcommands: `identify` (the safe default; ranked candidates — a raw value is
//! usually underdetermined), `decode <format> <value>`, `encode <format> <dt>`,
//! `scan <text>`, `list`. A bare value is a back-compat shortcut for `identify`;
//! `--as auto|int|hex|string` (default `auto`) forces one interpretation family.
//! Exit codes are pipeline-safe: `0` ok, `2` ambiguous or a sentinel (review
//! needed), `1` error.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
#[cfg(feature = "csv")]
use timeglyph::csv_enrich::{Conversion, EnrichOptions};
use timeglyph::interpret::{self, Candidate};
use timeglyph::{DateStyle, RenderZone};

const EXIT_OK: u8 = 0;
const EXIT_ERR: u8 = 1;
const EXIT_AMBIGUOUS: u8 = 2;

/// CLI selector for the engine's [`DateStyle`] display styles.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum FormatArg {
    /// RFC 3339 / ISO 8601 (the default), e.g. `2020-01-01T00:00:00Z`.
    Iso8601,
    /// Space-separated with a zone abbreviation, e.g. `2020-01-01 00:00:00 UTC`.
    Space,
    /// RFC 2822, e.g. `Wed, 01 Jan 2020 00:00:00 +0000`.
    Rfc2822,
    /// US 12-hour clock, e.g. `01/01/2020 12:00:00 AM UTC`.
    Us,
}

impl From<FormatArg> for DateStyle {
    fn from(f: FormatArg) -> Self {
        match f {
            FormatArg::Iso8601 => DateStyle::Iso8601,
            FormatArg::Space => DateStyle::SpaceSeparated,
            FormatArg::Rfc2822 => DateStyle::Rfc2822,
            FormatArg::Us => DateStyle::UsStyle,
        }
    }
}

/// Which interpretation family `identify` (and the bare-value shortcut) applies.
/// `Auto` detects and merges; the others force one family and skip the merge.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum AsArg {
    /// Detect and merge every family the value could belong to (the default).
    Auto,
    /// Integer epoch formats only (the value must parse as an `i64`).
    Int,
    /// Raw hex byte layouts only (LE/BE widths + packed on-disk forms).
    Hex,
    /// Self-describing string forms only (ISO 8601 / RFC 3339 / RFC 2822 /
    /// ASN.1 / ULID / UUID / `ObjectId` / EXIF …).
    String,
}

#[derive(Parser, Debug)]
#[command(name = "timeglyph", version, about = "Forensic timestamp decipherment")]
struct Cli {
    /// A value to IDENTIFY (back-compat shortcut for `identify <value>`).
    value: Option<String>,
    /// Emit JSON (with the bare-value shortcut).
    #[arg(long)]
    json: bool,
    /// Render dates in this timezone instead of UTC. Accepts `UTC`, a fixed
    /// offset (`+08:00`, `-0500`), or an IANA name (`America/New_York`). The
    /// instant is unchanged — only the displayed offset differs.
    #[arg(long, global = true, value_name = "ZONE")]
    tz: Option<String>,
    /// An artifact/source hint (e.g. `"chrome history"`, `"ntfs mft"`) that nudges
    /// identify readings whose format family matches it. A hint never hides a
    /// reading — it only adjusts the rank.
    #[arg(long, global = true, value_name = "HINT")]
    artifact: Option<String>,
    /// Force one interpretation family (default: auto — detect and merge). int =
    /// integer epoch formats; hex = raw hex byte layouts; string = self-describing
    /// string forms (ISO 8601 / RFC 3339 / RFC 2822 / ASN.1 / ULID / UUID /
    /// ObjectId / EXIF).
    #[arg(long = "as", value_enum, default_value_t = AsArg::Auto, global = true)]
    as_mode: AsArg,
    /// Datetime display style for rendered output (identify/decode/scan). The
    /// instant and zone are unchanged — only the textual shape differs.
    #[arg(long = "format", id = "date_format", global = true, value_enum, default_value_t = FormatArg::Iso8601)]
    format: FormatArg,
    /// Show at most N readings (identify). Default: all.
    #[arg(long, global = true, value_name = "N")]
    top: Option<usize>,
    /// Drop readings scoring below S (0.0–1.0) (identify). Default: keep all.
    #[arg(long = "min-score", global = true, value_name = "S")]
    min_score: Option<f64>,
    /// Treat the top two readings as ambiguous (exit 2) when their scores differ
    /// by at most GAP. Default: exact tie only.
    #[arg(
        long = "ambiguity-gap",
        global = true,
        value_name = "GAP",
        default_value_t = 1e-9
    )]
    ambiguity_gap: f64,
    /// Wrap `--json` identify output in a reproducible provenance envelope:
    /// engine name/version, a registry digest, the verbatim input, and each
    /// reading's citation — traceable back to the exact method version.
    #[arg(long, global = true)]
    provenance: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

/// How the ranked reading list is trimmed and when it counts as ambiguous —
/// the `--top` / `--min-score` / `--ambiguity-gap` knobs, bundled so they thread
/// through the identify/hex paths as one value.
#[derive(Clone, Copy)]
struct RankOpts {
    top: Option<usize>,
    min_score: Option<f64>,
    gap: f64,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Identify a value across all formats (ranked candidates, never one verdict).
    #[command(visible_alias = "id")]
    Identify {
        /// The value to identify.
        value: String,
        /// Emit JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Decode a value under ONE known format id (see `list`).
    Decode {
        /// Format id (e.g. `filetime`, `unix`, `gps` with the `leap` feature).
        format: String,
        /// The value (decimal integer, or a float for float formats).
        value: String,
    },
    /// Encode a datetime (ISO 8601 / RFC 3339 / ASN.1) into a format id.
    Encode {
        /// Target format id.
        format: String,
        /// The datetime string to encode.
        datetime: String,
    },
    /// Scan arbitrary text for timestamp candidates and decode each — the bulk
    /// counterpart to `identify`; always auto-detects integer, string, and
    /// raw-hex candidates. Reads stdin when no text is given.
    Scan {
        /// Text to scan; if omitted, read from stdin.
        text: Option<String>,
        /// Minimum consecutive digits for a numeric run to be considered.
        #[arg(long, default_value_t = 8)]
        min_digits: usize,
        /// Show at most N readings per value (opt-in brevity). Default: show ALL
        /// readings, ranked — likelihood orders them, it never filters.
        #[arg(long, value_name = "N")]
        top: Option<usize>,
        /// Emit JSONL (one JSON object per value) instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Carve raw bytes (hex) for timestamps at every offset — a bounded blob (a
    /// config, a record, a selection), window + score-thresholded.
    Carve {
        /// Hex bytes to carve (e.g. `aabbcc…`); omit or `-` to read hex from stdin.
        hex: Option<String>,
        /// Minimum score to report a hit.
        #[arg(long, default_value_t = 0.5)]
        min_score: f64,
        /// Plausibility window lower bound, a year (e.g. 2000).
        #[arg(long)]
        from: Option<i16>,
        /// Plausibility window upper bound, a year (e.g. 2030).
        #[arg(long)]
        to: Option<i16>,
        /// Emit JSONL (one hit per line) instead of text.
        #[arg(long)]
        json: bool,
        /// Emit ImHex bookmarks JSON.
        #[arg(long)]
        imhex: bool,
    },
    /// Explain a format: a spec card (epoch, tick unit, tz/leap, valid range,
    /// known sentinels, citation) generated from the registry.
    Explain {
        /// Format id (see `list`).
        format: String,
    },
    /// Run as an MCP (Model Context Protocol) stdio server — expose identify /
    /// decode / explain as tools for an LLM-driven DFIR workflow. Reads JSON-RPC
    /// from stdin, replies on stdout.
    Mcp,
    /// List every registered format with its citation.
    List,
    /// Enrich a CSV: add a human-readable column for each timestamp column.
    #[cfg(feature = "csv")]
    Csv {
        /// CSV file path, or `-` for stdin.
        path: String,
        /// Explicit conversion `COLUMN:FORMAT` (repeatable, e.g. `created:filetime`).
        #[arg(long = "convert", value_name = "COL:FMT")]
        convert: Vec<String>,
        /// Auto-detect numeric timestamp columns (the default when no --convert).
        #[arg(long)]
        auto: bool,
        /// Replace the source column in place instead of adding one to its right.
        #[arg(long)]
        replace: bool,
        /// Write output here instead of stdout.
        #[arg(short, long, value_name = "FILE")]
        output: Option<String>,
    },
    /// Render an instant in the Chinese lunisolar calendar + 干支 four pillars.
    /// Requires `--tz` (the conversion is meridian-relative).
    #[cfg(feature = "lunisolar")]
    Lunisolar {
        /// The instant: an ISO 8601 / RFC 3339 datetime, or a Unix-seconds integer.
        datetime: String,
        /// Longitude °E for the hour pillar's true-solar-time correction (optional).
        #[arg(long, allow_hyphen_values = true)]
        longitude: Option<f64>,
    },
    /// Forensic calendar: per-day UTC offset & DST folds/gaps, leap-second days,
    /// ISO week / day-of-year / JDN / GPS week / Unix, alt-calendar overlays,
    /// moon phase, and timestamp-format epoch markers. Honors `--tz`.
    Cal {
        /// `YYYY` (year), `YYYY-MM` (month), or `YYYY-MM-DD` (single-day detail);
        /// omitted = the current month.
        when: Option<String>,
        /// First day of the week: `monday` (ISO, default) or `sunday`.
        #[arg(long, value_name = "DAY", default_value = "monday")]
        week_start: String,
        /// Southern hemisphere: flip the season strip (December solstice = summer).
        #[arg(long)]
        south: bool,
        /// Emit the calendar as JSON (faithful, one record per day).
        #[arg(long)]
        json: bool,
    },
}

/// Install a tracing subscriber gated by `RUST_LOG` (silent by default), writing
/// to stderr so stdout stays clean for pipelines. `RUST_LOG=timeglyph=debug`
/// traces a decode/scan step by step.
fn init_tracing() {
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::EnvFilter;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        // Log each instrumented span when it closes (its fields + duration), so
        // the decode/scan call flow is visible even without explicit events.
        .with_span_events(FmtSpan::CLOSE)
        .try_init();
}

fn main() -> ExitCode {
    init_tracing();
    let cli = Cli::parse();
    // Resolve the output zone once, up front: a bad --tz must fail loudly before
    // any rendering, never silently fall back to UTC.
    let zone = match RenderZone::parse(cli.tz.as_deref().unwrap_or("")) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(EXIT_ERR);
        }
    };
    let style: DateStyle = cli.format.into();
    let mode = cli.as_mode;
    let opts = RankOpts {
        top: cli.top,
        min_score: cli.min_score,
        gap: cli.ambiguity_gap,
    };
    // `--as` and `--artifact` only affect `identify` (the default / bare value);
    // every other command sets its own interpretation, so a flag passed there is
    // ignored. Reject it loudly rather than silently doing nothing.
    let is_identify = matches!(cli.command, None | Some(Commands::Identify { .. }));
    if !is_identify && mode != AsArg::Auto {
        eprintln!("error: --as applies only to `identify` / the bare value; other commands set their own interpretation");
        return ExitCode::from(EXIT_ERR);
    }
    if !is_identify && cli.artifact.is_some() {
        eprintln!("error: --artifact applies only to `identify` / the bare value");
        return ExitCode::from(EXIT_ERR);
    }
    let code = match cli.command {
        Some(Commands::Identify { value, json }) => run_identify(
            &value,
            json,
            cli.provenance,
            &zone,
            style,
            cli.artifact.as_deref(),
            mode,
            opts,
        ),
        Some(Commands::Decode { format, value }) => run_decode(&format, &value, &zone, style),
        Some(Commands::Encode { format, datetime }) => run_encode(&format, &datetime),
        Some(Commands::Scan {
            text,
            min_digits,
            top,
            json,
        }) => run_scan(text.as_deref(), min_digits, top, json, &zone, style),
        Some(Commands::Carve {
            hex,
            min_score,
            from,
            to,
            json,
            imhex,
        }) => run_carve(hex.as_deref(), min_score, from, to, json, imhex),
        Some(Commands::Explain { format }) => run_explain(&format),
        Some(Commands::Cal {
            when,
            week_start,
            south,
            json,
        }) => run_cal(when.as_deref(), &week_start, south, json, &zone),
        Some(Commands::Mcp) => run_mcp(),
        Some(Commands::List) => run_list(),
        #[cfg(feature = "csv")]
        Some(Commands::Csv {
            path,
            convert,
            auto,
            replace,
            output,
        }) => run_csv(&path, &convert, auto, replace, output.as_deref(), &zone),
        #[cfg(feature = "lunisolar")]
        Some(Commands::Lunisolar {
            datetime,
            longitude,
        }) => run_lunisolar(&datetime, longitude, &zone, cli.tz.is_some()),
        None => {
            if let Some(v) = cli.value {
                run_identify(
                    &v,
                    cli.json,
                    cli.provenance,
                    &zone,
                    style,
                    cli.artifact.as_deref(),
                    mode,
                    opts,
                )
            } else {
                eprintln!("error: give a VALUE or a subcommand (see --help)");
                EXIT_ERR
            }
        }
    };
    ExitCode::from(code)
}

/// Exit code reflecting interpretation confidence (pipeline safety): a sentinel
/// top reading or a tie for the top score is "review needed" (`2`); a clear
/// single winner is `0`; no readings is `2` (nothing confident).
fn ambiguity_code(cands: &[Candidate], gap: f64) -> u8 {
    let Some(top) = cands.first() else {
        return EXIT_AMBIGUOUS;
    };
    if top.sentinel {
        return EXIT_AMBIGUOUS;
    }
    // Top two within `gap` → ambiguous. Default gap is ~0 (exact tie only); a
    // wider --ambiguity-gap flags near-ties (e.g. 0.671 vs 0.670) too.
    if cands.len() >= 2 && (top.score - cands[1].score).abs() <= gap {
        return EXIT_AMBIGUOUS;
    }
    EXIT_OK
}

/// True when `s` is raw hex bytes: all `[0-9a-fA-F]`, even length, at least one
/// byte, AND at least one `a-f`/`A-F` letter — the letter requirement keeps a
/// pure-decimal integer (which is also all hex digits) on the integer path.
fn looks_like_hex_bytes(s: &str) -> bool {
    s.len() >= 2
        && s.len().is_multiple_of(2)
        && s.bytes().all(|b| b.is_ascii_hexdigit())
        && s.bytes().any(|b| b.is_ascii_alphabetic())
}

// The identify shell threads eight distinct, unrelated CLI inputs (value, two
// output flags, zone, style, artifact hint, family selector, rank knobs); they
// don't form a natural bundle beyond the existing `RankOpts`.
#[allow(clippy::too_many_arguments)]
fn run_identify(
    input: &str,
    json: bool,
    provenance: bool,
    zone: &RenderZone,
    style: DateStyle,
    artifact: Option<&str>,
    mode: AsArg,
    opts: RankOpts,
) -> u8 {
    let s = input.trim();
    // Forced families delegate to their own handlers (hex/string keep their own
    // print + exit semantics); auto and int share the merge/rank tail below.
    match mode {
        AsArg::Hex => return run_hex(s, zone, style, opts.gap),
        AsArg::String => return run_string(s, zone, style),
        AsArg::Auto | AsArg::Int => {}
    }
    // Auto also auto-detects hex: a `0x`/`0X` prefix or a-f letters mean the value
    // is raw bytes, not a decimal integer — those win over string-looking digits.
    if mode == AsArg::Auto
        && (s.starts_with("0x") || s.starts_with("0X") || looks_like_hex_bytes(s))
    {
        return run_hex(s, zone, style, opts.gap);
    }
    let ctx = interpret::InterpretContext {
        artifact,
        ..Default::default()
    };
    // Merge every family the input could belong to: numeric readings when it
    // parses as an integer, plus (auto only) string readings (ISO 8601 / ASN.1 /
    // …). Forcing `int` builds numeric readings alone; a non-integer then yields
    // no candidates and the empty-check below fails loudly, which is correct.
    let mut cands = Vec::new();
    if let Ok(v) = s.parse::<i64>() {
        cands.extend(interpret::interpret_int_with_context(v, &ctx));
    } else if mode == AsArg::Auto {
        // Not an integer: a fractional literal (e.g. Cocoa/CFAbsoluteTime
        // `606940977.71577`) can only be a float epoch, so run the LinearFloat
        // decoders and keep the sub-second fraction. `--as int` skips this and
        // fails loudly below, as an integer selector should.
        if let Ok(v) = s.parse::<f64>() {
            cands.extend(interpret::interpret_float(v));
        }
    }
    if mode == AsArg::Auto {
        cands.extend(interpret::interpret_string(s));
    }
    cands.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    // Re-render each `rendered` field in the requested zone. The canonical
    // `instant` (nanoseconds) stays the absolute anchor; only the human-facing
    // string changes — serialized directly (serde_json's intermediate Value
    // can't hold the i128 instant; the Serializer can).
    render_candidates_in_zone(&mut cands, zone, style);
    if cands.is_empty() {
        let tried = if mode == AsArg::Int {
            "an integer (--as int)"
        } else {
            "an integer, hex, or datetime string"
        };
        eprintln!("error: could not interpret {s:?} as {tried}");
        return EXIT_ERR;
    }
    // Presentation trim (after the parse-check, so filtering to empty is not
    // mistaken for "could not interpret"): drop low readings, then cap the list.
    if let Some(min) = opts.min_score {
        cands.retain(|c| c.score >= min);
    }
    if let Some(n) = opts.top {
        cands.truncate(n);
    }
    if json {
        let serialized = if provenance {
            serde_json::to_string_pretty(&ProvenanceEnvelope {
                schema_version: SCHEMA_VERSION,
                engine: "timeglyph",
                engine_version: env!("CARGO_PKG_VERSION"),
                registry_digest: timeglyph::registry_digest(),
                input: s,
                readings: &cands,
            })
        } else {
            serde_json::to_string_pretty(&cands)
        };
        match serialized {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("error: serializing candidates: {e}");
                return EXIT_ERR;
            }
        }
        return ambiguity_code(&cands, opts.gap);
    }
    println!(
        "# readings consistent with {s} (ranked; a raw value is usually \
         underdetermined — not a single verdict):"
    );
    print_candidates(&cands, zone, style);
    ambiguity_code(&cands, opts.gap)
}

/// Overwrite each candidate's `rendered` string with its instant rendered in
/// `zone` using `style` (leaving it untouched when the instant is out of civil
/// range).
/// Render one candidate honoring its format's tz semantics — the single source
/// of truth for both the JSON (`render_candidates_in_zone`) and text
/// (`print_candidates`) paths. Mirrors `scan::render_in_zone`: a naive
/// wall-clock (FAT/exFAT/DOS) is NEVER zone-shifted or offset-stamped, since
/// shifting it asserts an instant and a zone the data never carried; a
/// UTC-anchored value shifts into the display zone when representable there.
fn render_candidate(c: &Candidate, zone: &RenderZone, style: DateStyle) -> Option<String> {
    let tz = timeglyph::format(c.format_id).map_or(timeglyph::TzSemantics::Utc, |f| f.tz);
    match tz {
        timeglyph::TzSemantics::LocalNaive => {
            Some(timeglyph::datefmt::format_naive(c.instant, style))
        }
        _ => c
            .instant
            .render(zone)
            .map(|_| timeglyph::datefmt::format_instant(c.instant, zone, style))
            .or_else(|| c.rendered.clone()),
    }
}

fn render_candidates_in_zone(cands: &mut [Candidate], zone: &RenderZone, style: DateStyle) {
    for c in cands {
        c.rendered = render_candidate(c, zone, style);
        // Fold/gap: a LocalNaive wall-clock interpreted IN a named zone is
        // ambiguous at DST transitions. The rendering stays naive (the value
        // carried no zone); this only NOTES the fold/gap as a lead, framed "if …".
        if matches!(zone, RenderZone::Named(_))
            && timeglyph::format(c.format_id)
                .is_ok_and(|f| matches!(f.tz, timeglyph::TzSemantics::LocalNaive))
        {
            match timeglyph::resolve_local(c.instant, zone) {
                timeglyph::LocalResolution::Fold { earlier, later } => c.assumptions.push(format!(
                    "if this wall-clock is in the chosen zone: AMBIGUOUS (DST fall-back fold) \
                     — two instants: {} / {}",
                    earlier.to_rfc3339().unwrap_or_default(),
                    later.to_rfc3339().unwrap_or_default()
                )),
                timeglyph::LocalResolution::Gap => c.assumptions.push(
                    "if this wall-clock is in the chosen zone: NONEXISTENT (DST spring-forward \
                     gap) — a correctly-clocked device in this zone cannot have written it"
                        .to_string(),
                ),
                timeglyph::LocalResolution::Unique(_) => {}
            }
        }
    }
}

/// Composite (two-word) decode: `Some((result, render_id))` iff `format` is a
/// composite id — the parsed instant (or a parse/decode error) plus the
/// single-value format id to render/label it with. `None` falls through to the
/// single-value path.
fn decode_composite(
    format: &str,
    value: &str,
) -> Option<(Result<timeglyph::PosixNs, String>, &'static str)> {
    match format {
        "filetime_hilo" => Some((parse_filetime_hilo(value), "filetime")),
        "unix_sec_nsec" => Some((parse_unix_sec_nsec(value), "unix")),
        "elapsed_realtime" => Some((parse_relative(value, timeglyph::Unit::Millis), "unix")),
        "mach_continuous" => Some((parse_relative(value, timeglyph::Unit::Nanos), "unix")),
        "syslog" => Some((parse_syslog(value), "unix")),
        "vmsd" => Some((parse_vmsd(value), "unix")),
        "oracle_date" => Some((
            parse_wave2_bytes(value, timeglyph::compose::oracle_date),
            "unix",
        )),
        "iso9660" => Some((
            parse_wave2_bytes(value, timeglyph::compose::iso9660),
            "unix",
        )),
        "cp56time2a" => Some((
            parse_wave2_bytes(value, timeglyph::compose::cp56time2a),
            "unix",
        )),
        "udf" => Some((parse_wave2_udf(value), "unix")),
        "ext4_extra" => Some((parse_ext4_extra(value), "unix")),
        _ => None,
    }
}

/// Parse a hex string into exactly `N` bytes (whitespace/`:`/`_`/`0x` tolerated).
fn parse_hex_array<const N: usize>(value: &str) -> Result<[u8; N], String> {
    let clean: String = value
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '_')
        .collect();
    let clean = clean
        .strip_prefix("0x")
        .or_else(|| clean.strip_prefix("0X"))
        .unwrap_or(&clean);
    let bytes = hex::decode(clean).map_err(|_| format!("not valid hex: {value:?}"))?;
    <[u8; N]>::try_from(bytes.as_slice())
        .map_err(|_| format!("expected {N} bytes, got {}", bytes.len()))
}

/// Wire a 7-byte wave-2 decoder (Oracle DATE / ISO 9660 / CP56Time2a) from hex.
fn parse_wave2_bytes(
    value: &str,
    decode: fn([u8; 7]) -> Result<timeglyph::PosixNs, timeglyph::ChronoError>,
) -> Result<timeglyph::PosixNs, String> {
    decode(parse_hex_array::<7>(value)?).map_err(|e| e.to_string())
}

/// UDF is 12 bytes rather than 7.
fn parse_wave2_udf(value: &str) -> Result<timeglyph::PosixNs, String> {
    timeglyph::compose::udf(parse_hex_array::<12>(value)?).map_err(|e| e.to_string())
}

/// ext4 extended timestamp from `"<seconds>,<extra>"`.
fn parse_ext4_extra(value: &str) -> Result<timeglyph::PosixNs, String> {
    let (s, e) = value
        .split_once([',', ':'])
        .ok_or_else(|| format!("expected 'seconds,extra', got {value:?}"))?;
    let secs: i64 = s
        .trim()
        .parse()
        .map_err(|_| format!("not an i64 seconds: {s:?}"))?;
    let extra: u32 = e
        .trim()
        .parse()
        .map_err(|_| format!("not a u32 extra field: {e:?}"))?;
    Ok(timeglyph::compose::ext4_extra(secs, extra))
}

/// Parse a `"<createTimeHigh>,<createTimeLow>"` VMware `.vmsd` pair (decimal i32s).
fn parse_vmsd(value: &str) -> Result<timeglyph::PosixNs, String> {
    let (h, l) = value
        .split_once([',', ':'])
        .ok_or_else(|| format!("expected 'high,low', got {value:?}"))?;
    let high: i32 = h
        .trim()
        .parse()
        .map_err(|_| format!("not a 32-bit createTimeHigh: {h:?}"))?;
    let low: i32 = l
        .trim()
        .parse()
        .map_err(|_| format!("not a 32-bit createTimeLow: {l:?}"))?;
    Ok(timeglyph::compose::vmsd(high, low))
}

/// Parse a `"<Mon DD HH:MM:SS>@<reference>"` RFC 3164 syslog value: the year is
/// inferred from the ISO-8601 reference instant.
fn parse_syslog(value: &str) -> Result<timeglyph::PosixNs, String> {
    let (dt_s, ref_s) = value
        .split_once('@')
        .ok_or_else(|| format!("expected '<Mon DD HH:MM:SS>@<reference>', got {value:?}"))?;
    let reference = interpret::interpret_string(ref_s.trim())
        .into_iter()
        .find(|c| c.format_id == "iso8601")
        .map(|c| c.instant)
        .ok_or_else(|| format!("reference must be an ISO 8601 datetime: {ref_s:?}"))?;
    interpret::parse_syslog_with_reference(dt_s.trim(), reference)
        .ok_or_else(|| format!("not an RFC 3164 syslog date: {dt_s:?}"))
}

/// Parse a `"<week>:<tow>"` GPS pair into a leap-correct reading.
#[cfg(feature = "leap")]
fn parse_gps_week_tow(value: &str) -> Result<timeglyph::leap::LeapReading, String> {
    let (w, t) = value
        .split_once([':', '@', ','])
        .ok_or_else(|| format!("expected '<week>:<tow>', got {value:?}"))?;
    let week: u32 = w
        .trim()
        .parse()
        .map_err(|_| format!("not an integer GPS week: {w:?}"))?;
    let tow: f64 = t
        .trim()
        .parse()
        .map_err(|_| format!("not a time-of-week: {t:?}"))?;
    Ok(timeglyph::compose::gps_week_tow(week, tow))
}

/// Parse a `"<ticks>@<anchor>"` boot-relative value: an integer duration in
/// `unit` after an ISO-8601 anchor instant.
fn parse_relative(value: &str, unit: timeglyph::Unit) -> Result<timeglyph::PosixNs, String> {
    let (ticks_s, anchor_s) = value
        .split_once('@')
        .ok_or_else(|| format!("expected '<ticks>@<anchor>', got {value:?}"))?;
    let ticks: i64 = ticks_s
        .trim()
        .parse()
        .map_err(|_| format!("not integer ticks: {ticks_s:?}"))?;
    let anchor = interpret::interpret_string(anchor_s.trim())
        .into_iter()
        .find(|c| c.format_id == "iso8601")
        .map(|c| c.instant)
        .ok_or_else(|| format!("anchor must be an ISO 8601 datetime: {anchor_s:?}"))?;
    Ok(timeglyph::compose::relative(anchor, ticks, unit))
}

/// Parse a `"sec:nsec"` (or `sec.nsec` / `sec,nsec`) decimal timespec pair.
fn parse_unix_sec_nsec(value: &str) -> Result<timeglyph::PosixNs, String> {
    let (s, n) = value
        .split_once([':', '.', ','])
        .ok_or_else(|| format!("expected 'sec:nsec', got {value:?}"))?;
    let sec: i64 = s
        .trim()
        .parse()
        .map_err(|_| format!("not integer seconds: {s:?}"))?;
    let nsec: u32 = n
        .trim()
        .parse()
        .map_err(|_| format!("not integer nanoseconds: {n:?}"))?;
    Ok(timeglyph::compose::unix_sec_nsec(sec, nsec))
}

/// Parse a `"low:high"` (or `low|high`) pair of 32-bit hex halves and reconstruct
/// the FILETIME they encode.
fn parse_filetime_hilo(value: &str) -> Result<timeglyph::PosixNs, String> {
    let (lo, hi) = value
        .split_once([':', '|'])
        .ok_or_else(|| format!("expected 'low:high', got {value:?}"))?;
    let half = |h: &str| -> Result<u32, String> {
        u32::from_str_radix(h.trim().trim_start_matches("0x"), 16)
            .map_err(|_| format!("not a 32-bit hex half: {h:?}"))
    };
    timeglyph::compose::filetime_hilo(half(lo)?, half(hi)?).map_err(|e| e.to_string())
}

fn run_decode(format: &str, value: &str, zone: &RenderZone, style: DateStyle) -> u8 {
    // Leap-aware scales (gps/tai64/ntp) decode separately — never via PosixNs.
    #[cfg(feature = "leap")]
    if let Ok(v) = value.parse::<i64>() {
        if let Some(result) = timeglyph::leap::decode(format, v) {
            return match result {
                Ok(r) => {
                    println!(
                        "{}  {value}  ->  {}  (leap-correct UTC)",
                        r.scale, r.utc_rfc3339
                    );
                    for a in &r.assumptions {
                        println!("    - {a}");
                    }
                    EXIT_OK
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    EXIT_ERR
                }
            };
        }
    }
    // GPS week+TOW is leap-aware (returns a LeapReading, out of the PosixNs
    // spine), so it decodes like the other leap scales, not via decode_composite.
    #[cfg(feature = "leap")]
    if format == "gps_week_tow" {
        return match parse_gps_week_tow(value) {
            Ok(r) => {
                println!(
                    "{}  {value}  ->  {}  (leap-correct UTC)",
                    r.scale, r.utc_rfc3339
                );
                for a in &r.assumptions {
                    println!("    - {a}");
                }
                EXIT_OK
            }
            Err(msg) => {
                eprintln!("error: cannot decode {value:?} as {format}: {msg}");
                EXIT_ERR
            }
        };
    }
    // Composite (two-word) formats take a "low:high" hex pair, reassembled and
    // decoded via the underlying single-value format's epoch math.
    if let Some((result, render_id)) = decode_composite(format, value) {
        return match result {
            Ok(instant) => match timeglyph::format(render_id) {
                Ok(f) => print_decode(f, value, instant, zone, style),
                Err(_) => EXIT_ERR, // cov:unreachable: render_id is always registered
            },
            Err(msg) => {
                eprintln!("error: cannot decode {value:?} as {format}: {msg}");
                EXIT_ERR
            }
        };
    }
    let f = match timeglyph::format(format) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: {e}");
            return EXIT_ERR;
        }
    };
    // Integer value first; fall back to a float for float-encoded formats.
    let mut int_err: Option<timeglyph::ChronoError> = None;
    if let Ok(v) = value.parse::<i64>() {
        let sentinel = interpret::sentinel_reason(v);
        match f.decode_int(v) {
            Ok(instant) => {
                print_decode(f, value, instant, zone, style);
                return sentinel_exit(v, sentinel);
            }
            Err(e) => {
                if let Some(reason) = sentinel {
                    // e.g. 0x7FFFFFFFFFFFFFFF ("never") overflows the decode but is
                    // itself a meaningful sentinel — report it, not a generic error.
                    eprintln!("warning: {v} is a likely sentinel ({reason}) — 'unset'/'never', not a real instant");
                    return EXIT_AMBIGUOUS;
                }
                // Keep the reason; a non-sentinel integer falls through to float.
                int_err = Some(e);
            }
        }
    }
    if let Ok(v) = value.parse::<f64>() {
        match f.decode_float(v) {
            Ok(instant) => return print_decode(f, value, instant, zone, style),
            Err(e) => {
                eprintln!("error: {e}");
                return EXIT_ERR;
            }
        }
    }
    // Neither path produced a value: surface the specific decode failure (e.g.
    // out-of-range, with the offending value) rather than a generic message.
    match int_err {
        Some(e) => eprintln!("error: cannot decode {value:?} as {format}: {e}"),
        None => eprintln!("error: could not decode {value:?} as {format}"),
    }
    EXIT_ERR
}

fn print_decode(
    f: &timeglyph::Format,
    value: &str,
    instant: timeglyph::PosixNs,
    zone: &RenderZone,
    style: DateStyle,
) -> u8 {
    // A LocalNaive format is a zone-less wall-clock: render it naively (no shift,
    // no offset), same as identify and scan. Only UTC-anchored values shift.
    let (rendered, caveat) = if matches!(f.tz, timeglyph::TzSemantics::LocalNaive) {
        (
            timeglyph::datefmt::format_naive(instant, style),
            "  (LOCAL naive — not UTC)",
        )
    } else {
        (timeglyph::datefmt::format_instant(instant, zone, style), "")
    };
    println!("{}  {value}  ->  {rendered}{caveat}", f.id);
    EXIT_OK
}

/// Exit code for a single-format decode given its sentinel classification: a
/// sentinel raw value warns and signals "review needed" (`2`), never a confident `0`.
fn sentinel_exit(value: i64, sentinel: Option<&str>) -> u8 {
    if let Some(reason) = sentinel {
        eprintln!(
            "warning: {value} is a likely sentinel ({reason}) — 'unset'/'never', not a real instant"
        );
        EXIT_AMBIGUOUS
    } else {
        EXIT_OK
    }
}

fn run_encode(format: &str, datetime: &str) -> u8 {
    let Some(instant) = interpret::interpret_string(datetime)
        .first()
        .map(|c| c.instant)
    else {
        eprintln!("error: could not parse datetime {datetime:?} (try ISO 8601 / RFC 3339)");
        return EXIT_ERR;
    };
    if format == "all" {
        return run_encode_all(instant);
    }
    let f = match timeglyph::format(format) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: {e}");
            return EXIT_ERR;
        }
    };
    match f.encode(instant) {
        Ok(v) => {
            println!("{v}");
            EXIT_OK
        }
        Err(e) => {
            eprintln!("error: {e}");
            EXIT_ERR
        }
    }
}

/// `(LE, BE)` on-disk hex of an encoded value at `width` bytes (float formats are
/// 8-byte IEEE-754). BE takes the low `width` bytes of the big-endian layout.
fn needle_bytes(enc: timeglyph::Encoded, width: u8) -> (String, String) {
    match enc {
        timeglyph::Encoded::Int(v) => {
            let w = width as usize;
            (
                hex::encode(&v.to_le_bytes()[..w]),
                hex::encode(&v.to_be_bytes()[8 - w..]),
            )
        }
        timeglyph::Encoded::Float(x) => {
            (hex::encode(x.to_le_bytes()), hex::encode(x.to_be_bytes()))
        }
    }
}

/// Encode `instant` into every format that can represent it, with on-disk hex
/// bytes (LE/BE) at each format's natural width — a disk-search "needle" table.
/// A format that can't represent the instant is skipped. These are SEARCH
/// representations of a time, not proof the event occurred.
fn run_encode_all(instant: timeglyph::PosixNs) -> u8 {
    println!("# on-disk needles for this time (format  value  LE  BE) — search representations, not proof of occurrence");
    let mut any = false;
    for f in timeglyph::registry::FORMATS.iter() {
        let Ok(enc) = f.encode(instant) else { continue };
        let (le, be) = needle_bytes(enc, f.storage_bytes());
        println!("{:<16} {:<22} LE {le:<16} BE {be}", f.id, enc);
        any = true;
    }
    if any {
        EXIT_OK
    } else {
        eprintln!("error: no format could represent this instant");
        EXIT_ERR
    }
}

fn run_hex(bytes: &str, zone: &RenderZone, style: DateStyle, gap: f64) -> u8 {
    match interpret::interpret_hex(bytes) {
        Ok(groups) => {
            // Byte layout is inherently ambiguous (LE/BE x widths x word orders x
            // formats); flatten every layout's candidates, rank globally, and
            // apply the SAME ambiguity_code as identify — so a hex value with
            // tied top readings reports EXIT_AMBIGUOUS, not a false OK.
            let mut all: Vec<Candidate> = Vec::new();
            for (layout, cands) in &groups {
                println!("# byte layout: {layout}");
                print_candidates(cands, zone, style);
                all.extend(cands.iter().cloned());
            }
            all.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            ambiguity_code(&all, gap)
        }
        Err(e) => {
            eprintln!("error: {e}");
            EXIT_ERR
        }
    }
}

fn run_string(text: &str, zone: &RenderZone, style: DateStyle) -> u8 {
    let cands = interpret::interpret_string(text);
    if cands.is_empty() {
        eprintln!("error: {text:?} did not parse as any known string timestamp form");
        return EXIT_ERR;
    }
    println!("# readings consistent with {text:?}:");
    print_candidates(&cands, zone, style);
    EXIT_OK
}

/// Scan `text` (or stdin) for timestamp candidates and print each with its
/// readings. `all` keeps sentinel/out-of-window readings and shows every one.
/// JSON output schema version for the machine-readable surfaces. Bump on any
/// breaking shape change so downstream parsers can pin the contract.
const SCHEMA_VERSION: u32 = 1;

/// The `--provenance` envelope wrapping identify `--json` output: a reproducible
/// record of what decoded the value, for court-defensible/citable output. The
/// readings are the full `Candidate`s (citation, components, assumptions).
#[derive(serde::Serialize)]
struct ProvenanceEnvelope<'a> {
    schema_version: u32,
    engine: &'static str,
    engine_version: &'static str,
    registry_digest: String,
    input: &'a str,
    readings: &'a [Candidate],
}

/// One reading in JSON output — a deliberate CLI-owned shape (not a serialized
/// display type), so the wire contract is stable and versioned.
#[derive(serde::Serialize)]
struct ReadingJson<'a> {
    format_id: &'a str,
    label: &'a str,
    rendered: &'a str,
    score: f64,
    local: bool,
}

/// One scanned value with its ranked readings (a JSONL record for `scan --json`).
#[derive(serde::Serialize)]
struct ScanRecordJson<'a> {
    schema_version: u32,
    number: &'a str,
    readings: Vec<ReadingJson<'a>>,
}

fn run_scan(
    text: Option<&str>,
    min_digits: usize,
    top: Option<usize>,
    json: bool,
    zone: &RenderZone,
    style: DateStyle,
) -> u8 {
    let input = if let Some(t) = text {
        t.to_string()
    } else {
        use std::io::Read;
        let mut s = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut s) {
            eprintln!("error: could not read stdin: {e}");
            return EXIT_ERR;
        }
        s
    };
    // Show-all: no likelihood filter (include_all = true → out-of-window and
    // sentinel readings are shown too, ranked). --top only caps the count for
    // brevity; likelihood orders the readings, it never gates them.
    let max = top.unwrap_or(usize::MAX);
    let hits = timeglyph::scan::inspect_text_opts(&input, max, min_digits, true, zone, style);
    if json {
        // JSONL: one object per value, so the stream is line-consumable (jq -c,
        // SIEM ingest) without buffering the whole scan.
        for nr in &hits {
            let record = ScanRecordJson {
                schema_version: SCHEMA_VERSION,
                number: &nr.number,
                readings: nr
                    .readings
                    .iter()
                    .map(|r| ReadingJson {
                        format_id: &r.format_id,
                        label: &r.label,
                        rendered: &r.rendered,
                        score: r.score,
                        local: r.local,
                    })
                    .collect(),
            };
            match serde_json::to_string(&record) {
                Ok(line) => println!("{line}"),
                Err(e) => {
                    eprintln!("error: serializing scan record: {e}");
                    return EXIT_ERR;
                }
            }
        }
        return EXIT_OK;
    }
    for nr in &hits {
        println!("{}", nr.number);
        for r in &nr.readings {
            println!("    {r}");
        }
    }
    EXIT_OK
}

#[cfg(feature = "lunisolar")]
fn run_lunisolar(datetime: &str, longitude: Option<f64>, zone: &RenderZone, tz_given: bool) -> u8 {
    if !tz_given {
        eprintln!(
            "error: lunisolar conversion requires a timezone (--tz) — the Chinese calendar is \
             meridian-relative (China UTC+8, Vietnam UTC+7, Korea UTC+9)"
        );
        return EXIT_ERR;
    }
    // Accept a Unix-seconds integer or any self-describing string form (ISO 8601
    // / RFC 3339 / ASN.1) that interpret_string can parse.
    let instant = if let Ok(secs) = datetime.parse::<i64>() {
        match timeglyph::format("unix").and_then(|f| f.decode_int(secs)) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("error: {e}");
                return EXIT_ERR;
            }
        }
    } else if let Some(c) = interpret::interpret_string(datetime).first() {
        c.instant
    } else {
        eprintln!("error: could not parse {datetime:?} as a datetime (try ISO 8601 / RFC 3339)");
        return EXIT_ERR;
    };
    match timeglyph::lunisolar::render(instant, zone, longitude) {
        Ok(r) => {
            let leap = if r.is_leap_month { "閏" } else { "" };
            println!("{}", r.civil_local);
            println!(
                "  lunisolar: {}年 {leap}{}月 {}日",
                r.lunar_year, r.lunar_month, r.lunar_day
            );
            println!(
                "  四柱 pillars: {}年 {}月 {}日 {}時",
                r.year_pillar, r.month_pillar, r.day_pillar, r.hour_pillar
            );
            println!(
                "  solar: λ {:.2}° ({})",
                r.solar_longitude_deg, r.solar_term
            );
            for a in &r.assumptions {
                println!("    - {a}");
            }
            EXIT_OK
        }
        Err(e) => {
            eprintln!("error: {e}");
            EXIT_ERR
        }
    }
}

fn run_list() -> u8 {
    for f in timeglyph::registry::FORMATS.iter() {
        println!("{:<16} {:<48} {}", f.id, f.label, f.citation);
    }
    EXIT_OK
}

/// `mcp` subcommand: an MCP stdio server. Reads one JSON-RPC message per line
/// from stdin, hands it to the pure [`timeglyph::mcp::handle`], and writes each
/// response line to stdout. The loop is the irreducible I/O shell; the protocol
/// logic is tested in the library.
fn run_mcp() -> u8 {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = timeglyph::mcp::handle(&line) {
            if writeln!(stdout, "{response}").is_err() || stdout.flush().is_err() {
                break;
            }
        }
    }
    EXIT_OK
}

/// `explain` subcommand: print a format's registry-generated spec card.
fn run_explain(format: &str) -> u8 {
    if let Some(card) = interpret::explain(format) {
        println!("{card}");
        EXIT_OK
    } else {
        eprintln!("error: unknown format '{format}' (see `list` for the ids)");
        EXIT_ERR
    }
}

/// What `cal`'s `WHEN` argument selects.
enum CalWhen {
    Year(i16),
    Month(i16, i8),
    Day(jiff::civil::Date),
}

/// Parse `YYYY` / `YYYY-MM` / `YYYY-MM-DD`, or default to the current month in
/// `zone`. Returns the offending string on a parse error.
fn parse_cal_when(when: Option<&str>, zone: &RenderZone) -> Result<CalWhen, String> {
    let Some(w) = when else {
        let now = today_in(zone);
        return Ok(CalWhen::Month(now.year(), now.month()));
    };
    let parts: Vec<&str> = w.split('-').collect();
    let bad = || format!("error: expected YYYY, YYYY-MM, or YYYY-MM-DD, got \"{w}\"");
    match parts.as_slice() {
        [y] => y.parse::<i16>().map(CalWhen::Year).map_err(|_| bad()),
        [y, m] => {
            let (y, m) = (
                y.parse::<i16>().map_err(|_| bad())?,
                m.parse::<i8>().map_err(|_| bad())?,
            );
            Ok(CalWhen::Month(y, m))
        }
        [y, m, d] => {
            let (y, m, d) = (
                y.parse::<i16>().map_err(|_| bad())?,
                m.parse::<i8>().map_err(|_| bad())?,
                d.parse::<i8>().map_err(|_| bad())?,
            );
            jiff::civil::Date::new(y, m, d)
                .map(CalWhen::Day)
                .map_err(|_| bad())
        }
        _ => Err(bad()),
    }
}

/// The current date in the render zone (the clock read lives in the shell).
fn today_in(zone: &RenderZone) -> jiff::civil::Date {
    let ts = jiff::Timestamp::now();
    let tz = match zone {
        RenderZone::Utc => jiff::tz::TimeZone::UTC,
        RenderZone::Fixed(o) => o.to_time_zone(),
        RenderZone::Named(t) => t.clone(),
    };
    ts.to_zoned(tz).date()
}

/// `cal` subcommand: render a forensic calendar (year / month / single day).
#[cfg_attr(not(feature = "lunisolar"), allow(unused_variables))]
fn run_cal(when: Option<&str>, week_start: &str, south: bool, json: bool, zone: &RenderZone) -> u8 {
    use timeglyph::cal::{build_day, build_month, WeekStart};
    let ws = match week_start.to_ascii_lowercase().as_str() {
        "monday" | "mon" => WeekStart::Monday,
        "sunday" | "sun" => WeekStart::Sunday,
        other => {
            eprintln!("error: --week-start must be monday or sunday, got \"{other}\"");
            return EXIT_ERR;
        }
    };
    let target = match parse_cal_when(when, zone) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            return EXIT_ERR;
        }
    };
    let today = today_in(zone);

    match target {
        CalWhen::Day(date) => {
            let Ok(day) = build_day(date, zone) else {
                eprintln!("error: {date} is out of the representable range");
                return EXIT_ERR;
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&day).unwrap_or_default());
            } else {
                println!("{}", timeglyph::cal_render::render_day_text(&day));
            }
            EXIT_OK
        }
        CalWhen::Month(y, m) => match build_month(y, m, zone, ws) {
            Ok(month) => {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&month).unwrap_or_default()
                    );
                } else {
                    print!(
                        "{}",
                        timeglyph::cal_render::render_month_text(&month, Some(today))
                    );
                }
                EXIT_OK
            }
            Err(e) => {
                eprintln!("error: {e}");
                EXIT_ERR
            }
        },
        CalWhen::Year(y) => {
            let mut months = Vec::new();
            for m in 1..=12 {
                match build_month(y, m, zone, ws) {
                    Ok(month) => months.push(month),
                    Err(e) => {
                        eprintln!("error: {e}");
                        return EXIT_ERR;
                    }
                }
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&months).unwrap_or_default()
                );
            } else {
                #[cfg(feature = "lunisolar")]
                {
                    use timeglyph::cal::{season_markers, Hemisphere};
                    let hemi = if south {
                        Hemisphere::South
                    } else {
                        Hemisphere::North
                    };
                    print!(
                        "{}",
                        timeglyph::cal_art::season_strip(y, &season_markers(y), hemi)
                    );
                    println!();
                }
                for month in &months {
                    print!(
                        "{}",
                        timeglyph::cal_render::render_month_text(month, Some(today))
                    );
                    println!();
                }
            }
            EXIT_OK
        }
    }
}

/// `carve` subcommand: hex bytes → bounded carve → text / JSONL / ImHex bookmarks.
fn run_carve(
    hex: Option<&str>,
    min_score: f64,
    from: Option<i16>,
    to: Option<i16>,
    json: bool,
    imhex: bool,
) -> u8 {
    use std::io::Read;
    let raw = match hex {
        Some(h) if h != "-" => h.to_string(),
        _ => {
            let mut s = String::new();
            if std::io::stdin().read_to_string(&mut s).is_err() {
                eprintln!("error: could not read hex from stdin");
                return EXIT_ERR;
            }
            s
        }
    };
    let clean: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != ':')
        .collect();
    let clean = clean
        .strip_prefix("0x")
        .or_else(|| clean.strip_prefix("0X"))
        .unwrap_or(&clean);
    let Ok(bytes) = hex::decode(clean) else {
        eprintln!("error: input is not valid hex bytes");
        return EXIT_ERR;
    };
    let year_ns = |y: i16| -> Option<i128> {
        jiff::civil::Date::new(y, 1, 1)
            .ok()?
            .at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .ok()
            .map(|z| z.timestamp().as_nanosecond())
    };
    let window = match (from.and_then(year_ns), to.and_then(year_ns)) {
        (Some(lo), Some(hi)) => Some((lo, hi)),
        _ => None,
    };
    let hits = timeglyph::carve::carve(&bytes, min_score, window);
    if json {
        println!("{}", timeglyph::carve::to_jsonl(&hits));
    } else if imhex {
        println!("{}", timeglyph::carve::to_imhex_bookmarks(&hits));
    } else if hits.is_empty() {
        println!("# no timestamp readings above the score/window threshold");
    } else {
        for h in &hits {
            println!(
                "  @{:<5} [{:.2}] {:<14} {}  ({})",
                h.offset,
                h.reading.score,
                h.reading.format_id,
                h.reading.rendered.as_deref().unwrap_or("?"),
                h.lane
            );
        }
    }
    EXIT_OK
}

#[cfg(feature = "csv")]
fn run_csv(
    path: &str,
    convert: &[String],
    auto: bool,
    replace: bool,
    output: Option<&str>,
    zone: &RenderZone,
) -> u8 {
    let input = if path == "-" {
        let mut s = String::new();
        if let Err(e) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut s) {
            eprintln!("error: could not read stdin: {e}");
            return EXIT_ERR;
        }
        s
    } else {
        match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read {path}: {e}");
                return EXIT_ERR;
            }
        }
    };
    let mut conversions = Vec::new();
    for c in convert {
        match c.split_once(':') {
            Some((col, fmt)) if !col.is_empty() && !fmt.is_empty() => {
                conversions.push(Conversion {
                    column: col.to_string(),
                    format: fmt.to_string(),
                });
            }
            _ => {
                eprintln!("error: --convert expects COLUMN:FORMAT, got {c:?}");
                return EXIT_ERR;
            }
        }
    }
    // Auto-detect by default when no explicit conversion was requested.
    let auto = auto || conversions.is_empty();
    let opts = EnrichOptions {
        conversions,
        auto,
        replace,
        zone: zone.clone(),
    };
    match timeglyph::csv_enrich::enrich(&input, &opts) {
        Ok(out) => {
            if let Some(path) = output {
                if let Err(e) = std::fs::write(path, out) {
                    eprintln!("error: cannot write {path}: {e}");
                    return EXIT_ERR;
                }
            } else {
                print!("{out}");
            }
            EXIT_OK
        }
        Err(e) => {
            eprintln!("error: {e}");
            EXIT_ERR
        }
    }
}

fn print_candidates(cands: &[Candidate], zone: &RenderZone, style: DateStyle) {
    if cands.is_empty() {
        println!("  (no plausible interpretation)");
        return;
    }
    for c in cands {
        let flag = if c.sentinel { " [sentinel]" } else { "" };
        // tz-aware (see render_candidate): local-naive stays a naked wall-clock;
        // UTC-anchored styles into the display zone when representable.
        let rendered = render_candidate(c, zone, style);
        println!(
            "  [{:.2}] {:<16} {}  ({}){flag}",
            c.score,
            c.format_id,
            rendered.as_deref().unwrap_or("<out of range>"),
            c.label,
        );
    }
}
