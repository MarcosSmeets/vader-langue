# Vader — Distribution, VSCode, and Docker (plan)

> How people will **install/use** Vader, get the **VSCode extension**, and do
> **deployment via Docker**. A plan; not yet implemented. Status: `draft`.

---

## 1. Distribution (how people install and use it)

The `vader` compiler is a Rust binary. Distribution paths:

- **Pre-compiled binaries** per platform (linux/mac/windows), via **GitHub Releases**
  (`cargo build --release` + cross-compile). The user downloads it and puts it on the PATH.
- **Install script** rustup-style: `curl -fsSL https://.../install.sh | sh` (downloads the
  right binary for the OS/arch).
- **Package managers**: Homebrew (`brew install vader`), Scoop/winget (Windows),
  `cargo install vader` (if published on crates.io), later apt/AUR.
- **Versioning**: semver, `vader --version`, `vader upgrade`.

⚠️ **Today there is one dependency: the backend transpiles to Go**, so the user needs
**Go installed**. This goes away when the **LLVM backend** (Phase 3) is ready: then `vader`
becomes a **self-sufficient** toolchain (single binary, no Go). → distribution gets clean
**after LLVM**.

## 2. VSCode extension

Three layers, from easiest to most powerful — and each one **reuses what already exists**:

1. **Syntax highlighting** (quick, no compiler) — a TextMate grammar
   (`vader.tmLanguage.json`): keywords, types, strings, comments. Immediate visual payoff.
2. **Language Server (LSP)** — the killer move. We already have a **lexer + parser + checker with
   line:column errors** and **`vader fmt`**: that's exactly what an LSP needs. Plan:
   add a `vader lsp` mode (speaks the Language Server Protocol over stdio) that:
   - publishes **diagnostics** (type errors + architecture lint) on save/edit — we already compute this;
   - later: hover, go-to-definition, autocomplete;
   - **format on save** → calls `vader fmt`.
3. **VSCode extension** (TypeScript) — registers the `.vd` language + the grammar and brings up the
   LSP client connecting to `vader lsp`. Publish to the **Marketplace**.

> Suggested order: (1) highlighting → (2) `vader lsp` reusing the checker → (3) client extension.

## 3. Docker

Two uses, and the first is **a Vader trump card** (compiles to a static binary):

### a) Deploying apps built in Vader (tiny image)
`vader build` produces a **statically-linked ELF binary** (already proven). So the image
can be `scratch`/distroless, just a few MB:

```dockerfile
# build
FROM vader-toolchain AS build
WORKDIR /app
COPY . .
RUN vader build .

# runtime (minimal image)
FROM scratch
COPY --from=build /app/<project>/<project> /app
ENTRYPOINT ["/app"]
```

Idea: `vader new` could **generate the Dockerfile + .dockerignore** along with the project
(fits the scaffolding differentiator).

### b) Distributing the toolchain via Docker
A `vader` image (with vader + go for now) to use the compiler without installing anything:
`docker run --rm -v $PWD:/app vader build .`. Good for CI.

> After LLVM, the toolchain image gets smaller (no Go) and the app-image stays minimal.

## Recommended order

1. **LLVM backend** (next session) — removes the Go dependency and unlocks clean distribution.
2. **VSCode**: highlighting → `vader lsp` (reuses the checker) → extension.
3. **Docker**: `vader new` generates a Dockerfile; toolchain image for CI.
4. **Distribution**: releases + install script + Homebrew/winget.

---

# Implementation — current state (jun/2026)

LLVM ready (self-sufficient toolchain, no Go), so distribution got clean.

## Installing the compiler

### From source (works today)
Needs Rust (`cargo`) and, for the native backend, `clang`.
```bash
./install.sh                 # cargo build --release + installs into ~/.local/bin
# or: VADER_BINDIR=/usr/local/bin ./install.sh
vader version
```

### From releases (once there's a GitHub repo)
- **Linux/macOS:** download the release binary, `chmod +x`, move it to the PATH.
- **Homebrew:** `brew install YOUR-USERNAME/tap/vader` (`packaging/homebrew/vader.rb`).
- **Windows (winget):** `winget install Marco.Vader` (`packaging/winget/`).

## Publishing releases (cross-platform)
`.github/workflows/release.yml` compiles Linux/macOS(x64+arm64)/Windows and attaches the binaries
when you push a tag:
```bash
git tag v0.1.0 && git push origin v0.1.0
```
Then fill in the `sha256` values in Homebrew and generate the winget manifests.

## VS Code extension
- Usage/installation: `editors/vscode/README.md`
- **Publishing to the Marketplace:** `editors/vscode/PUBLISHING.md`

## Honest state

| Item | State |
|---|---|
| `vader version` + `install.sh` (from source) | ✅ tested (Linux/WSL) |
| Linux x86_64 binary (`cargo build --release`) | ✅ |
| macOS / Windows binaries | ⬜ only via the workflow (needs a GitHub repo) |
| Release workflow | ✅ written, ⬜ not run (no repo) |
| Homebrew / winget | ✅ templates, ⬜ need a published release |
| Extension ready for the Marketplace | ✅ (still needs a real `publisher` + `vsce publish`) |
