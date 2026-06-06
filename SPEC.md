# Vader — Especificação

> Linguagem de programação compilada, rápida e ergonômica, com **boas práticas embutidas no toolchain**.
> Status: `draft v0.1` — especificação antes de qualquer código.

---

## 1. Visão

Uma linguagem **compilada pra binário único** (estilo Go/C), **fácil de escrever**, cujo
diferencial não é só performance — é o **toolchain opinativo**: o projeto já nasce em
Clean Architecture + TDD, funções geram testes automaticamente, e boas práticas vêm
embutidas, não opcionais.

**Pitch de uma linha:** _"A velocidade do Go, a ergonomia que você quer, e o rigor de
engenharia que normalmente custa disciplina — tudo embutido."_

## 2. Público-alvo

Devs e times que precisam de performance de sistema (APIs de alta demanda, workers,
processamento pesado) **sem** abrir mão de produtividade e qualidade de código.
Ambição de longo prazo: sistemas tempo-real / sem-GC (embarcados, controle).

## 3. Princípios de design

- **Convenção > configuração** — estilo Rails/NestJS, mas embutido na *linguagem*.
- **Boas práticas por padrão, não por disciplina** — o tooling força o caminho certo.
- **Sintaxe limpa** estilo Go: sem `;` obrigatório, sem parênteses em `if`/`for`.
- **Tipagem estática com inferência** — segurança sem verbosidade.
- **Binário único, sem runtime pesado.**
- **Erros explícitos** — sem exceptions escondidas.

## 4. O diferencial (o coração do projeto) ⭐

Mais importante que o backend. É o que torna a Vader única:

- `vader new api meu-projeto` → esqueleto completo em **Clean Architecture**
  (`domain / usecase / adapter / infra`), já com **TDD** configurado.
- **Função criada → teste gerado automaticamente** (stub espelhado em `*_test.vd`).
- **Test runner, formatter e linter embutidos** (zero config, estilo `cargo`/`go`).
- **Scaffolding por comando:** `vader gen usecase`, `vader gen worker`, `vader gen handler`.
- **Convenções de arquitetura verificadas pelo compilador** (ex.: `domain` não pode
  importar `infra` — vira erro de compilação, não só convenção).
- **Múltiplas arquiteturas** geradas e fiscalizadas: `clean`, `hexagonal`, `mvc`, `minimal`.
  Cada uma = template + ruleset próprio. Catálogo em [`docs/architectures.md`](docs/architectures.md).
- **Persistência batteries-included:** drivers de Postgres/MySQL/SQLite/Mongo na stdlib
  (sem instalar lib) + migrations no toolchain. Ver [`docs/persistence.md`](docs/persistence.md).
- **Gerenciador de pacotes** embutido (criar/instalar libs), registro central + git/URL.
  Ver [`docs/packages.md`](docs/packages.md).

## 5. Decisões fundacionais (TRAVADAS)

| Decisão | Escolha | Porquê |
|---|---|---|
| **Backend** | Faseado: **transpilar pra Go → depois LLVM** | Entrega rápida e prova o tooling já; front-end desenhado pra plugar LLVM depois sem reescrever. |
| **Host (linguagem do compilador)** | **Rust** | Enums + pattern matching ideais pra AST; performático; pronto pra LLVM via `inkwell` na fase 2. |
| **Inspiração semântica** | Go (concorrência, erros, simplicidade) + ergonomia própria | — |

## 6. Modelo da linguagem (direção — a refinar)

- **Tipos primitivos:** `int`, `float`, `bool`, `string`.
- **Compostos:** `struct`, slices (`[]T`), maps (`map[K]V`).
- **Variáveis:** tipagem forte e explícita, estilo C — `int x = 0`, `string name = "Vader"`, `bool ok = true`. Sem `let`/`var`/`mut`, sem inferência. `const` para constantes.
- **Funções:** `fn nome(a, b int): (int, error) { ... }` — retorno após `:`, params agrupáveis, múltiplo retorno explícito.
- **Tipos:** primitivos + `struct`, `interface`, **`enum`** (tipos-soma), **genéricos** (`[T]`), slices, maps, `chan[T]`.
- **Pattern matching:** `match` exaustivo sobre `enum`.
- **Laços:** só existe `for` (igual Go) — cobre while/range (`..`/`..=`)/infinito.
- **Módulos:** pacote por pasta, `import` por caminho, stdlib em `std/`.
- **Concorrência:** modelo goroutines/canais — ótimo pra workers e APIs.
- **Erros:** explícitos, estilo `int r, error err = faz()`.
- **Memória:** Fase 1 com GC (herdado do Go); Fase 2 opção sem-GC (via LLVM).

