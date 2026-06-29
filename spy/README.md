# timeglyph-spy

A Spy++-style **Windows** inspector for [timeglyph](../). Hover any UI element
and, if its text contains a number, see timeglyph's ranked datetime readings —
right where the cursor is.

```text
element: lastVisitTime  13390845530064940
13390845530064940
    iostime      2001-06-04T23:40:45.53006494Z  (Apple NSDate iOS 11+)
    webkit       2025-05-04T15:18:50.06494Z     (Chrome / WebKit µs since 1601)
```

## Modes

- **Live (Windows, no args)** — opens a small always-on-top window that follows
  the cursor. It uses UI Automation (`IUIAutomation::ElementFromPoint`) to read
  the element under the pointer (native, Win32, WPF, browsers, Electron), scans
  its text for long numbers, and shows the top readings via the timeglyph engine.

  ```powershell
  timeglyph-spy
  ```

- **Text (any platform)** — decode the numbers in an argument string. This is
  how the cross-platform scan core is exercised without a desktop:

  ```bash
  timeglyph-spy "cookie value 13390845530064940 and ts 1577836800"
  ```

## Architecture (Humble Object)

| Module | Platform | Role |
|--------|----------|------|
| `scan` | all | extract long numbers → `timeglyph::interpret::interpret_int` → top in-window readings. **Unit-tested.** |
| `picker` | Windows | `IUIAutomation` element-under-cursor → its text |
| `overlay` | Windows | always-on-top window + cursor timer, refreshing a label |

All the logic lives in the testable `scan` core; the Win32/UIA shell is thin and
behind `#[cfg(windows)]`, so the library and `scan` tests build on every
platform while the live inspector is compiled on Windows.

## Build

```bash
cargo build --release            # on Windows: full live inspector
cargo run -- "ts 1577836800"     # text mode, any platform
```

This crate is a companion tool (`publish = false`); it is excluded from the
timeglyph crate published to crates.io.

---

[Privacy Policy](https://securityronin.github.io/timeglyph/privacy/) · [Terms of Service](https://securityronin.github.io/timeglyph/terms/) · © 2026 Security Ronin Ltd
