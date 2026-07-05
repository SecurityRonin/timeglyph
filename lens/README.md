# timeglyph-lens

A Spy++-style desktop inspector for [timeglyph](../), on **macOS and Windows**.
Hover any UI element and, if its text contains a number, see timeglyph's ranked
datetime readings — in an always-on-top window that follows your cursor.

```text
element: lastVisitTime  13390845530064940
13390845530064940
    iostime      2001-06-04T23:40:45.53006494Z  (Apple NSDate iOS 11+)
    webkit       2025-05-04T15:18:50.06494Z     (Chrome / WebKit µs since 1601)
```

## Modes

- **Live overlay (no args)** — an always-on-top egui window that follows the
  cursor, reads the element under it via the OS accessibility tree, scans its
  text for long numbers, and shows the top readings live.

  ```bash
  timeglyph-lens
  ```

- **Live console (`--live`)** — the same, printed to the terminal (no window).

- **Text (any platform)** — decode the numbers in an argument string; this is
  how the cross-platform scan core is exercised without a desktop:

  ```bash
  timeglyph-lens "cookie value 13390845530064940 and ts 1577836800"
  ```

> **macOS**: the live modes need **Accessibility** permission — grant your
> terminal (or the `timeglyph-lens` binary) access in *System Settings → Privacy
> & Security → Accessibility*. Without it, the element under the cursor reads as
> empty (no crash).

## Architecture (Humble Object)

| Module | Platform | Role |
|--------|----------|------|
| `scan` | all | extract long numbers → `timeglyph::interpret::interpret_int` → top in-window readings. **Unit-tested.** |
| `picker` | macOS (`AXUIElementCopyElementAtPosition`) · Windows (`IUIAutomation::ElementFromPoint`) | element under cursor → its text |
| `overlay` | all (eframe/egui) | always-on-top window, polls the picker, renders readings |

All the logic lives in the testable `scan` core; the accessibility picker is the
only platform-specific code, and the overlay is one cross-platform egui window.

## Build

```bash
cargo run                        # the always-on-top overlay
cargo run -- --live              # console inspector
cargo run -- --help              # usage; -V/--version for the version
cargo build --release
```

This crate is a companion tool (`publish = false`); it is excluded from the
timeglyph crate published to crates.io.

---

[Privacy Policy](https://securityronin.github.io/timeglyph/privacy/) · [Terms of Service](https://securityronin.github.io/timeglyph/terms/) · © 2026 Security Ronin Ltd