> Sintaxe fina (palavras-chave exatas, blocos, pattern matching) tem doc próprio:
> `docs/grammar.md` (a criar).

## 7. Toolchain / CLI (`vader`)

Dois eixos: **kind** (`api`/`worker`/`cli`/`lib` — muda o entrypoint) e **architecture**
(`clean`/`hexagonal`/`mvc`/`minimal` — muda estrutura + ruleset). Ex.: `vader new api x --arch hexagonal`.

| Comando | Faz |
|---|---|
| `vader new <kind> <nome> [--arch <arch>]` | Scaffolda projeto na arquitetura escolhida + TDD |
| `vader build` | Compila pra binário |
| `vader run` | Compila e roda |
| `vader test` | Roda os testes (runner embutido) |
| `vader gen <tipo>` | Gera usecase/handler/worker/struct + teste |
| `vader fmt` | Formata (estilo único, sem config) |
| `vader lint` | Lint + checagem de convenções de arquitetura |
| `vader migrate <sub>` | Migrations: `gen`/`new`/`up`/`down`/`status` |
| `vader add` / `remove` / `update` | Gerencia dependências |
| `vader publish` | Publica uma lib no registro |

## 8. Arquitetura do compilador (fases internas)

```
fonte .vd
   │
   ▼
[ Lexer ]  → tokens
   │
   ▼
[ Parser ] → AST
   │
   ▼
[ Checker ] → AST tipada + checagem de convenções de arquitetura
   │
   ▼
[ Backend ]  ── Fase 1: gera código Go → `go build` → binário
             └─ Fase 2: gera LLVM IR  → binário nativo (sem GC)
```

**Regra de ouro:** Lexer/Parser/Checker são **independentes do backend**. Trocar de Go
pra LLVM mexe só na última caixa.

## 9. Roadmap por fases

### Fase 0 — Specs ✅ CONCLUÍDA
- [x] Decisões fundacionais (backend, host)
- [x] Keywords em inglês
- [x] `docs/grammar.md` — gramática e sintaxe fina (draft)
- [x] Exemplos de código Vader "como deveria parecer" (`examples/`)
- [x] Regras de arquitetura fiscalizadas (`docs/architecture-rules.md`)
- [x] Catálogo de arquiteturas: clean, hexagonal, mvc, minimal (`docs/architectures.md`)
- [x] Persistência + migrations (`docs/persistence.md`)
- [x] Pacotes e dependências (`docs/packages.md`)

### Fase 1 — MVP usável (transpile pra Go)  ✅ FUNCIONAL
- [x] Lexer (em Rust) + testes
- [x] Parser + AST — funções, métodos, structs, interfaces, enums, genéricos, match, imports, concorrência (parseia todos os 9 exemplos)
- [x] Checker de tipos básico — vars/tipos, aridade de chamada/retorno, campos, condições, declarações duplicadas; **erros com linha:coluna** (valida `basics.vd`)
- [x] Backend transpile-pra-Go (inc.1) — `.vd` → Go → **binário nativo**. `hello.vd` e `basics.vd` rodam.
- [x] Backend inc.2 — enum→interface+structs, `match`→switch, interfaces, genéricos→generics Go. `shapes.vd` roda; `generics.vd` transpila.
- [x] Canais — checker + codegen (chan/make/send/recv/spawn/range). `concurrency.vd` roda. **Os 9 exemplos compilam.**
- [x] `_` discard em retorno múltiplo; stdlib mínima (`std/db`→`Conn`); **scaffold `clean` builda inteiro** (`vader new api` → binário)
- [x] CLI: `vader build` / `run` / **`new`** (scaffolder das 4 arquiteturas, com TDD) ✅
- [x] `vader gen` (fn/struct/usecase/handler) + **teste espelho automático** ✅
- [x] `test {}` / `assert` na linguagem (lexer/parser/checker/codegen) ✅
- [x] `vader fmt` — formatador canônico (round-trip de AST garantido, idempotente) ✅
- [x] `vader test` — roda os `test {}`, **relatório de cobertura** + **gate de push** (min configurável no `vader.toml`, desativável, `--install-hook`) ✅
- [x] Templates de projeto customizados — `vader template save/list` + `vader new --template` (placeholder `__name__`) ✅

