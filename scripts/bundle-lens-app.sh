#!/usr/bin/env bash
# Wrap the timeglyph-lens macOS binary in a .app bundle (icon + Info.plist) so the
# GUI ships as a Homebrew Cask — a clickable app in /Applications (Launchpad /
# Spotlight / Dock), not a bare CLI on $PATH. A stable CFBundleIdentifier also
# gives the Accessibility-permission grant (the AX element picker) a stable
# identity across versions.
#
# Usage: bundle-lens-app.sh <lens-binary> <cli-binary> <version> [output-dir]
#   -> writes "<output-dir>/TimeGlyph Lens.app"  (default output-dir: cwd)
# The bundle uses the display-style name macOS GUI apps conventionally carry
# ("TimeGlyph Lens.app", like "Google Chrome.app"); the executable inside stays
# the hyphenated CLI name (timeglyph-lens).
#
# The `timeglyph` CLI is bundled ALONGSIDE the GUI in Contents/MacOS. That lets the
# cask expose both via `binary` stanzas and drop `depends_on formula:` — which is what
# makes `brew install --cask …` a genuine ONE-command install: Homebrew 6 refuses to
# load a formula the user did not name (an indirect `depends_on`) from an untrusted
# tap, aborting the whole install, whereas the explicitly-named cask is trusted. Cost:
# the CLI is duplicated (it also ships in the formula and the tarball).
set -euo pipefail

BIN="$1"
CLI="$2"
VERSION="$3"
OUTDIR="${4:-.}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICON_PNG="$REPO_ROOT/lens/assets/icon.png"

# Fail loud: a bundle missing the CLI would silently produce a cask that installs no
# `timeglyph` at all (the cask no longer depends on the formula to supply it).
[ -f "$BIN" ] || { echo "error: lens binary not found: $BIN" >&2; exit 1; }
[ -f "$CLI" ] || { echo "error: CLI binary not found: $CLI" >&2; exit 1; }

APP="$OUTDIR/TimeGlyph Lens.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN" "$APP/Contents/MacOS/timeglyph-lens"
chmod +x "$APP/Contents/MacOS/timeglyph-lens"
cp "$CLI" "$APP/Contents/MacOS/timeglyph"
chmod +x "$APP/Contents/MacOS/timeglyph"

# Multi-resolution .icns from the 256px source (no upscaling past the source).
ICONSET="$(mktemp -d)/icon.iconset"
mkdir -p "$ICONSET"
for s in 16 32 64 128 256; do
  sips -z "$s" "$s" "$ICON_PNG" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
done
sips -z 32 32 "$ICON_PNG" --out "$ICONSET/icon_16x16@2x.png" >/dev/null
sips -z 64 64 "$ICON_PNG" --out "$ICONSET/icon_32x32@2x.png" >/dev/null
sips -z 256 256 "$ICON_PNG" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/timeglyph-lens.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>timeglyph-lens</string>
  <key>CFBundleName</key><string>TimeGlyph Lens</string>
  <key>CFBundleDisplayName</key><string>TimeGlyph Lens</string>
  <key>CFBundleIdentifier</key><string>com.securityronin.timeglyph-lens</string>
  <key>CFBundleIconFile</key><string>timeglyph-lens</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
</dict>
</plist>
PLIST

plutil -lint "$APP/Contents/Info.plist" >/dev/null
echo "built $APP (v${VERSION})"
