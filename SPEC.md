# Vader — Specification

> A compiled programming language — fast and ergonomic, with **best practices built into the toolchain**.
> Status: `draft v0.1` — specification before any code.

---

## 1. Vision

A language **compiled to a single binary** (Go/C style), **easy to write**, whose
differentiator isn't just performance — it's the **opinionated toolchain**: projects are
born in Clean Architecture + TDD, functions generate tests automatically, and best practices
come built in, not optional.

**One-line pitch:** _"The speed of Go, the ergonomics you want, and the engineering rigor
that normally costs discipline — all built in."_

## 2. Target audience

Devs and teams that need systems performance (high-demand APIs, workers, heavy
processing) **without** giving up productivity and code quality.
Long-term ambition: real-time / GC-free systems (embedded, control).

## 3. Design principles

- **Convention > configuration** — Rails/NestJS style, but built into the *language*.
- **Best practices by default, not by discipline** — the tooling enforces the right path.
- **Clean syntax** Go-style: no mandatory `;`, no parentheses in `if`/`for`.
- **Static typing with inference** — safety without verbosity.
- **Single binary, no heavy runtime.**
- **Explicit errors** — no hidden exceptions.

## 4. The differentiator (the heart of the project) ⭐

More important than the backend. It's what makes Vader unique:

- `vader new api my-project` → complete skeleton in **Clean Architecture**
  (`domain / usecase / adapter / infra`), already with **TDD** configured.
- **Function created → test generated automatically** (mirrored stub in `*_test.vd`).
- **Built-in test runner, formatter, and linter** (zero config, `cargo`/`go` style).
- **Scaffolding by command:** `vader gen usecase`, `vader gen worker`, `vader gen handler`.
- **Architecture conventions verified by the compiler** (e.g.: `domain` cannot
  import `infra` — it becomes a compile error, not just a convention).
- **Multiple architectures** generated and enforced: `clean`, `hexagonal`, `mvc`, `minimal`.
  Each one = its own template + ruleset. Catalog in [`docs/architectures.md`](docs/architectures.md).
- **Persistence batteries-included:** Postgres/MySQL/SQLite/Mongo drivers in the stdlib
  (no lib to install) + migrations in the toolchain. See [`docs/persistence.md`](docs/persistence.md).
- **Built-in package manager** (create/install libs), central registry + git/URL.
  See [`docs/packages.md`](docs/packages.md).

## 5. Foundational decisions (LOCKED)

| Decision | Choice | Why |
|---|---|---|
| **Backend** | Phased: **transpile to Go → then LLVM** | Fast delivery and proves the tooling right away; front-end designed to plug in LLVM later without a rewrite. |
| **Host (compiler language)** | **Rust** | Enums + pattern matching ideal for the AST; performant; ready for LLVM via `inkwell` in phase 2. |
| **Semantic inspiration** | Go (concurrency, errors, simplicity) + its own ergonomics | — |

## 6. Language model (direction — to be refined)

- **Primitive types:** `int`, `float`, `bool`, `string`.
- **Composite:** `struct`, slices (`[]T`), maps (`map[K]V`).
- **Variables:** strong, explicit typing, C-style — `int x = 0`, `string name = "Vader"`, `bool ok = true`. No `let`/`var`/`mut`, no inference. `const` for constants.
- **Functions:** `fn name(a, b int): (int, error) { ... }` — return after `:`, groupable params, explicit multiple return.
- **Types:** primitives + `struct`, `interface`, **`enum`** (sum types), **generics** (`[T]`), slices, maps, `chan[T]`.
- **Pattern matching:** exhaustive `match` over `enum`.
- **Loops:** only `for` exists (like Go) — covers while/range (`..`/`..=`)/infinite.
- **Modules:** package per folder, `import` by path, stdlib under `std/`.
- **Concurrency:** goroutines/channels model — great for workers and APIs.
- **Errors:** explicit, `int r, error err = does()` style.
- **Memory:** Phase 1 with GC (inherited from Go); Phase 2 GC-free option (via LLVM).

> Fine-grained syntax (exact keywords, blocks, pattern matching) has its own doc:
> `docs/grammar.md` (to be created).