### Fase 2 — Diferencial completo
- [x] Templates das 4 arquiteturas (clean/hexagonal/mvc/minimal) ✅
- [x] `vader gen` (fn/struct/usecase/handler) ✅
- [x] Checagem de convenções de arquitetura (`vader lint` + auto no build/check, ruleset por arquitetura) ✅
- [x] `vader migrate` (new/gen/status/up/down) — gera SQL das entidades, rastreia local
- [x] **Driver REAL de SQLite** — `import "std/db"`: `sqlite3.c` (amalgamation, domínio público)
      embarcado e linkado pelo clang no backend nativo. API `open/exec/query/next/col_int/col_text/col_float/close`.
      **Zero instalação, binário self-contained.** `examples/db_sqlite.vd` roda (persiste em arquivo). Cache do `.o`.
- [x] **Driver Postgres** (wire protocol puro + SCRAM-SHA-256, `postgres://...`) — compila,
      crypto (SHA-256) validada com vetor conhecido; round-trip ao vivo pendente. Mesma API.
- [x] **Driver MySQL/MariaDB** (protocolo nativo + `mysql_native_password`/SHA-1, `mysql://...`)
      — compila, crypto (SHA-1) validada; round-trip ao vivo pendente.
- [x] **TLS pro Postgres** (`vader llvm --tls`) — SSLRequest + OpenSSL sob `#ifdef VADER_TLS`,
      opt-in (sem libssl pra quem não usa). Código compila contra a API do OpenSSL; link real
      precisa de `libssl-dev` + servidor TLS pra verificar. v1 sem verificação de certificado.
- [ ] Auth MD5 (legado) + caching_sha2 do MySQL 8 + driver Mongo — próxima fase
- [x] **Execução real das migrations** — `vader migrate up/down [--db <dsn>]` (ou `[database] url`
      no vader.toml) roda o SQL no banco via `std/db` (`db.must` aborta se falhar; só marca
      aplicada em sucesso). **Verificado contra SQLite** (up cria+seed, down reverte).
- [x] **Gerenciador de pacotes (git/URL)** — `vader add <git-url|path>[@versão]` / `vader remove`:
      `git clone` num cache (`~/.vader/pkg`), `[dependencies]` no `vader.toml` + `vader.lock`
      (commit pinado). `module::load` faz fetch e injeta os `.vd` da dep no projeto. **Verificado
      end-to-end** (dep git local → `check`/`run`). `src/pkg.rs`.
- [x] **Registro de pacotes** — `vader add <nome> [--registry <dir|git-url>]` resolve por um
      `index.json` (dir local ou repo git, sem servidor dedicado); `vader publish` registra o
      pacote no índice. **Verificado** (publish → add por nome → run). Índice central pode ser
      um repo no GitHub (estilo tap).
- [x] **stdlib `std/http`** — servidor (`listen/accept/method/path/body/header/respond`) +
      cliente (`get/post`), HTTP/1.1 no runtime C. **Verificado com `curl`** e com o cliente
      HTTP da própria Vader. Sem TLS no cliente v1.
- [x] **stdlib `std/json`** — `parse` + acessores (`field/elem/as_str/as_int/as_float/as_bool/count`)
      + builder (`object/array/set*/add*`) + `encode`, no runtime C. **Verificado** (round-trip).

