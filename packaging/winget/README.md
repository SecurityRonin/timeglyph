# winget packaging

The Windows Package Manager (`winget`) hosts manifests in
[microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs), not in this
repo. The **first** version of `SecurityRonin.timeglyph` must be submitted there
**manually** (the automated `winget` job in `.github/workflows/release.yml` only
*updates* an already-registered package). After that first PR is merged, every
tagged release updates winget automatically — no manual steps.

The manifests under
[`manifests/s/SecurityRonin/timeglyph/0.2.0/`](manifests/s/SecurityRonin/timeglyph/0.2.0)
are the ready-to-submit initial set (zip-portable installer wrapping
`timeglyph.exe`).

## One-time submission

Prerequisites: a `securityronin-bot` fork of `microsoft/winget-pkgs`, and the
`WINGET_TOKEN` (a classic PAT for that bot with `public_repo`). Both are also what
the release workflow uses for ongoing updates.

**Validate (on Windows):**

```powershell
winget validate --manifest packaging\winget\manifests\s\SecurityRonin\timeglyph\0.2.0
```

**Submit — option A (wingetcreate):**

```powershell
wingetcreate submit --token <WINGET_TOKEN> packaging\winget\manifests\s\SecurityRonin\timeglyph\0.2.0
```

**Submit — option B (manual PR):** copy the `0.2.0/` directory into the
`securityronin-bot` fork at
`manifests/s/SecurityRonin/timeglyph/0.2.0/`, then open a PR to
`microsoft/winget-pkgs`.

Once merged, install with:

```powershell
winget install SecurityRonin.timeglyph
```

## Keeping these in sync

The committed manifests are a snapshot for the initial submission. For later
versions the workflow's `winget-releaser` step regenerates them from the release
`.zip`, so you do not need to edit these by hand — bump the `InstallerSha256` /
`PackageVersion` here only if you ever re-bootstrap from scratch.
