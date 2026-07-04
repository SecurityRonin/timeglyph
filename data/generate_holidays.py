#!/usr/bin/env python3
"""Regenerate data/holidays.json.gz — timeglyph's embedded holiday dataset.

Source of truth: the MIT-licensed python-holidays project (https://github.com/
vacanza/holidays). This script exports every canonical (ISO-3166 alpha-2)
country it supports, for the years 1980–2100, into a compact gzipped JSON map:

    { "US": { "2020-07-04": "Independence Day", ... }, ... }

Names are in each country's DEFAULT locale, exactly as python-holidays emits
them (so e.g. CN holidays are in Chinese). Coverage varies by country:
python-holidays supports different year ranges per locale and silently clips the
request, so some countries carry fewer years than 1980–2100.

Reproducibility: keys are sorted so the output is byte-stable for a given
python-holidays version. Record that version in data/README.md when you rerun.

    pip install holidays
    python3 data/generate_holidays.py
"""
import gzip
import json
import pathlib

import holidays

Y0, Y1 = 1980, 2100
OUT = pathlib.Path(__file__).with_name("holidays.json.gz")


def main() -> None:
    codes = sorted(c for c in holidays.list_supported_countries() if len(c) == 2)
    data: dict[str, dict[str, str]] = {}
    for code in codes:
        try:
            h = holidays.country_holidays(code, years=range(Y0, Y1 + 1))
        except Exception:  # noqa: BLE001 — skip a country the lib can't build
            continue
        by_date = {d.isoformat(): name for d, name in h.items()}
        if by_date:
            data[code] = dict(sorted(by_date.items()))

    raw = json.dumps(data, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    OUT.write_bytes(gzip.compress(raw.encode("utf-8"), 9))
    entries = sum(len(v) for v in data.values())
    print(f"holidays {holidays.__version__}: {len(data)} countries, "
          f"{entries} dates, {OUT.stat().st_size} bytes -> {OUT}")


if __name__ == "__main__":
    main()