### Robustez (em andamento)
- [x] Posições (linha:coluna) nos erros do type checker
- [x] Detecção de declarações duplicadas
- [x] Lint de arquitetura automático no `build`/`run`/`check`
- [x] Sistema de módulos v1 — `check`/`build`/`run`/`test` aceitam um diretório; junta os `.vd`, normaliza nomes qualificados (`domain.User`→`User`) e compila como um programa. Projeto multi-pasta vira binário.
- [x] Checker modela canais; `_` discard; stdlib mínima — `concurrency.vd` e scaffold `clean` buildam
- [ ] Sistema de módulos v2 — namespaces de verdade (sem exigir nomes globais únicos), pacotes Go separados

### Fase 3 — Baixo nível de verdade  ✅ LLVM compila 100% da linguagem
- [x] Backend LLVM — **`vader llvm <file>`**: Vader → LLVM IR (texto) → `clang` → binário nativo, **sem Go**.
- [x] Núcleo sequencial completo no LLVM: int/bool/float/string, **structs, métodos, multi-retorno, enum+match, strings, slices, recursão, if/for**.
- [x] **Interfaces** no LLVM (fat-pointer `{data,vtable}` + shims + despacho dinâmico) — `interfaces.vd` roda.
- [x] **Genéricos** no LLVM (monomorfização sob demanda, inferência de `T` pelos args) — `generics_demo.vd` roda.
- [x] ASI-lite no parser (terminação de statement por quebra de linha)
- [x] **Canais + goroutines** no LLVM — runtime C (`runtime/vader_rt.c`, pthreads: canais bloqueantes + spawn), linkado pelo clang. `concurrency.vd` roda nativo. **Concorrência sem Go.**
- [x] **Maps** no LLVM — hash table no runtime C (chave int/string), `map[K]V` + `newmap()`. `maps.vd` roda nativo.
- [x] **LLVM compila a linguagem INTEIRA** — `vader llvm <file>` produz binário nativo sem Go pra todo recurso (int/float/string/struct/método/enum/match/slice/interface/genérico/canal/goroutine/map). 7 exemplos rodam nativos.
- [ ] Modo sem-GC explícito / liberação de memória (hoje vaza); mirar embarcados / tempo-real

### Fase 4 — Ecossistema & adoção  ◀ INICIADA
- [x] **Extensão VSCode — syntax highlighting** (`editors/vscode/`): gramática TextMate
      (`.vd`), `language-configuration` (comentários, brackets, auto-close). JSONs validados.
- [x] **`vader lsp`** — Language Server por stdio reusando lexer/parser/checker; publica
      diagnósticos (parse + tipo, linha:coluna, 0-based). JSON próprio sem deps (`src/json.rs`),
      servidor em `src/lsp.rs`. **Verificado end-to-end** (initialize/didOpen/didChange/shutdown).
      Cliente da extensão (`extension.js`, vscode-languageclient) só lança o processo.
- [x] **Distribuição (do fonte)** — `vader version`, `install.sh` (cargo build --release →
      `~/.local/bin`). **Testado**: binário instalado roda standalone (runtime embutido).
- [x] **Templates de release** — `.github/workflows/release.yml` (Linux/macOS×2/Windows),
      `packaging/homebrew/vader.rb`, `packaging/winget/`. Prontos; precisam de repo GitHub.
- [x] **Extensão PUBLICADA no Marketplace** — `Vader.vader` v0.3.0 no ar
      (marketplace.visualstudio.com/items?itemName=Vader.vader): highlighting + LSP +
      geração no botão direito. Guia em `editors/vscode/PUBLISHING.md`; `.vscodeignore` enxuga o pacote.
- [ ] Imagem Docker do toolchain (app-image já: `vader new` gera Dockerfile).

## 10. Decisões em aberto (não bloqueiam Fase 0)

- Palavras-chave exatas (`fn` vs `func`, `let` vs `var`, etc.) — PT-BR ou EN?
- Modelo de módulos / imports.
- Detalhe do modelo de concorrência.
- Sistema de pacotes / gerenciador de dependências.
- Nome da extensão de arquivo (`.vd` proposto).
