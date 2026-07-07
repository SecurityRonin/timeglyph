# Test data — provenance

Every artifact here carries a documented provenance entry (fleet standard):
source, identity/metadata, original download URL, checksums, contents,
redistribution/license, and the use case. Small, clearly-licensed fixtures are
committed; artifacts whose redistribution is unclear are **not committed** —
they are documented here and downloaded manually.

These are real WhatsApp message stores carved from **Josh Hickman's public
research device images** on **Digital Corpora** (AWS Open Data Sponsorship —
freely redistributable with attribution). They are the tier-1 ground truth for
timeglyph's two WhatsApp timestamp families: Android's Unix-milliseconds and
iOS's Cocoa/CFAbsoluteTime. The full multi-GB source images are gitignored large
artifacts owned by **issen** (`~/src/issen/tests/data/`, documented there); only
the small DB files are extracted and committed here.

## `josh-hickman-android10/msgstore.db` — WhatsApp (Android), real

| Field | Value |
|---|---|
| **Source** | Josh Hickman (The Binary Hick), Android 10 **Pixel 3** public research image, via **Digital Corpora**. Blog: <https://thebinaryhick.blog/>. |
| **Source image** | `josh-hickman-android10/android10-pixel3-fs.zip` in the issen corpus (SHA-256 `ca6918ef…a8aa00a`). |
| **Original download** | <https://digitalcorpora.s3.amazonaws.com/corpora/mobile/android_10/Non-Cellebrite%20Extraction/Pixel%203.zip> |
| **Extracted from** | `Pixel 3/data/data/com.whatsapp/databases/msgstore.db` inside that zip. The live `-wal` was checkpointed into the main DB, so the committed file is a self-contained single `.db`. |
| **Size** | 804 KB (823,296 bytes) |
| **MD5** | `9313bcb2d92c3249aff3c20ad8a2ab7a` |
| **SHA-256** | `9e133d7262f526b1dab7313f636a1a3f32984d8008310a1a699c83496dd13105` |
| **Format** | SQLite 3, WhatsApp/Android schema (legacy `messages` table populated; newer `message` table empty), plus `chat`, `jid`. |
| **Contents** | Real WhatsApp data from the research device's test account — 18 messages in the `messages` table. |
| **Timestamps** | `messages.timestamp` in **Unix milliseconds**, spanning `1487100001000` (2017-02-14T19:20:01Z) → `1581271502890` (2020-02-09T18:05:02.89Z). |
| **License / redistribution** | **Digital Corpora / AWS Open Data Sponsorship** — freely redistributable for research/testing with attribution. Committed here. |
| **Use case** | Ground truth for WhatsApp Android timestamp decoding: `messages.timestamp` decodes via timeglyph to the `unix_ms` reading — e.g. `1581271502890` → `2020-02-09T18:05:02.89Z`. |
| **Consumed by** | Env-gated validation test (skips cleanly when absent). |

## `josh-hickman-ios13/ChatStorage.sqlite` — WhatsApp (iOS), real

