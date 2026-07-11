# timeglyph

**Decode, encode, and identify forensic timestamps — ranked, scored, and cited.**

Python bindings for [timeglyph](https://github.com/SecurityRonin/timeglyph), the Rust
engine that reads a raw timestamp value *every way a system might have written it* and
returns the results ranked by likelihood, each with the spec citation — honest about
the ambiguity instead of guessing one answer. Built for DFIR work in notebooks, plaso,
and Timesketch.

## Install

```bash
pip install timeglyph
```

A single self-contained extension (stable-ABI wheel, CPython 3.9+). No system
dependencies.

## Use

```python
import timeglyph

# One raw value → every plausible reading, ranked and cited:
for r in timeglyph.identify("133801920000000000"):
    print(f'{r["score"]:.2f}  {r["format_id"]:10}  {r["rendered"]}  ({r["citation"]})')
```

```
0.89  filetime    2025-01-01T08:00:00Z  ([MS-DTYP] §2.3.3 FILETIME)
0.70  ...
```

Each reading is a dict: `format_id`, `label`, `rendered`, `instant`, `score`,
`citation`, `assumptions`, `components`, `sentinel`. A raw value is usually
underdetermined, so `identify` returns *all* confident readings (16 for the example
above) rather than a single verdict — you decide which fits the artifact.

Need the raw payload for a pipeline? `timeglyph.identify_json(value)` returns the same
result as a JSON string.

## Learn more

- **Docs & the browser playground:** <https://securityronin.github.io/timeglyph/>
- **Source, CLI, and the live GUI overlay:** <https://github.com/SecurityRonin/timeglyph>

[Privacy Policy](https://securityronin.github.io/timeglyph/privacy/) · [Terms of Service](https://securityronin.github.io/timeglyph/terms/) · © 2026 Security Ronin Ltd
