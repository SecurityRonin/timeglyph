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

## Two packages

The release workflow ships two winget packages from the same tag:

| Identifier | Installs | Registered |
|---|---|---|
| `SecurityRonin.timeglyph` | the CLI (`timeglyph.exe`) | yes |
| `SecurityRonin.timeglyph-spy` | the GUI overlay (`timeglyph-spy.exe`) | pending first submission |

Each has its own `winget-releaser` job in `release.yml`, matched by a distinct
installer regex (`^timeglyph-\d…` vs `^timeglyph-spy-…`). `SecurityRonin.timeglyph-spy`
is **not yet registered**: its first-time manual submission (below) can only be
prepared once a `v0.3.0`+ release has produced
`timeglyph-spy-<ver>-x86_64-pc-windows-msvc.zip`, since the manifest needs that
asset's real `InstallerSha256`. Author its manifests under
`manifests/s/SecurityRonin/timeglyph-spy/<ver>/` modeled on the CLI set
(`PackageIdentifier: SecurityRonin.timeglyph-spy`, `PortableCommandAlias: timeglyph-spy`),
then submit the same way. After that, the `winget-spy` job updates it on every tag.

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
