//! Bounded carve: slide [`identify_bytes`](crate::interpret::identify_bytes) over
//! a blob and report scored hits per offset — the validatable core of "find
//! timestamps in raw bytes" (a malware config blob, a hex selection, one record).
//!
//! At every offset *something* plausibly decodes, so a `window` + `min_score` are
//! the false-positive controls, and the sweep is **bounded** (`MAX_CARVE_BYTES`)
//! so a large input can't drive an unbounded `O(n·formats)` scan. This is NOT an
//! image-wide firehose — that needs a measured precision/recall harness before it
//! can claim "discovery"; here the caller passes a bounded region.

use crate::interpret::{identify_bytes, Candidate};

/// A reading found at a byte offset, tagged with the [`identify_bytes`] lane it
/// came from (e.g. `"u64 LE"`, `"f64 LE (IEEE-754 double)"`).
pub struct CarveHit {
    /// Byte offset into the carved buffer where the reading starts.
    pub offset: usize,
    /// The `identify_bytes` lane label the reading came from.
    pub lane: String,
    /// The scored reading.
    pub reading: Candidate,
}

/// Hard ceiling on the swept length — keeps a bounded carve bounded.
const MAX_CARVE_BYTES: usize = 4096;

/// Slide [`identify_bytes`] over every offset of `bytes` and return each reading
/// whose score ≥ `min_score` and whose instant falls in `window` (nanoseconds,
/// `[lo, hi)`, if given), tagged with its offset. Sorted by offset, then score
/// descending. At most [`MAX_CARVE_BYTES`] of input is swept.
#[must_use]
pub fn carve(bytes: &[u8], min_score: f64, window: Option<(i128, i128)>) -> Vec<CarveHit> {
    let n = bytes.len().min(MAX_CARVE_BYTES);
    let in_window = |i: i128| window.is_none_or(|(lo, hi)| i >= lo && i < hi);
    let mut hits = Vec::new();
    for off in 0..n {
        for (lane, cands) in identify_bytes(&bytes[off..n]) {
            for c in cands {
                if c.score >= min_score && in_window(c.instant.0) {
                    hits.push(CarveHit {
                        offset: off,
                        lane: lane.clone(),
                        reading: c,
                    });
                }
            }
        }
    }
    hits.sort_by(|a, b| {
        a.offset.cmp(&b.offset).then(
            b.reading
                .score
                .partial_cmp(&a.reading.score)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    hits
}

/// The on-disk byte width a carve lane decoded from — the region size for a
/// bookmark. Derived from the lane label (`u32`/`FAT` = 4, `SYSTEMTIME` = 16,
/// everything else — `u64`/`f64`/all-ones — 8).
fn lane_size(lane: &str) -> usize {
    if lane.contains("SYSTEMTIME") {
        16
    } else if lane.contains("u32") || lane.contains("FAT") {
        4
    } else {
        8
    }
}

/// Serialize carve hits as JSONL — one object per line (`offset`, `size`, `lane`,
/// `format`, `rendered`, `instant_ns`, `score`, `citation`) so the stream pipes,
/// greps, and processes with bounded memory. Machine-faithful (the exact format
/// id, the full instant as a string so no precision is lost).
#[must_use]
pub fn to_jsonl(hits: &[CarveHit]) -> String {
    hits.iter()
        .map(|h| {
            serde_json::json!({
                "offset": h.offset,
                "size": lane_size(&h.lane),
                "lane": h.lane,
                "format": h.reading.format_id,
                "rendered": h.reading.rendered,
                "instant_ns": h.reading.instant.0.to_string(),
                "score": h.reading.score,
                "citation": h.reading.citation,
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Serialize carve hits as ImHex bookmarks — one `region {address, size}` per hit
/// so a hex editor marks each timestamp offset. The comment carries the rendered
/// time, lane, and score. Import via ImHex → Bookmarks → Import.
#[must_use]
pub fn to_imhex_bookmarks(hits: &[CarveHit]) -> String {
    let bookmarks: Vec<serde_json::Value> = hits
        .iter()
        .map(|h| {
            serde_json::json!({
                "region": { "address": h.offset, "size": lane_size(&h.lane) },
                "name": h.reading.format_id,
                "comment": format!(
                    "{} — {} (score {:.2})",
                    h.reading.rendered.as_deref().unwrap_or("?"),
                    h.lane,
                    h.reading.score
                ),
                "color": 0x5000_ff00_u32, // translucent green
                "locked": false,
            })
        })
        .collect();
    serde_json::json!({ "bookmarks": bookmarks }).to_string()
}
