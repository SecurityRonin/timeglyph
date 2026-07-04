#!/usr/bin/env python3
"""Regenerate data/zone_country.json — IANA time-zone → ISO-3166 country.

Two public-domain sources from the IANA tz database:
  * zone.tab  — canonical zone → country, read from the local tzdata install.
  * backward  — `Link <canonical> <alias>` lines mapping legacy aliases (e.g.
    Asia/Chongqing → Asia/Shanghai) to their canonical zone, fetched from the
    IANA tz repo. Without these, an alias resolves to no country and its
    holidays never show (jiff still renders the time for it, so users hit it).

    pip is not needed; only network access to the backward file.
    python3 data/generate_zone_country.py
"""
import json
import pathlib
import urllib.request

ZONE_TAB = "/usr/share/zoneinfo/zone.tab"
BACKWARD_URL = "https://raw.githubusercontent.com/eggert/tz/main/backward"
OUT = pathlib.Path(__file__).with_name("zone_country.json")


def main() -> None:
    canonical: dict[str, str] = {}
    for line in open(ZONE_TAB, encoding="utf-8"):
        if line.startswith("#") or not line.strip():
            continue
        fields = line.rstrip("\n").split("\t")
        if len(fields) >= 3 and len(fields[0]) == 2:
            canonical[fields[2]] = fields[0]

    zones = dict(canonical)
    backward = urllib.request.urlopen(BACKWARD_URL, timeout=30).read().decode("utf-8")
    for line in backward.splitlines():
        if not line.startswith("Link"):
            continue
        parts = line.split("#")[0].split()  # Link <canonical> <alias>
        if len(parts) >= 3:
            target, alias = parts[1], parts[2]
            cc = canonical.get(target)
            if cc and alias not in zones:
                zones[alias] = cc

    OUT.write_text(json.dumps(dict(sorted(zones.items())), ensure_ascii=False, separators=(",", ":")))
    print(f"{len(zones)} zones ({len(zones) - len(canonical)} aliases) -> {OUT}")


if __name__ == "__main__":
    main()