## 7. Toolchain / CLI (`vader`)

Two axes: **kind** (`api`/`worker`/`cli`/`lib` — changes the entrypoint) and **architecture**
(`clean`/`hexagonal`/`mvc`/`minimal` — changes structure + ruleset). E.g.: `vader new api x --arch hexagonal`.

| Command | Does |
|---|---|
| `vader new <kind> <name> [--arch <arch>]` | Scaffolds a project in the chosen architecture + TDD |
| `vader build` | Compiles to a binary |
| `vader run` | Compiles and runs |
| `vader test` | Runs the tests (built-in runner) |
| `vader gen <type>` | Generates usecase/handler/worker/struct + test |
| `vader fmt` | Formats (single style, no config) |
| `vader lint` | Lint + architecture convention checking |
| `vader migrate <sub>` | Migrations: `gen`/`new`/`up`/`down`/`status` |
| `vader add` / `remove` / `update` | Manages dependencies |
| `vader publish` | Publishes a lib to the registry |

## 8. Compiler architecture (internal phases)

```
source .vd
   │
   ▼
[ Lexer ]  → tokens
   │
   ▼
[ Parser ] → AST
   │
   ▼
[ Checker ] → typed AST + architecture convention checking
   │
   ▼
[ Backend ]  ── Phase 1: emits Go code → `go build` → binary
             └─ Phase 2: emits LLVM IR  → native binary (GC-free)
```

**Golden rule:** Lexer/Parser/Checker are **backend-independent**. Switching from Go
to LLVM touches only the last box.

## 9. Roadmap by phases

### Phase 0 — Specs ✅ DONE
- [x] Foundational decisions (backend, host)
- [x] Keywords in English
- [x] `docs/grammar.md` — grammar and fine-grained syntax (draft)
- [x] Vader code examples "as it should look" (`examples/`)
- [x] Enforced architecture rules (`docs/architecture-rules.md`)
- [x] Architecture catalog: clean, hexagonal, mvc, minimal (`docs/architectures.md`)
- [x] Persistence + migrations (`docs/persistence.md`)
- [x] Packages and dependencies (`docs/packages.md`)

### Phase 1 — Usable MVP (transpile to Go)  ✅ FUNCTIONAL
- [x] Lexer (in Rust) + tests
- [x] Parser + AST — functions, methods, structs, interfaces, enums, generics, match, imports, concurrency (parses all 9 examples)
- [x] Basic type checker — vars/types, call/return arity, fields, conditions, duplicate declarations; **errors with line:column** (validates `basics.vd`)
- [x] Transpile-to-Go backend (inc.1) — `.vd` → Go → **native binary**. `hello.vd` and `basics.vd` run.
- [x] Backend inc.2 — enum→interface+structs, `match`→switch, interfaces, generics→Go generics. `shapes.vd` runs; `generics.vd` transpiles.
- [x] Channels — checker + codegen (chan/make/send/recv/spawn/range). `concurrency.vd` runs. **All 9 examples compile.**
- [x] `_` discard in multiple return; minimal stdlib (`std/db`→`Conn`); **`clean` scaffold builds end to end** (`vader new api` → binary)
- [x] CLI: `vader build` / `run` / **`new`** (scaffolder for the 4 architectures, with TDD) ✅
- [x] `vader gen` (fn/struct/usecase/handler) + **automatic mirror test** ✅
- [x] `test {}` / `assert` in the language (lexer/parser/checker/codegen) ✅
- [x] `vader fmt` — canonical formatter (guaranteed AST round-trip, idempotent) ✅
- [x] `vader test` — runs the `test {}` blocks, **coverage report** + **push gate** (configurable minimum in `vader.toml`, disablable, `--install-hook`) ✅
- [x] Custom project templates — `vader template save/list` + `vader new --template` (`__name__` placeholder) ✅

