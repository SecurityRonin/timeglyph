# macOS code signing + notarization (for the Homebrew Cask)

The `TimeGlyph Lens.app` shipped via the Homebrew Cask **must** be Developer-ID-signed
and notarized, or macOS Gatekeeper blocks it ("app is damaged") and Homebrew drops the
cask (unsigned casks lose support 2026-09-01). See ronin-issen ADR 0002. The CLI
(`brew install timeglyph`, a Formula) needs none of this.

`release.yml` already has the signing/notarization/stapling steps — they are **gated on
the secrets below** (`HAS_MACOS_SIGNING`), so CI stays green until they exist. The
moment all six secrets are set, the next release signs the `.app` automatically.

## One-time setup

### 1. Apple Developer Program membership ($99/yr)

Enroll as the **organization "Security Ronin Ltd"** (matches the Windows verified
publisher). Org enrollment needs a **D-U-N-S number** (free from Dun & Bradstreet,
~1–5 business days) + legal-entity verification. Note the **Team ID** (10 chars, e.g.
`AB12CD34EF`) from developer.apple.com → Membership.

### 2. Developer ID Application certificate

developer.apple.com → Certificates → **+** → **Developer ID Application** → follow the
CSR flow (Keychain Access → Certificate Assistant → Request a Certificate from a CA).
Install the issued cert, then in Keychain Access **export the cert + its private key as
a `.p12`** (set a strong password). The identity string is
`Developer ID Application: Security Ronin Ltd (TEAMID)`.

### 3. Notarization API key (App Store Connect)

appstoreconnect.apple.com → Users and Access → **Integrations → App Store Connect API**
→ **+** → role **Developer** (or Admin) → download the **`AuthKey_XXXX.p8`** (once
only). Note the **Key ID** and the **Issuer ID** (a UUID on that page).

### 4. Add the six GitHub secrets

Repo → Settings → Secrets and variables → Actions → New repository secret:

| Secret | Value |
|---|---|
| `MACOS_CERT_P12_BASE64` | `base64 -i cert.p12` (the exported `.p12`, base64-encoded) |
| `MACOS_CERT_PASSWORD` | the `.p12` export password |
| `MACOS_SIGN_IDENTITY` | `Developer ID Application: Security Ronin Ltd (TEAMID)` |
| `MACOS_NOTARY_KEY_P8_BASE64` | `base64 -i AuthKey_XXXX.p8` |
| `MACOS_NOTARY_KEY_ID` | the API Key ID |
| `MACOS_NOTARY_ISSUER_ID` | the API Issuer ID (UUID) |

```sh
# base64 helpers (macOS)
base64 -i cert.p12 | pbcopy            # → MACOS_CERT_P12_BASE64
base64 -i AuthKey_XXXX.p8 | pbcopy     # → MACOS_NOTARY_KEY_P8_BASE64
```

## What the CI does (per release, macOS legs)

1. Build `TimeGlyph Lens.app` (`scripts/bundle-lens-app.sh`).
2. Import the `.p12` into a temporary keychain; `codesign --options runtime --timestamp`
   with the Developer ID identity; `codesign --verify`.
3. `xcrun notarytool submit … --wait` (Apple scans it, ~1–5 min).
4. `xcrun stapler staple` (embeds the ticket for offline Gatekeeper checks).
5. `ditto`-zip the stapled `.app` → `timeglyph-lens-<ver>-<target>.app.zip`.

## Activation

Until the secrets exist, the `.app.zip` ships **unsigned** and the cask is held as a
**draft** (`SecurityRonin/homebrew-tap` PR). Once the secrets are set and a release
signs the `.app`, mark the cask PR ready — then `brew install --cask timeglyph-lens`
installs a clean, Gatekeeper-passing app.

Verify a shipped release locally:

```sh
codesign --verify --deep --strict --verbose=2 "/Applications/TimeGlyph Lens.app"
spctl -a -t exec -vvv "/Applications/TimeGlyph Lens.app"   # → "accepted, source=Notarized Developer ID"
xcrun stapler validate "/Applications/TimeGlyph Lens.app"
```