| Field | Value |
|---|---|
| **Source** | Josh Hickman (The Binary Hick), iOS 13.3.1 public research image, via **Digital Corpora**. Blog: <https://thebinaryhick.blog/>. |
| **Source image** | `josh-hickman-ios13/ios_13_3_1.zip` in the issen corpus (SHA-256 `f194e8bb…d34b2643`). |
| **Original download** | <https://digitalcorpora.s3.amazonaws.com/corpora/mobile/ios_13_3_1/ios_13_3_1.zip> |
| **Extracted from** | the nested `iOS 13.3.1 Extraction/Extraction/13-3-1.tar` full-FS image, at `…/private/var/mobile/Containers/Shared/AppGroup/BAF442BF-69A8-4336-86BC-37604B5C9A7C/ChatStorage.sqlite`. |
| **Size** | 336 KB (344,064 bytes) |
| **MD5** | `8a597e2c9aa5e024661bd56ce5eef4a6` |
| **SHA-256** | `e5f6559b278cc219eff09ae9b8303a69aab3a751c8ee35b8d154496ae830f4a9` |
| **Format** | SQLite 3, WhatsApp/iOS Core Data schema — `ZWAMESSAGE`, `ZWACHATSESSION`, `ZWAMESSAGEDATAITEM`. |
| **Contents** | Real WhatsApp data from the research device's test account — 12 messages in `ZWAMESSAGE`. |
| **Timestamps** | `ZWAMESSAGE.ZMESSAGEDATE` in **Cocoa / CFAbsoluteTime** — a `double` of seconds since 2001-01-01 *with sub-second precision*, e.g. `606940977.71577` → `608322295.31165`, spanning 2020-03-26T18:42:57.716Z → 2020-04-11T18:24:55.312Z. |
| **License / redistribution** | **Digital Corpora / AWS Open Data Sponsorship** — freely redistributable for research/testing with attribution. Committed here. |
| **Use case** | Ground truth for WhatsApp iOS timestamp decoding via the **`cocoa_float`** decoder, which keeps the sub-second fraction: `timeglyph decode cocoa_float 606940977.71577` → `2020-03-26T18:42:57.715769984Z`. Exercises the Cocoa/CFAbsoluteTime (float) family that the Android `msgstore.db` (Unix-ms integer) does not. (The auto-identify path currently ingests integers only, so `timeglyph 606940977` decodes whole-seconds via the integer `cocoa` decoder — pass the float to `decode cocoa_float` for full precision.) |
| **Consumed by** | Env-gated validation test (skips cleanly when absent). |

## Not included: iOS 17

Josh Hickman's iOS 17 image (`josh-hickman-ios17-biome-segb/` in issen) also
contains WhatsApp, but only inside nested archives — a 2.3 GB Finder backup and a
36.9 GB Cellebrite FFS zip. Its `ZMESSAGEDATE` is the **same Cocoa/CFAbsoluteTime
format** already covered by `josh-hickman-ios13/`, so extracting it adds no new
timestamp-decoder coverage. Skipped deliberately; extract it only if a newer iOS
17 WhatsApp sample is needed for other reasons.

## `calibration.csv` — labeled ground-truth for scoring calibration

A committed dataset of `value,true_format,source` rows, each a real timestamp
whose true format is known from the artifact it was carved from. It measures the
ranking's top-1/top-3 identification accuracy (Tier-3 scoring work) against real
values rather than self-authored fixtures. All values come from **reproducible
public images** (AWS Open Data — redistributable); only the integer values +
labels are committed, no URLs or content.

| true_format | n | source |
|---|---|---|
| `unix_ms` | 18 | `josh-hickman-android10/msgstore.db` — `messages.timestamp` (real) |
| `cocoa` | 11 | `josh-hickman-ios13/ChatStorage.sqlite` — `ZWAMESSAGE.ZMESSAGEDATE` int part (real) |
| `webkit` | 13 | `josh-hickman-android10` Chrome — `urls.last_visit_time` (real) |
| `unix` | 15 | `josh-hickman-android10` MediaStore `external.db` — `files.date_added` (real) |
| `filetime` | 15 | `magnet-summit-2023-ctf` `SOFTWARE` hive — registry key LastWrite via `regipy` (real) |
| `iostime` | 10 | `time-decode --iostime` for real 2015–2024 dates — **tier-1 oracle-generated** (no clean in-hand artifact uses ns-since-2001; the values carry real-era magnitudes) |

**Measured baseline (this corpus, 82 values):** top-1 26.2% on the first three
formats alone was optimistic; adding `unix`/`filetime`/`iostime` exposed the
honest number — **top-1 13.4%, top-3 91.5%**, with only `cocoa` reliably ranked
#1. That diversity is the guard against tuning a magnitude/recency prior to a
narrow corpus. Cross-checking oracles: `time-decode` (per-format) and `unfurl`
(embedded-ID family).

## Checksum manifest

```
9313bcb2d92c3249aff3c20ad8a2ab7a  josh-hickman-android10/msgstore.db
8a597e2c9aa5e024661bd56ce5eef4a6  josh-hickman-ios13/ChatStorage.sqlite
```
