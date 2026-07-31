---
title: "timeglyph-lens — the cursor-hover timestamp overlay"
description: >-
  timeglyph-lens is an always-on-top overlay that follows your cursor and decodes
  any number under the pointer into ranked timestamp readings — with weekday,
  public holiday, and Chinese 干支 pillars. macOS and Windows.
---

# timeglyph-lens — the cursor overlay

`timeglyph-lens` is the interactive front-end to the [timeglyph](index.md) engine.
It is an always-on-top window that follows your cursor and shows timeglyph's
ranked readings for any number in the UI element under the pointer — no
copy-paste, no switching windows. Point at a Unix time in a log, a FILETIME in a
registry viewer, or a column in a database browser, and the decodings appear.

<p align="center">
  <img src="assets/lens-demo.gif" alt="timeglyph-lens decoding a timestamp live as the cursor hovers a value on screen" width="720" />
</p>

## What each reading shows

Every candidate is a row in a compact instrument panel:

- **Confidence** — a red/amber/green dot and a percentage from the engine's
  plausibility score (the same scoring as the CLI; hover for the component
  breakdown).
- **Format** — the amber chip (e.g. `unix`, `webkit`, `cocoa`); hover for the full
  format name.
- **Instant** — rendered in the chosen display timezone.
- **Weekday** — the day of week of the shown date.
- **Public holiday** — if that date is a public holiday in the display zone's
  country, its name (see [Public holidays](holidays.md)). Only for a named IANA
  zone; *consistent with* a holiday, an annotation, not proof.
- **干支 pillars** (opt-in) — the Chinese lunisolar date and the four
  Heavenly-Stem / Earthly-Branch pillars, stems over branches, each spot-colored
  by its 五行 (Five Element), with the day branch ringed. Enable it in Settings.

<p align="center">
  <img src="assets/lens.png" alt="the timeglyph-lens overlay up close" width="460" />
</p>

## Display timezone

The footer selects how instants are rendered: **UTC** (the default), **Local**,
a **fixed offset** or IANA zone via **Region / Zone…**, or a click on the **🌐**
world map. UTC-anchored readings shift into the chosen zone (with an explicit
offset); naive wall-clock readings are shown as-is and tagged *local time (not
time-zone adjusted)*. The zone is session-scoped and never persisted — a prior
case's zone can't silently carry into the next launch.

## Decoding the clipboard

The cursor picker reads the UI element under the pointer through the platform
accessibility layer, and some values are simply not in that tree. A **VM guest
window** is one opaque framebuffer to the host. So are a canvas, a rendered image, a
remote-desktop session, and a PDF page drawn as graphics. Hovering those yields
nothing, however the picker is configured.

The clipboard crosses those boundaries. Copy the value, then press the header **🗐**
button and the Lens decodes it into the same ranked readings, with the source shown
as *clipboard*.

Reading is **pull-based**: the clipboard is read once, when you press the button.
Nothing watches or polls it, so there is no background access to your pasteboard and
no surveillance state for the overlay to disclose — the press is the consent
boundary. If the clipboard is empty, holds non-text, or holds nothing that decodes,
the current readings are left alone rather than cleared.

One thing to know before you press it: the **caption** never shows clipboard text —
the type carrying the source is structurally incapable of holding it, so the raw
contents cannot be drawn or logged. But a value the engine *recognises* is printed as
the reading's subject, because that is the point of the button. A copied bearer token
or recovery code that happens to decode will appear on screen, so press it
deliberately.

The button is greyed out when the platform has no clipboard to open (a headless host,
or no window server).

## The header controls

| Control | What it does |
|---|---|
| **🗐** | Decode the clipboard once (see above) |
| **⏸** / **▶** | Freeze the reading so it stops following the cursor while you read or copy it. **Space** toggles the same state |
| **⚙** | Open Settings |

## Settings

The **⚙** button (and, on macOS, the ⌘, menu item) opens Settings:

- **Theme** — dark (default) or light; both palettes clear WCAG AA.
- **Chinese lunisolar / 干支** — turns on the 干支 pillars and the longitude
  input (which refines only the hour pillar to true solar time).

## Platforms & permissions

| Platform | Status | Picker |
|---|---|---|
| macOS | supported | Accessibility API (`AXUIElementCopyElementAtPosition`) |
| Windows | supported | UI Automation (`IUIAutomation::ElementFromPoint`) |
| Linux (X11) | supported | AT-SPI (`GetAccessibleAtPoint`, descended to the deepest element) |

On macOS the picker narrows to the exact word under the cursor; on Windows it
reads the hovered element's name; on Linux it descends the AT-SPI tree to the
deepest element under the X11 pointer and reads its Text interface (falling back
to the accessible name).

Linux needs **assistive technologies enabled** so the AT-SPI accessibility bus is
reachable (the Lens reports the bus as unavailable if not), and the picker reads the
**X11** pointer — so run it under X11, not a pure Wayland session.

### First launch on macOS — grant Accessibility

macOS gates the Accessibility API behind an explicit grant. On first launch
timeglyph-lens triggers the system prompt; until you allow it, the overlay shows a
**"Grant Accessibility to timeglyph-lens"** reminder and no readings appear. To
grant it:

1. Open **System Settings → Privacy & Security → Accessibility**.
2. Turn on **timeglyph-lens** (click **+** and add it if it isn't listed).
3. Quit and relaunch timeglyph-lens.

### macOS — the overlay cannot appear over a full-screen app

When another application occupies a full-screen Space (the green-button full
screen, not a maximised window), the overlay is invisible there. It is not merely
behind the full-screen window: macOS omits it from that Space's window list
altogether, alongside every other third-party window, including menu-bar accessory
windows and windows raised to `NSScreenSaverWindowLevel`. The only overlays macOS
admits onto a full-screen Space are its own — such as the Hover Text accessibility
window, which carries a private entitlement. Getting a third-party window in
requires private CGS/SkyLight calls and a weakened SIP, which is not an acceptable
trade on an evidence workstation, so treat this as a platform limit rather than a
pending fix. (Measured on macOS 15.7.8.)

Two ways to read a value that sits behind a full-screen app:

- Run the app you are reading **windowed**. Hover works as documented, and for
  anything the picker cannot reach, copy the value and press **🗐** — see
  [Decoding the clipboard](#decoding-the-clipboard).
- Keep it full screen and use the CLI on the copied value:

  ```bash
  pbpaste | timeglyph scan            # macOS
  xclip -o | timeglyph scan           # Linux (X11)
  Get-Clipboard | timeglyph scan      # Windows PowerShell
  ```

  The clipboard crosses a VM boundary that the accessibility picker does not, so
  this also works for a value inside a full-screen guest.

### Windows

No special permission is needed. To inspect an **elevated** (Run-as-administrator)
window, run timeglyph-lens elevated too — Windows isolates UI access across
privilege levels (UIPI).

## Install

A single install brings both the `timeglyph` CLI and the `timeglyph-lens` overlay:

- **Windows:** `winget install SecurityRonin.timeglyph`.
- **macOS:** `brew install --cask securityronin/tap/timeglyph-lens` — installs
  `TimeGlyph Lens.app` into `/Applications` (Launchpad/Spotlight/Dock) and puts both the
  overlay and the `timeglyph` CLI on your `PATH`. The CLI ships inside the app bundle, so
  the cask needs no formula dependency — and therefore no `brew trust` step.

Or download the `timeglyph-lens` binary from the
[latest release](https://github.com/SecurityRonin/timeglyph/releases/latest), or
build from source:

```bash
cargo build --release --manifest-path lens/Cargo.toml
```

Grant Accessibility permission on first launch.
