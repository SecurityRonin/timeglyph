# GUI screenshot validation on Windows — launch the REAL timeglyph-lens window,
# capture just that window, and assert it renders meaningful content (not the
# all-black frame a dropped-font atlas silently produces).
#
# Needs an INTERACTIVE desktop session (e.g. autologon on the GCP/native-x86 test
# VM). A service/headless session has no desktop for the window to appear on, so
# FindWindow returns null. macOS/Linux: use scripts/screenshot-validate.sh.
#
# Writes the PNG to $env:OUT (default %TEMP%\lens-shot.png) and exits non-zero if
# the frame is all-black/uniform.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$out  = if ($env:OUT) { $env:OUT } else { Join-Path $env:TEMP 'lens-shot.png' }
$lens      = Join-Path $repo 'lens\target\release\timeglyph-lens.exe'
$shotcheck = Join-Path $repo 'lens\target\release\examples\shotcheck.exe'

Write-Host '==> building lens + shotcheck (release)'
cargo build --release --manifest-path (Join-Path $repo 'lens\Cargo.toml')
cargo build --release --example shotcheck --manifest-path (Join-Path $repo 'lens\Cargo.toml')

Add-Type @'
using System;
using System.Runtime.InteropServices;
public class Win {
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string name);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public struct RECT { public int Left, Top, Right, Bottom; }
}
'@

$proc = Start-Process $lens -PassThru
try {
  Start-Sleep -Seconds 4
  $h = [Win]::FindWindow($null, 'TimeGlyph Lens')
  if ($h -eq [IntPtr]::Zero) {
    throw "lens window 'TimeGlyph Lens' not found — is this an interactive desktop session?"
  }
  $r = New-Object 'Win+RECT'
  [void][Win]::GetWindowRect($h, [ref]$r)
  $w = $r.Right - $r.Left
  $ht = $r.Bottom - $r.Top
  Write-Host "==> lens window at $($r.Left),$($r.Top) size ${w}x${ht}"

  Add-Type -AssemblyName System.Drawing
  $bmp = New-Object System.Drawing.Bitmap $w, $ht
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
  $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  Write-Host "==> saved $out"
}
finally {
  $proc | Stop-Process -Force -ErrorAction SilentlyContinue
}

& $shotcheck $out
exit $LASTEXITCODE
