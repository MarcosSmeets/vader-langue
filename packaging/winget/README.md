# winget (Windows)

`winget` installs the native Windows build (`vader.exe`) published on a GitHub Release
(the `.github/workflows/release.yml` workflow produces `vader-windows-x86_64.exe`).

Filled-in manifests for the current release live in
[`manifests/m/Marco/Vader/0.4.0/`](manifests/m/Marco/Vader/0.4.0/) — the standard three:
`Marco.Vader.yaml` (version), `Marco.Vader.installer.yaml` (URL + `InstallerSha256`), and
`Marco.Vader.locale.en-US.yaml` (metadata). They point at the real `v0.4.0` asset and its
SHA-256.

## Publishing to winget

Submit the manifests via a PR to [microsoft/winget-pkgs]. The easiest way to (re)generate
them for a new release is `wingetcreate`, which fills the SHA-256 and opens the PR for you:

```powershell
winget install wingetcreate
wingetcreate new https://github.com/MarcosSmeets/vader-langue/releases/download/v0.4.0/vader-windows-x86_64.exe
```

Once accepted, users install with:

```powershell
winget install Marco.Vader
```

## Refreshing for a new release

Each `v*` tag triggers `.github/workflows/release.yml`, which builds the binaries, publishes
a `<asset>.sha256` for each, and **regenerates these manifests + the Homebrew formula**
(`packaging/bump.sh`) committed back to `main`. To push the update into the winget catalog,
open a PR to [microsoft/winget-pkgs] with the new version folder (or rerun
`wingetcreate update Marco.Vader`).

> Note: the GitHub release tag (`v0.4.0`) is the package version; the compiler's own
> `vader version` reports its internal number independently.

[microsoft/winget-pkgs]: https://github.com/microsoft/winget-pkgs
