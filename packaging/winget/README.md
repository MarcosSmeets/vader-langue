# winget (Windows) — template

`winget` requer um build **nativo de Windows** (`vader.exe`) publicado numa release do
GitHub (o workflow `.github/workflows/release.yml` já gera `vader-windows-x86_64.exe`).
A submissão é via PR no repo [microsoft/winget-pkgs], com 3 manifestos. Abaixo o
**installer manifest** (portable) como ponto de partida — preencha `Sha256` e URLs.

```yaml
# manifests/m/Marco/Vader/0.1.0/Marco.Vader.installer.yaml
PackageIdentifier: Marco.Vader
PackageVersion: 0.1.0
InstallerType: portable
Commands:
  - vader
Installers:
  - Architecture: x64
    InstallerUrl: https://github.com/MarcosSmeets/vader-langue/releases/download/v0.1.0/vader-windows-x86_64.exe
    InstallerSha256: PREENCHER_SHA256
ManifestType: installer
ManifestVersion: 1.6.0
```

Também precisa de um `Marco.Vader.yaml` (version) e um
`Marco.Vader.locale.en-US.yaml` (metadados). O jeito mais fácil de gerar tudo certo:

```powershell
winget install wingetcreate
wingetcreate new https://github.com/MarcosSmeets/vader-langue/releases/download/v0.1.0/vader-windows-x86_64.exe
```

O `wingetcreate` monta os 3 manifestos e abre o PR pro winget-pkgs.

[microsoft/winget-pkgs]: https://github.com/microsoft/winget-pkgs
