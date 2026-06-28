---
title: "Identifiers with embedded timestamps — Snowflake, UUID, ULID, ObjectId"
description: >-
  Forensic reference for IDs that embed a creation time: Twitter/X and Discord
  Snowflakes, UUID v1/v6/v7 (RFC 9562), ULID, MongoDB ObjectId, KSUID, and
  Sonyflake — with bit layouts, epochs, extraction formulas, and worked examples.
---

# Identifiers with embedded timestamps

Many object IDs embed their **creation time**. Decoding it leaks when a tweet, message,
document, or record was created — no separate metadata required. Each scheme hides the
timestamp in different bits with a different epoch.

## Snowflake (Twitter/X and Discord) {#snowflake}

A 64-bit ID with a millisecond timestamp in the high bits; the low 22 bits are
machine/sequence, so `id >> 22` is milliseconds since a custom epoch.

| Scheme | Epoch (ms) | Epoch (UTC) | Extraction |
|---|---|---|---|
| **Twitter/X** | `1288834974657` | 2010-11-04T01:42:54.657Z | `(id >> 22) + 1288834974657` |
| **Discord** | `1420070400000` | 2015-01-01T00:00:00Z | `(id >> 22) + 1420070400000` |

Sources: [Twitter snowflake](https://github.com/twitter-archive/snowflake/blob/snowflake-2010/README.mkd)
(epoch is the source constant `twepoch`), [Discord developer docs — Snowflakes](https://discord.com/developers/docs/reference#snowflakes).

- **Worked example (Discord, from the docs):** `175928847299117063 >> 22 = 41944705396`,
  `+ 1420070400000 = 1462015105796` ms → **2016-04-30T11:18:25.796Z**.
- **Gotcha:** IDs are JSON strings to avoid 53-bit float truncation — treat as unsigned
  64-bit. The custom epoch must be known; the datacenter/worker bits can fingerprint the
  generating infrastructure. A small value decodes to ~the scheme's own epoch, which is
  implausible — `timeglyph`'s `magnitude_fit` score sinks such false reads.

## UUID (RFC 9562) {#uuid}

[RFC 9562](https://www.rfc-editor.org/rfc/rfc9562.txt) (which obsoletes RFC 4122)
defines several versions; three carry a timestamp:

### UUIDv1 / v6 — the 1582 Gregorian epoch

A **60-bit count of 100-nanosecond intervals since 1582-10-15** — the
[date of the Gregorian reform](../concepts/calendars.md) itself. The Gregorian↔Unix
offset is `0x01b21dd213814000 = 122192928000000000` (×100 ns). Extraction:
`unix_ms = (gregorian_100ns − 122192928000000000) / 10000`. **v6** is field-compatible
with v1 but stores the timestamp most-significant-first for database locality.

!!! note "MAC address leak"
    v1/v6 also include a 48-bit `node` field that is frequently the **real MAC address**
    of the generating host — high-value attribution, not just a timestamp.

### UUIDv7 — the modern, sortable one

The first **48 bits are a big-endian Unix millisecond timestamp**, followed by version,
variant, and randomness. Extraction: `unix_ms = int(uuid_hex[:12], 16)`.

- **Worked example (RFC test vector):** `017F22E2-79B0-7CC3-…` → `0x017F22E279B0 =
  1645557742000` ms → 2022-02-22T19:22:22Z.

(UUIDv4 is fully random and carries **no** timestamp. The version nibble — first hex digit
of the third group — identifies the scheme.)

## ULID {#ulid}

128 bits = **48-bit Unix-millisecond timestamp** + 80-bit randomness, Crockford base32,
26 chars, lexicographically sortable. The first **10 characters** encode the timestamp.
Source: [ULID spec](https://github.com/ulid/spec).

- **Gotcha:** lexical sort = chronological sort, so ordering alone reveals relative
  creation order. Same 48-bit-ms front as UUIDv7.

## MongoDB ObjectId {#objectid}

12 bytes: a **4-byte big-endian Unix-seconds timestamp** + 5-byte random + 3-byte
counter. Source: [MongoDB ObjectId](https://www.mongodb.com/docs/manual/reference/method/ObjectId/).
Extraction: `unix_seconds = int(hex[0:8], 16)`.

- **Gotcha:** the first 4 bytes of any `_id` leak document creation time to **1-second**
  resolution — the canonical "when was this row inserted" artifact. The 3-byte counter
  gives intra-second insertion order; legacy (pre-3.4) ObjectIds embedded machine-id +
  PID instead of the random field.

## KSUID {#ksuid}

20 bytes = **4-byte big-endian seconds timestamp** + 16-byte payload, 27-char base62,
with a **custom epoch of `1400000000`** (2014-05-13T16:53:20Z). Source:
[Segment KSUID](https://github.com/segmentio/ksuid). Extraction:
`unix_seconds = big_endian_4_bytes + 1400000000`.

- **Gotcha:** a raw Unix-epoch interpretation is wrong by ~44.4 years — the `1400000000`
  offset must be added.

## Sonyflake {#sonyflake}

A Snowflake variant: **39-bit time in 10-ms units** + 8-bit sequence + 16-bit machine id,
default epoch **2014-09-01** (`1409529600000` ms). Source:
[sony/sonyflake](https://github.com/sony/sonyflake). Extraction:
`unix_ms = (id >> 24) * 10 + 1409529600000`.

- **Gotcha:** coarser **10-ms** resolution; the machine id defaults to the lower 16 bits
  of the host's private IPv4 address (partial host leak). Epoch, time unit, and bit split
  are configurable — confirm against the generating code.

## Cross-scheme summary

| Scheme | TS bits | Resolution | Epoch (UTC) | Extraction core |
|---|---|---|---|---|
| Twitter Snowflake | 41 | 1 ms | 2010-11-04T01:42:54.657Z | `(id>>22)+epoch` |
| Discord Snowflake | 42 | 1 ms | 2015-01-01 | `(id>>22)+epoch` |
| UUIDv1 / v6 | 60 | 100 ns | 1582-10-15 (Gregorian) | `(gts−offset)/10000` |
| UUIDv7 | 48 | 1 ms | 1970-01-01 | first 48 bits |
| ULID | 48 | 1 ms | 1970-01-01 | first 10 base32 chars |
| MongoDB ObjectId | 32 | 1 s | 1970-01-01 | first 4 bytes (BE) |
| KSUID | 32 | 1 s | 2014-05-13 | `BE4bytes + 1400000000` |
| Sonyflake | 39 | 10 ms | 2014-09-01 | `(id>>24)*10 + epoch` |

**Highest attribution value:** UUIDv1/v6 `node` (often real MAC); Sonyflake machine id
(private IP low bits); ObjectId per-process random (legacy: machine-id + PID).

## See also

- [References](../references.md), [Methodology](../concepts/methodology.md) — how
  `magnitude_fit` keeps epoch-hugging ID false reads ranked low.
