# timeglyph DFIR integrations

Thin adapters that plug timeglyph into tools DFIR pros already use. Each is a
**thin adapter over one of two stable surfaces** — the CLI's JSON output
(`identify … --json`, `scan --json`) or the Python wheel (`pip install timeglyph`).
timeglyph stays the *time layer*: it never parses artifacts itself; you point it
at the values other tools surface.

| Adapter | Surface | What it does |
|---|---|---|
| [`velociraptor/`](velociraptor/) | CLI JSON | A `Custom.Timeglyph.Identify` artifact — `execve` the binary, parse the readings as VQL rows for live triage across a fleet. |
| [`kape/`](kape/) | CLI JSONL | A `.mkape` module — run `timeglyph scan --json` over module output as a post-process pass, enriching every collection. |
| [`timesketch/`](timesketch/) | Python wheel | An analyzer that re-interprets numeric fields and annotates events whose value decodes as a high-confidence timestamp (surfacing mis-parsed / unrecognised time fields in a super-timeline). |

## Install

- **Velociraptor**: import `velociraptor/Custom.Timeglyph.Identify.yaml` (or add
  it to the artifact exchange); deploy the `timeglyph` binary to endpoints.
- **KAPE**: drop `kape/timeglyph.mkape` in `KAPE\Modules\` and place
  `timeglyph.exe` on the module path.
- **Timesketch**: `pip install timeglyph`, copy
  `timesketch/timeglyph_analyzer.py` into the analyzers directory, register it.

Every reading these adapters surface carries timeglyph's ranked score and a spec
citation — the same auditable, ambiguity-first output as the CLI.
