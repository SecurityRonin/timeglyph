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
    #[command(subcommand)]
    command: Option<Commands>,
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
        /// Include sentinel and out-of-window readings too (noisier).
        #[arg(long)]
        all: bool,
    },
    /// List every registered format with its citation.
    List,
    /// Enrich a CSV: add a human-readable column for each timestamp column.
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
        Some(Commands::Identify { value, json }) => {
            run_identify(&value, json, &zone, style, cli.artifact.as_deref(), mode)
        }
        Some(Commands::Decode { format, value }) => run_decode(&format, &value, &zone, style),
        Some(Commands::Encode { format, datetime }) => run_encode(&format, &datetime),
        Some(Commands::Scan {
            text,
            min_digits,
            all,
        }) => run_scan(text.as_deref(), min_digits, all, &zone, style),
        Some(Commands::List) => run_list(),
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
                run_identify(&v, cli.json, &zone, style, cli.artifact.as_deref(), mode)
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
fn ambiguity_code(cands: &[Candidate]) -> u8 {
    let Some(top) = cands.first() else {
        return EXIT_AMBIGUOUS;
    };
    if top.sentinel {
        return EXIT_AMBIGUOUS;
    }
    if cands.len() >= 2 && (top.score - cands[1].score).abs() < 1e-9 {
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

fn run_identify(
    input: &str,
    json: bool,
    zone: &RenderZone,
    style: DateStyle,
    artifact: Option<&str>,
    mode: AsArg,
) -> u8 {
    let s = input.trim();
    // Forced families delegate to their own handlers (hex/string keep their own
    // print + exit semantics); auto and int share the merge/rank tail below.
    match mode {
        AsArg::Hex => return run_hex(s, zone, style),
        AsArg::String => return run_string(s, zone, style),
        AsArg::Auto | AsArg::Int => {}
    }
    // Auto also auto-detects hex: a `0x`/`0X` prefix or a-f letters mean the value
    // is raw bytes, not a decimal integer — those win over string-looking digits.
    if mode == AsArg::Auto
        && (s.starts_with("0x") || s.starts_with("0X") || looks_like_hex_bytes(s))
    {
        return run_hex(s, zone, style);
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
    if json {
        match serde_json::to_string_pretty(&cands) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("error: serializing candidates: {e}");
                return EXIT_ERR;
            }
        }
        return ambiguity_code(&cands);
    }
    println!(
        "# readings consistent with {s} (ranked; a raw value is usually \
         underdetermined — not a single verdict):"
    );
    print_candidates(&cands, zone, style);
    ambiguity_code(&cands)
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
    }
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

fn run_hex(bytes: &str, zone: &RenderZone, style: DateStyle) -> u8 {
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
            ambiguity_code(&all)
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
fn run_scan(
    text: Option<&str>,
    min_digits: usize,
    all: bool,
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
    let max = if all { usize::MAX } else { 4 };
    for nr in timeglyph::scan::inspect_text_opts(&input, max, min_digits, all, zone, style) {
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
    for f in timeglyph::registry::FORMATS {
        println!("{:<16} {:<48} {}", f.id, f.label, f.citation);
    }
    EXIT_OK
}

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
