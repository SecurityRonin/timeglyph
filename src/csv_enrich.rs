//! CSV enrichment: add human-readable timestamp columns to a CSV.
//!
//! Two modes (composable):
//! - **Explicit** — convert a named column under a chosen format, writing the
//!   rendered instant into a new column immediately to its right.
//! - **Auto** — detect numeric columns whose values are plausibly timestamps and
//!   append a rendered column. Auto is deliberately conservative: a column is
//!   only enriched when every non-empty cell is an integer at or above a
//!   magnitude floor (ruling out counts, ids, and years) AND they share one
//!   confident (in-window, non-sentinel) top interpretation.
//!
//! `--replace` overwrites the source column in place instead of adding one. The
//! engine never *guesses a single answer* for an ambiguous column — it only
//! enriches when the top reading is consistent across the column.

use std::collections::HashMap;

use crate::{format, interpret, ChronoError, RenderZone};

/// An explicit column→format conversion request.
pub struct Conversion {
    /// The source column header name.
    pub column: String,
    /// A registered format id (see [`crate::registry::FORMATS`]).
    pub format: String,
}

/// How to enrich the CSV.
pub struct EnrichOptions {
    /// Explicit per-column conversions.
    pub conversions: Vec<Conversion>,
    /// Also auto-detect numeric timestamp columns.
    pub auto: bool,
    /// Replace the source column in place rather than adding a new one.
    pub replace: bool,
    /// Output timezone for the rendered cells (default [`RenderZone::Utc`]).
    pub zone: RenderZone,
}

/// Integers below this are too small to be plausible timestamps (counts, ids,
/// years). ~1e8 ≈ Unix-seconds year 1973; ms/µs/ns timestamps are far larger.
const AUTO_MIN_MAGNITUDE: i64 = 100_000_000;

fn csv_err(e: &csv::Error) -> ChronoError {
    ChronoError::Render(format!("csv: {e}"))
}

/// Enrich `csv_text` per `opts`, returning the new CSV as a string.
pub fn enrich(csv_text: &str, opts: &EnrichOptions) -> Result<String, ChronoError> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(csv_text.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| csv_err(&e))?
        .iter()
        .map(String::from)
        .collect();
    let mut records: Vec<csv::StringRecord> = Vec::new();
    for rec in rdr.records() {
        records.push(rec.map_err(|e| csv_err(&e))?);
    }

    // Build the column → format plan.
    let mut plan: HashMap<usize, String> = HashMap::new();
    for c in &opts.conversions {
        let idx = headers
            .iter()
            .position(|h| h == &c.column)
            .ok_or_else(|| ChronoError::UnknownFormat(format!("column not found: {}", c.column)))?;
        format(&c.format)?; // validate the format id (loud error on unknown)
        plan.insert(idx, c.format.clone());
    }
    if opts.auto {
        for idx in 0..headers.len() {
            if plan.contains_key(&idx) {
                continue;
            }
            if let Some(fmt) = detect_column_format(&records, idx) {
                plan.insert(idx, fmt);
            }
        }
    }

    let mut wtr = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());

    let mut out_header: Vec<String> = Vec::new();
    for (idx, h) in headers.iter().enumerate() {
        match plan.get(&idx) {
            Some(fmt) if !opts.replace => {
                out_header.push(h.clone());
                out_header.push(format!("{h}_{fmt}"));
            }
            _ => out_header.push(h.clone()),
        }
    }
    wtr.write_record(&out_header).map_err(|e| csv_err(&e))?;

    for rec in &records {
        let mut row: Vec<String> = Vec::new();
        for (idx, _h) in headers.iter().enumerate() {
            let cell = rec.get(idx).unwrap_or("");
            match plan.get(&idx) {
                Some(fmt) => {
                    let rendered = render_cell(cell, fmt, &opts.zone);
                    if opts.replace {
                        row.push(rendered.unwrap_or_else(|| cell.to_string()));
                    } else {
                        row.push(cell.to_string());
                        row.push(rendered.unwrap_or_default());
                    }
                }
                None => row.push(cell.to_string()),
            }
        }
        wtr.write_record(&row).map_err(|e| csv_err(&e))?;
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| ChronoError::Render(format!("csv: {e}")))?;
    String::from_utf8(bytes).map_err(|e| ChronoError::Render(e.to_string()))
}

/// Render one cell under a known format id → RFC 3339, or `None` if it cannot be
/// decoded (empty/non-numeric/out of range). Integer first, then float.
fn render_cell(cell: &str, fmt: &str, zone: &RenderZone) -> Option<String> {
    let f = format(fmt).ok()?;
    if let Ok(v) = cell.trim().parse::<i64>() {
        if let Ok(inst) = f.decode_int(v) {
            return inst.render(zone);
        }
    }
    if let Ok(v) = cell.trim().parse::<f64>() {
        if let Ok(inst) = f.decode_float(v) {
            return inst.render(zone);
        }
    }
    None
}

/// Auto-detect a column's timestamp format, conservatively. Returns the format
/// id only when every non-empty cell is an integer at or above the magnitude
/// floor and they all share the same confident (in-window, non-sentinel) top
/// interpretation; otherwise `None` (the column is left untouched).
fn detect_column_format(records: &[csv::StringRecord], idx: usize) -> Option<String> {
    // Gather the column's numeric values first: they become each cell's
    // neighbours, so the column-coherence (`neighbour_monotonicity`) signal can
    // help disambiguate the per-cell top reading.
    let mut values: Vec<i64> = Vec::new();
    for rec in records {
        let cell = rec.get(idx).unwrap_or("").trim();
        if cell.is_empty() {
            continue;
        }
        let value: i64 = cell.parse().ok()?; // any non-integer → not a time column
        if value.abs() < AUTO_MIN_MAGNITUDE {
            return None;
        }
        values.push(value);
    }
    if values.is_empty() {
        return None;
    }
    let ctx = interpret::InterpretContext {
        neighbours: &values,
        ..Default::default()
    };
    let mut chosen: Option<String> = None;
    for &value in &values {
        let candidates = interpret::interpret_int_with_context(value, &ctx);
        let top = candidates.first()?;
        if top.sentinel {
            return None;
        }
        let in_window = top
            .components
            .iter()
            .any(|(name, v)| *name == "in_window" && *v > 0.0);
        if !in_window {
            return None;
        }
        match &chosen {
            None => chosen = Some(top.format_id.to_string()),
            Some(f) if f != top.format_id => return None, // inconsistent column
            _ => {}
        }
    }
    chosen
}
