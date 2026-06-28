---
title: "CSV enrichment — add human-readable timestamp columns"
description: >-
  Enrich a CSV with timeglyph: auto-detect numeric timestamp columns or convert
  named columns under a chosen format, writing a human-readable column to the
  right (or replacing the original).
---

# CSV enrichment

`timeglyph csv` reads a CSV and writes it back with timestamp columns rendered to
human-readable UTC. It works two ways, which can be combined.

## Auto-detect (default)

With no other options, every **numeric** column is examined and, when it
confidently looks like one timestamp format, a rendered column is appended to its
right:

```console
$ timeglyph csv events.csv
name,ts,ts_unix,count
a,1577836800,2020-01-01T00:00:00Z,5
```

Auto-detection is deliberately conservative — it only enriches a column when:

- every non-empty cell is an integer **at or above ~10⁸** (so counts, ids, and
  years are skipped — `count` above is left untouched), and
- the cells share **one** confident, in-window, non-[sentinel](concepts/sentinel-values.md)
  top interpretation.

If a column is ambiguous, it is left alone rather than guessed — consistent with
the engine's "never a single verdict" stance.

## Explicit conversion

Convert a specific column under a chosen [format](formats/index.md) with
`--convert COLUMN:FORMAT` (repeatable):

```console
$ timeglyph csv files.csv --convert created:filetime --convert modified:unix
id,created,created_filetime,modified,modified_unix
1,132223104000000000,2020-01-01T00:00:00Z,1577836800,2020-01-01T00:00:00Z
```

The new column is named `<column>_<format>` and sits immediately to the right of
its source.

## Replace in place

Use `--replace` to overwrite the source column instead of adding one:

```console
$ timeglyph csv files.csv --convert created:filetime --replace
id,created
1,2020-01-01T00:00:00Z
```

## Options

| Option | Effect |
|---|---|
| *(none)* | auto-detect numeric timestamp columns |
| `--convert COL:FMT` | convert column `COL` under format `FMT` (repeatable) |
| `--auto` | force auto-detect in addition to `--convert` |
| `--replace` | overwrite the source column instead of adding one |
| `-o, --output FILE` | write to `FILE` (default: stdout) |
| `path` of `-` | read the CSV from stdin |

## See also

- [Format reference](formats/index.md) · [Input conventions](concepts/input-conventions.md)
  · [Sentinel values](concepts/sentinel-values.md)
