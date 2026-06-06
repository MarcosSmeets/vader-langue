# Vader — Distribuição, VSCode e Docker (plano)

> Como as pessoas vão **instalar/usar** a Vader, ter **extensão no VSCode**, e fazer
> **deploy via Docker**. Plano; ainda não implementado. Status: `draft`.

---

## 1. Distribuição (como as pessoas instalam e usam)

O compilador `vader` é um binário Rust. Caminhos de distribuição:

- **Binários pré-compilados** por plataforma (linux/mac/windows), via **GitHub Releases**
  (`cargo build --release` + cross-compile). Usuário baixa e põe no PATH.
- **Script de install** estilo rustup: `curl -fsSL https://.../install.sh | sh` (baixa o
  binário certo pro SO/arch).
- **Gerenciadores**: Homebrew (`brew install vader`), Scoop/winget (Windows),
  `cargo install vader` (se publicar no crates.io), depois apt/AUR.
- **Versionamento**: semver, `vader --version`, `vader upgrade`.

⚠️ **Hoje há uma dependência: o backend transpila pra Go**, então o usuário precisa do
**Go instalado**. Isso some quando o **backend LLVM** (Fase 3) ficar pronto: aí o `vader`
vira um toolchain **autossuficiente** (binário único, sem Go). → distribuição fica limpa
**depois do LLVM**.

## 2. Extensão do VSCode

Três camadas, da mais fácil pra mais poderosa — e cada uma **reaproveita o que já existe**:

1. **Syntax highlighting** (rápido, sem compilador) — uma gramática TextMate
   (`vader.tmLanguage.json`): keywords, tipos, strings, comentários. Entrega visual imediata.
2. **Language Server (LSP)** — o pulo do gato. Já temos **lexer + parser + checker com
   erros em linha:coluna** e **`vader fmt`**: é exatamente o que um LSP precisa. Plano:
   adicionar um modo `vader lsp` (fala o Language Server Protocol por stdio) que:
   - publica **diagnósticos** (erros de tipo + lint de arquitetura) ao salvar/editar — já computamos isso;
   - depois: hover, go-to-definition, autocomplete;
   - **format on save** → chama `vader fmt`.
3. **Extensão VSCode** (TypeScript) — registra a linguagem `.vd` + a gramática e sobe o
   cliente LSP conectando no `vader lsp`. Publica no **Marketplace**.

> Ordem sugerida: (1) highlighting → (2) `vader lsp` reusando o checker → (3) extensão cliente.

## 3. Docker

Dois usos, e o primeiro é **trunfo da Vader** (compila pra binário estático):

### a) Deploy de apps feitos em Vader (imagem minúscula)
O `vader build` gera um **binário ELF statically-linked** (já comprovado). Então a imagem
pode ser `scratch`/distroless de poucos MB:

```dockerfile
# build
FROM vader-toolchain AS build
WORKDIR /app
COPY . .
RUN vader build .

# runtime (imagem mínima)
FROM scratch
COPY --from=build /app/<projeto>/<projeto> /app
ENTRYPOINT ["/app"]
```

Ideia: o `vader new` pode **gerar o Dockerfile + .dockerignore** junto do projeto
(encaixa no diferencial de scaffolding).

### b) Distribuir o toolchain via Docker
Uma imagem `vader` (com vader + go por enquanto) pra usar o compilador sem instalar nada:
`docker run --rm -v $PWD:/app vader build .`. Bom pra CI.

> Depois do LLVM, a imagem do toolchain fica menor (sem Go) e o app-image continua mínima.

## Ordem recomendada

1. **Backend LLVM** (próxima sessão) — remove a dependência do Go e destrava distribuição limpa.
2. **VSCode**: highlighting → `vader lsp` (reusa o checker) → extensão.
3. **Docker**: `vader new` gera Dockerfile; imagem do toolchain pra CI.
4. **Distribuição**: releases + script de install + Homebrew/winget.

---

# Implementação — estado atual (jun/2026)

LLVM pronto (toolchain autossuficiente, sem Go), então a distribuição ficou limpa.

## Instalar o compilador

### A partir do fonte (funciona hoje)
Precisa de Rust (`cargo`) e, pro backend nativo, `clang`.
```bash
./install.sh                 # cargo build --release + instala em ~/.local/bin
# ou: VADER_BINDIR=/usr/local/bin ./install.sh
vader version
```

### A partir de releases (quando houver repo GitHub)
- **Linux/macOS:** baixe o binário da release, `chmod +x`, mova pro PATH.
- **Homebrew:** `brew install SEU-USUARIO/tap/vader` (`packaging/homebrew/vader.rb`).
- **Windows (winget):** `winget install Marco.Vader` (`packaging/winget/`).

## Publicar releases (cross-platform)
`.github/workflows/release.yml` compila Linux/macOS(x64+arm64)/Windows e anexa os binários
ao dar push numa tag:
```bash
git tag v0.1.0 && git push origin v0.1.0
```
Depois preencha os `sha256` no Homebrew e gere os manifestos winget.

## Extensão VS Code
- Uso/instalação: `editors/vscode/README.md`
- **Publicar no Marketplace:** `editors/vscode/PUBLISHING.md`

## Estado honesto

| Item | Estado |
|---|---|
| `vader version` + `install.sh` (do fonte) | ✅ testado (Linux/WSL) |
| Binário Linux x86_64 (`cargo build --release`) | ✅ |
| Binários macOS / Windows | ⬜ só via o workflow (precisa repo GitHub) |
| Workflow de release | ✅ escrito, ⬜ não rodado (sem repo) |
| Homebrew / winget | ✅ templates, ⬜ precisam de release publicada |
| Extensão pronta pro Marketplace | ✅ (falta `publisher` real + `vsce publish`) |
