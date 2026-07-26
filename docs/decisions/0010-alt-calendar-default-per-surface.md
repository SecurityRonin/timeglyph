# 0010 — Alternative-calendar visibility default differs by surface

Status: Accepted

## Context

A decoded date can be annotated with alternative calendars — 中華民國 (ROC),
和曆 (Japanese era), Buddhist, Hebrew, Islamic (tabular civil), and Persian
(Solar Hijri). Two surfaces render them, and each has a different default for how
many show before the user configures anything:

- the **`timeglyph-lens` GUI overlay**, an interactive tool the user leaves open
  and explores, and
- the **`timeglyph cal` CLI command**, a terse, scriptable one-shot.

The question is what a user sees with zero configuration.

## Decision

The default is deliberately opposite on the two surfaces:

- **Lens GUI — opt-out (all six ON).** `CalendarVisibility::default()` and the
  per-field `#[serde(default = "enabled")]` are all `true`
  (`lens/src/settings.rs`), so a fresh user sees every calendar and *hides* the
  ones they don't want via the visible Settings checkboxes. A persisted
  `settings.json` overrides this, so a returning user sees whatever subset they
  last saved.
- **CLI `cal` — opt-in (none).** The command renders no alternative calendars
  until the user names them with `--calendars`.

The rationale is the persona (cf. "Design for the Human Using It"): the GUI is
exploratory and its toggles are discoverable, so show everything and let the user
pare down. The CLI is composable and its output is often piped, so keep the
default compact and require an explicit flag to add columns.

## Consequences

- The two surfaces intentionally disagree on default visibility — this is by
  design, not a bug. A GUI showing "only 2 calendars" is a *persisted user
  choice*, never the default.
- Flipping either default is a one-line change, but would collapse the
  opt-out/opt-in split and the persona reasoning behind it.
- The lens still persists per-calendar choices; the CLI is stateless and reads
  `--calendars` each invocation.
