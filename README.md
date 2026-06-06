# Vader

Uma linguagem de programação **compilada, rápida e ergonômica**, cujo diferencial é o
**toolchain opinativo**: o projeto já nasce em Clean Architecture + TDD, funções geram
testes automaticamente, e as convenções de arquitetura são **verificadas pelo compilador**.

> _"A velocidade do Go, a ergonomia que você quer, e o rigor de engenharia que normalmente
> custa disciplina — tudo embutido."_

## Estado

Compilador completo em Rust com **dois backends nativos**:

- **LLVM** (`vader llvm`) — Vader → LLVM IR → `clang` → binário nativo, **sem Go**.
  Compila a linguagem **inteira**: structs, métodos, enums + match, slices, interfaces,
  genéricos, **canais + goroutines** (runtime pthreads) e **maps**.
- **Go** (`vader build`) — transpila pra Go e compila. Maduro.

Toolchain: `new · gen · fmt · test · lint · migrate · template · build · run · llvm · lsp`.
~103 testes.

## Instalar

Precisa de [Rust](https://rustup.rs) e, pro backend nativo, `clang`.

```bash
./install.sh          # cargo build --release + instala em ~/.local/bin
vader version
```

Detalhes (releases, Homebrew, winget, Docker): [`docs/distribution.md`](docs/distribution.md).

## Começo rápido

```bash
vader new api meu-projeto      # scaffold Clean Architecture + TDD
cd meu-projeto
vader build                    # compila pra binário
vader run                      # compila e roda
vader test                     # testes + cobertura
vader gen usecase CriarPedido  # gera artefato + teste espelho
```

Compilar um arquivo nativo via LLVM:
```bash
vader llvm examples/maps.vd
```

## Editor

Extensão do VS Code (realce + erros em tempo real via `vader lsp` + geração no botão
direito): [`editors/vscode/`](editors/vscode/). Publicação: [`editors/vscode/PUBLISHING.md`](editors/vscode/PUBLISHING.md).

## Exemplos

[`examples/`](examples/) — `basics`, `shapes` (enum/match), `slices`, `interfaces`,
`generics_demo`, `concurrency` (canais/goroutines), `maps`. Todos rodam nativos via `vader llvm`.

## Documentação

- [`SPEC.md`](SPEC.md) — especificação + roadmap.
- [`docs/`](docs/) — gramática, arquiteturas, persistência, pacotes, distribuição.

## Licença

MIT.
