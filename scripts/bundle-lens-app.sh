#!/usr/bin/env bash
# Wrap the timeglyph-lens macOS binary in a .app bundle (icon + Info.plist) so the
# GUI ships as a Homebrew Cask — a clickable app in /Applications (Launchpad /
# Spotlight / Dock), not a bare CLI on $PATH. A stable CFBundleIdentifier also
# gives the Accessibility-permission grant (the AX element picker) a stable
# identity across versions.
#
# Usage: bundle-lens-app.sh <lens-binary> <version> [output-dir]
#   -> writes <output-dir>/timeglyph-lens.app  (default output-dir: cwd)
set -euo pipefail

BIN="$1"
VERSION="$2"
OUTDIR="${3:-.}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICON_PNG="$REPO_ROOT/lens/assets/icon.png"

APP="$OUTDIR/timeglyph-lens.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN" "$APP/Contents/MacOS/timeglyph-lens"
chmod +x "$APP/Contents/MacOS/timeglyph-lens"

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
