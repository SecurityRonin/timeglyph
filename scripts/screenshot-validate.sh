#!/usr/bin/env bash
# GUI screenshot validation — launch the REAL timeglyph-lens window, capture just
# that window, and assert it renders meaningful content (not the all-black frame a
# dropped-font atlas silently produces; see lens/Cargo.toml's default_fonts note).
#
# A real desktop is required: the lens is picker-gated (macOS Accessibility /
# Linux AT-SPI), so it will not open headless — that is why this is a real-desktop
# script, not a CI job. Windows: use scripts/screenshot-validate.ps1.
#
#   macOS : captures the lens window via its Accessibility bounds + screencapture.
#           The controlling terminal needs Accessibility + Screen Recording
#           permission (System Settings > Privacy & Security).
#   Linux : runs the lens under Xvfb (needs xvfb, imagemagick, at-spi2) and
#           captures the virtual display.
#
# Writes the PNG to ${OUT:-/tmp/lens-shot.png} and exits non-zero if the frame is
# all-black/uniform — so it doubles as a validation gate on a real box.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${OUT:-/tmp/lens-shot.png}"
LENS="$REPO/lens/target/release/timeglyph-lens"
SHOTCHECK="$REPO/lens/target/release/examples/shotcheck"
LENS_PID=""
XVFB_PID=""
cleanup() {
  [[ -n "$LENS_PID" ]] && kill "$LENS_PID" 2>/dev/null || true
  [[ -n "$XVFB_PID" ]] && kill "$XVFB_PID" 2>/dev/null || true
}
trap cleanup EXIT

echo "==> building lens + shotcheck (release)"
cargo build --release --manifest-path "$REPO/lens/Cargo.toml" -q
cargo build --release --example shotcheck --manifest-path "$REPO/lens/Cargo.toml" -q

case "$(uname -s)" in
  Darwin)
    "$LENS" & LENS_PID=$!
    sleep 4
    # Window position (x,y) + size (w,h) from the Accessibility API.
    geom="$(osascript -e 'tell application "System Events" to tell (first process whose name is "timeglyph-lens") to get {position, size} of window 1' 2>/dev/null || true)"
    if [[ -z "$geom" ]]; then
      echo "!! could not read the lens window bounds — is the window open, and does"
      echo "   Terminal have Accessibility + Screen Recording permission?"
      echo "   Falling back to a full-screen capture (WEAKER: desktop content can"
      echo "   mask an all-black lens window)."
      screencapture -x "$OUT"
    else
      IFS=', ' read -r x y w h <<<"$geom"
      echo "==> lens window at ${x},${y} size ${w}x${h}"
      screencapture -x -R"${x},${y},${w},${h}" "$OUT"
    fi
    ;;
  Linux)
    command -v Xvfb  >/dev/null || { echo "need Xvfb (apt install xvfb)"; exit 3; }
    command -v import >/dev/null || { echo "need ImageMagick 'import' (apt install imagemagick)"; exit 3; }
    export DISPLAY=:99
    Xvfb :99 -screen 0 1280x800x24 >/dev/null 2>&1 & XVFB_PID=$!
    sleep 1
    # AT-SPI bus so the picker initialises (else Picker::new() fails, no window).
    eval "$(dbus-launch --sh-syntax)" 2>/dev/null || true
    /usr/libexec/at-spi-bus-launcher --launch-immediately >/dev/null 2>&1 || true
    sleep 1
    "$LENS" & LENS_PID=$!
    sleep 5
    # The lens is the only mapped window; capture the whole virtual display.
    import -window root "$OUT"
    ;;
  *)
    echo "unsupported OS $(uname -s); use scripts/screenshot-validate.ps1 on Windows"
    exit 3
    ;;
esac

echo "==> saved $OUT"
"$SHOTCHECK" "$OUT"