### Phase 2 — Full differentiator
- [x] Templates for the 4 architectures (clean/hexagonal/mvc/minimal) ✅
- [x] `vader gen` (fn/struct/usecase/handler) ✅
- [x] Architecture convention checking (`vader lint` + automatic on build/check, ruleset per architecture) ✅
- [x] `vader migrate` (new/gen/status/up/down) — generates SQL from entities, tracks locally
- [x] **REAL SQLite driver** — `import "std/db"`: `sqlite3.c` (amalgamation, public domain)
      embedded and linked by clang in the native backend. API `open/exec/query/next/col_int/col_text/col_float/close`.
      **Zero install, self-contained binary.** `examples/db_sqlite.vd` runs (persists to a file). `.o` cache.
- [x] **Postgres driver** (pure wire protocol + SCRAM-SHA-256, `postgres://...`) — compiles,
      crypto (SHA-256) validated against a known vector; live round-trip pending. Same API.
- [x] **MySQL/MariaDB driver** (native protocol + `mysql_native_password`/SHA-1, `mysql://...`)
      — compiles, crypto (SHA-1) validated; live round-trip pending.
- [x] **TLS for Postgres** (`vader llvm --tls`) — SSLRequest + OpenSSL under `#ifdef VADER_TLS`,
      opt-in (no libssl for those who don't use it). Code compiles against the OpenSSL API; real
      linking needs `libssl-dev` + a TLS server to verify. v1 without certificate verification.
- [ ] MD5 auth (legacy) + MySQL 8 caching_sha2 — next phase
- [x] **Mongo driver** (`std/mongo`) — a document API (not SQL): `mongo.connect(dsn)`,
      `mongo.insert(m, coll, doc)`, `mongo.find(m, coll, query): docs`, `mongo.close(m)`.
      Own BSON encoder/decoder (reusing the `vader_json` value tree) + the OP_MSG wire protocol
      (`runtime/vader_mongo.c`) + **SCRAM-SHA-256 authentication** (shared crypto in
      `runtime/vader_scram.c`). **Live-verified** against MongoDB 7 (Docker): with credentials
      (`mongodb://user:pass@host/db`) insert + find succeed, and without credentials inserts are
      rejected. Pending: update/delete and the aggregation pipeline.
- [x] **Real execution of migrations** — `vader migrate up/down [--db <dsn>]` (or `[database] url`
      in vader.toml) runs the SQL against the database via `std/db` (`db.must` aborts on failure; only marks
      applied on success). **Verified against SQLite** (up creates+seeds, down reverts).
- [x] **Package manager (git/URL)** — `vader add <git-url|path>[@version]` / `vader remove`:
      `git clone` into a cache (`~/.vader/pkg`), `[dependencies]` in `vader.toml` + `vader.lock`
      (pinned commit). `module::load` fetches and injects the dep's `.vd` files into the project. **Verified
      end-to-end** (local git dep → `check`/`run`). `src/pkg.rs`.
- [x] **Package registry** — `vader add <name> [--registry <dir|git-url>]` resolves via an
      `index.json` (local dir or git repo, no dedicated server); `vader publish` registers the
      package in the index. **Verified** (publish → add by name → run). The central index can be
      a GitHub repo (tap style).
- [x] **stdlib `std/http`** — server (`listen/accept/method/path/body/header/respond`) +
      client (`get/post`), HTTP/1.1 on the C runtime. **Verified with `curl`** and with Vader's own
      HTTP client. No TLS in the v1 client.
- [x] **stdlib `std/json`** — `parse` + accessors (`field/elem/as_str/as_int/as_float/as_bool/count`)
      + builder (`object/array/set*/add*`) + `encode`, on the C runtime. **Verified** (round-trip).

### Robustness (in progress)
- [x] Positions (line:column) in type checker errors
- [x] Duplicate declaration detection
- [x] **Strict type checker** — an unknown type name becomes an **error** (`unknown type \`Foo\``),
      no longer a silent `Unknown`. Preserves what is legitimately polymorphic: type
      parameters (generics), interfaces, and opaque stdlib handles (`DB/Rows/Server/Json/Conn`).
      The 11 pure examples + scaffold keep passing; programs with a nonexistent type fail.
- [x] Automatic architecture lint on `build`/`run`/`check`
- [x] Module system v1 — `check`/`build`/`run`/`test` accept a directory; merges the `.vd` files, normalizes qualified names (`domain.User`→`User`) and compiles as one program. A multi-folder project becomes a binary.
- [x] Checker models channels; `_` discard; minimal stdlib — `concurrency.vd` and the `clean` scaffold build
- [ ] Module system v2 — real namespaces (without requiring globally unique names), separate Go packages

### Phase 3 — Truly low-level  ✅ LLVM compiles 100% of the language
- [x] LLVM backend — **`vader llvm <file>`**: Vader → LLVM IR (text) → `clang` → native binary, **without Go**.
- [x] Complete sequential core in LLVM: int/bool/float/string, **structs, methods, multi-return, enum+match, strings, slices, recursion, if/for**.
- [x] **Interfaces** in LLVM (fat-pointer `{data,vtable}` + shims + dynamic dispatch) — `interfaces.vd` runs.
- [x] **Generics** in LLVM (on-demand monomorphization, `T` inference from args) — `generics_demo.vd` runs.
- [x] ASI-lite in the parser (statement termination by line break)
- [x] **Channels + goroutines** in LLVM — C runtime (`runtime/vader_rt.c`, pthreads: blocking channels + spawn), linked by clang. `concurrency.vd` runs native. **Concurrency without Go.**
- [x] **Maps** in LLVM — hash table in the C runtime (int/string key), `map[K]V` + `newmap()`. `maps.vd` runs native.
- [x] **LLVM compiles the WHOLE language** — `vader llvm <file>` produces a native binary without Go for every feature (int/float/string/struct/method/enum/match/slice/interface/generic/channel/goroutine/map). 7 examples run native.
- [x] **Long-running service memory (arena/region, GC-free)** — `runtime/vader_mem.c`: an arena
      allocator (bump + bulk free) with thread-local scope. Strings/JSON/HTTP/db routed
      through `vader_alloc`. `http.accept` cycles the arena per request automatically; `std/mem.scope/
      release` for the workers. **+ codegen fix:** `hoist_allocas` moves allocas to the `entry`
      block (allocas in loops leaked the stack). **Verified: HTTP server stays at constant RSS under
      8000 requests** (runtime proven 0 heap leak). Deterministic, aligned with real-time.
- [ ] No-arena-by-default mode / explicit release for pure embedded (today: no scope = malloc that leaks, on purpose)

### Phase 4 — Ecosystem & adoption  ◀ STARTED
- [x] **VSCode extension — syntax highlighting** (`editors/vscode/`): TextMate grammar
      (`.vd`), `language-configuration` (comments, brackets, auto-close). JSONs validated.
- [x] **`vader lsp`** — Language Server over stdio reusing lexer/parser/checker; publishes
      diagnostics (parse + type, line:column, 0-based). Custom JSON with no deps (`src/json.rs`),
      server in `src/lsp.rs`. **Verified end-to-end** (initialize/didOpen/didChange/shutdown).
      The extension client (`extension.js`, vscode-languageclient) only launches the process.
- [x] **Distribution (from source)** — `vader version`, `install.sh` (cargo build --release →
      `~/.local/bin`). **Tested**: the installed binary runs standalone (runtime embedded).
- [x] **Release templates** — `.github/workflows/release.yml` (Linux/macOS×2/Windows),
      `packaging/homebrew/vader.rb`, `packaging/winget/`. Ready; need a GitHub repo.
- [x] **Extension PUBLISHED on the Marketplace** — `Vader.vader` v0.3.0 is live
      (marketplace.visualstudio.com/items?itemName=Vader.vader): highlighting + LSP +
      right-click generation. Guide in `editors/vscode/PUBLISHING.md`; `.vscodeignore` trims the package.
- [ ] Docker image of the toolchain (app-image already: `vader new` generates a Dockerfile).

## 10. Open decisions (do not block Phase 0)

- Exact keywords (`fn` vs `func`, `let` vs `var`, etc.) — PT-BR or EN?
- Module / imports model.
- Concurrency model detail.
- Package system / dependency manager.
- File extension name (`.vd` proposed).
