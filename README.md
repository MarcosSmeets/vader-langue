# Vader

[![VS Code Marketplace](https://img.shields.io/visual-studio-marketplace/v/Vader.vader?label=VS%20Code%20Marketplace&color=e21d2e)](https://marketplace.visualstudio.com/items?itemName=Vader.vader)
[![License: MIT](https://img.shields.io/badge/License-MIT-14161b.svg)](LICENSE)

**A compiled, fast, statically-typed language whose real edge is an opinionated toolchain.**
A Vader project is born in Clean Architecture + TDD: functions generate their own tests,
architecture conventions are **enforced by the compiler**, and good practices are built in
rather than optional.

> _The speed of Go, the ergonomics you actually want, and the engineering rigor that
> usually costs discipline — all built in._

Vader compiles to a single native binary (Go/C style), is strongly and explicitly typed,
and ships database drivers (SQLite, PostgreSQL, MySQL, MongoDB) and a concurrency runtime embedded
in the compiler — zero install for the end user.

---

## Table of contents

- [Status](#status)
- [Benchmarks](#benchmarks)
- [Install](#install)
- [VS Code extension](#vs-code-extension)
- [Quick start](#quick-start)
- [Project types](#project-types)
- [The language](#the-language)
- [Toolchain](#toolchain)
- [Repository layout](#repository-layout)
- [Documentation](#documentation)
- [For LLMs / AI coding](#for-llms--ai-coding)
- [License](#license)

---

## Status

A complete compiler written in **Rust**, with **two native backends**:

- **LLVM** (`vader llvm`) — Vader → LLVM IR → `clang` → native binary, **no Go required**.
  Compiles the **entire** language: structs, methods, enums + `match`, slices, interfaces,
  generics, **channels + goroutines** (pthreads runtime) and **maps**.
- **Go** (`vader build` / `vader run`) — transpiles to Go and compiles. Mature and stable.

Toolchain: `new · gen · fmt · test · lint · migrate · template · add · build · run · llvm · lsp`.

The compiler has **zero external Rust dependencies**, and the database drivers + concurrency
runtime are embedded C, linked at compile time only when your program uses them.

---

## Benchmarks

Vader emits LLVM IR and lets `clang -O2` optimize it — the same backend `clang` uses for
C — so compute-bound code runs at native speed. Single-threaded micro-benchmarks, best of
3 runs, everything built at `-O2` / release on the same machine. Sources and the runner
live in [`benchmarks/`](benchmarks/) (`bash benchmarks/run.sh`):

| Benchmark      |  Vader |      C |    C++ |   Rust |     Go |
|----------------|-------:|-------:|-------:|-------:|-------:|
| `fib(40)`      | 0.341s | 0.323s | 0.316s | 0.331s | 0.596s |
| primes &lt; 2M | 0.344s | 0.333s | 0.331s | 0.344s | 0.519s |

Vader lands within a few percent of C/C++/Rust and comfortably ahead of Go. These are
tight compute loops (no allocation or I/O); absolute numbers vary by machine — reproduce
them with `benchmarks/run.sh`.

---

## Install

**One line — Linux / macOS** (downloads a prebuilt binary and adds it to your PATH):

```bash
curl -fsSL https://raw.githubusercontent.com/MarcosSmeets/vader-langue/main/install.sh | sh
```

**Windows** (PowerShell):

```powershell
irm https://raw.githubusercontent.com/MarcosSmeets/vader-langue/main/install.ps1 | iex
```

Installs to `~/.local/bin` (override with `VADER_BINDIR`) and wires up your shell profile.
Pin a version with `VADER_VERSION=v1.0.0`; skip the PATH edit with `VADER_NO_MODIFY_PATH=1`.
For the native backends you also want `clang` (for `vader llvm`) and/or `go` (for `vader build`/`run`).

**From source** — needs [Rust](https://rustup.rs):

```bash
git clone https://github.com/MarcosSmeets/vader-langue.git
cd vader-langue
./install.sh --source     # cargo build --release  →  installs to ~/.local/bin
vader version
```

> **Windows / WSL:** the prebuilt binary is a Linux ELF. If you develop under WSL, run the
> `curl | sh` installer *inside* WSL (and use Remote-WSL in VS Code) so the editor and
> compiler share the same environment. See [`editors/vscode/README.md`](editors/vscode/README.md).

Other channels (Homebrew, winget, Docker): [`docs/distribution.md`](docs/distribution.md).

---

## VS Code extension

**[Vader Language on the VS Code Marketplace →](https://marketplace.visualstudio.com/items?itemName=Vader.vader)**

Install it from the Extensions panel (search "Vader Language") or:

```bash
code --install-extension Vader.vader
```

It gives you syntax highlighting for `.vd`, real-time parse/type errors via the language
server (`vader lsp`, the compiler itself), and right-click code generation. Source and setup:
[`editors/vscode/`](editors/vscode/).

---

## Quick start

**1. Install** (Linux / macOS — or run it inside WSL on Windows):

```bash
curl -fsSL https://raw.githubusercontent.com/MarcosSmeets/vader-langue/main/install.sh | sh
```

**2. A running REST API in under a minute** — `vader new api` asks which database you
want (SQLite / Postgres / MySQL / MongoDB), then scaffolds a project that already has a
router, a `/health` route, a CRUD example, and a DB connection read from `DATABASE_URL`:

```bash
vader new api my-api
cd my-api
cp .env.example .env        # set DATABASE_URL (SQLite needs no server)
vader llvm .                # builds natively and runs

# in another terminal:
curl localhost:8080/health                                  # {"status":"ok"}
curl -X POST localhost:8080/users -d '{"name":"Ada"}'       # {"status":"created"}
curl localhost:8080/users                                   # [{"id":1,"name":"Ada"}]
```

**3. Or just run a file:**

```bash
echo 'public fn main() { print("hello from Vader") }' > hello.vd
vader llvm hello.vd
```

**Day-to-day toolchain:**

```bash
vader test                      # run tests + coverage gate
vader gen usecase CreateOrder   # generate an artifact + its mirror test
vader fmt . && vader lint .     # format + enforce the architecture rules
vader llvm --out server .       # build a deployable binary (no run) — used by the Dockerfile
```

---

## Project types

Scaffold a new project with `vader new <kind> <name> [--arch <arch>]`.

**Kinds** (what you're building) — each picks a sensible default architecture:

| Kind     | Default arch | For                              |
|----------|--------------|----------------------------------|
| `api`    | `clean`      | HTTP/REST services               |
| `worker` | `clean`      | Background jobs / consumers      |
| `cli`    | `minimal`    | Command-line tools               |
| `lib`    | `minimal`    | Reusable libraries (no Docker)   |

**Architectures** (`--arch`) — the folder layout and the rules the compiler enforces:

| Arch         | Layout                                            |
|--------------|---------------------------------------------------|
| `clean`      | domain / usecase / adapter / infra separation     |
| `hexagonal`  | ports & adapters                                  |
| `mvc`        | model / view / controller                         |
| `minimal`    | flat, no ceremony                                 |

```bash
vader new api my-api                  # api + clean (default)
vader new cli my-tool                 # cli + minimal
vader new api my-api --arch hexagonal # override the architecture
```

A generated `api` (clean) project looks like:

```
my-api/
├── vader.toml                  # project manifest
├── Dockerfile                  # (non-lib kinds)
├── cmd/main.vd                 # entry point
├── domain/
│   ├── user.vd                 # entities + ports (pure domain)
│   ├── user_test.vd            # auto-generated mirror test
│   └── user_repository.vd
├── usecase/
│   ├── create_user.vd
│   └── create_user_test.vd
├── adapter/http/
│   └── user_handler.vd
└── infra/db/
    └── user_repository_pg.vd
```

The manifest, `vader.toml`:

```toml
[project]
name         = "my-api"
version      = "0.1.0"
kind         = "api"
architecture = "clean"

[test]
coverage_gate = true
min_coverage  = 80

[dependencies]
# greeter = "https://github.com/user/greeter@v1.0"
```

**Code generation** — every artifact comes with a mirror `*_test.vd`:

```bash
vader gen fn      Calculate     # calculate.vd            + calculate_test.vd
vader gen struct  User          # user.vd                 + user_test.vd
vader gen usecase CreateOrder   # usecase/create_order.vd + test
vader gen handler UserCreate    # adapter/http/user_create.vd + test
```

**Custom templates:**

```bash
vader template save my-tmpl ./some-dir   # save a folder as a template
vader template list
vader new --template my-tmpl new-project
```

---

## The language

Vader source files use the `.vd` extension. Types are **explicit and written first**
(C-style) — there is no `let`/`var`/`mut` and no type inference. `public` exports a symbol;
the default is `private`.

### Hello, world

```vader
// hello.vd — the smallest Vader program.
fn main() {
    print("Hello, Vader!")
}
```

```bash
vader run hello.vd      # or: vader llvm hello.vd
```

### Variables, types and constants

Primitive types: `int` (64-bit signed), `float` (64-bit), `bool`, `string` (UTF-8),
`error`, plus `nil`.

```vader
string name   = "Vader"
int    count  = 0
bool   active = true
float  ratio  = 1.5

count = count + 1            // reassignment doesn't repeat the type

const int MAX_RETRIES = 3    // compile-time constant
```

### Functions

```vader
fn add(a, b int): int {      // params grouped by type: a and b are both int
    return a + b
}
```

Functions can return **multiple values** (the idiomatic way to surface errors), and methods
attach to a type via a receiver:

```vader
public fn divide(a, b int): (int, error) {
    if b == 0 {
        return 0, error("division by zero")
    }
    return a / b, nil
}

public struct User {
    id   int
    name string
}

public fn (u User) greeting(): string {     // method: (u User) receiver
    return "Hi, " + u.name
}
```

Generics use square brackets:

```vader
fn id[T](x T): T        { return x }
fn first[T](xs []T): T  { return xs[0] }
```

### Structs

```vader
public struct User {
    id   int
    name string
}

User user = User{ id: 1, name: "Marco" }   // struct literal
print(user.name)                            // field access
```

### Interfaces

Interfaces are method signatures; any struct with matching methods implements them
(implicit, structural):

```vader
interface Animal {
    fn sound(): string
}

struct Dog { name string }
fn (d Dog) sound(): string { return "woof" }

fn describe(a Animal): string {
    return a.sound()
}
```

### Enums & pattern matching

Sum types carry data per variant; `match` deconstructs them and is itself an expression:

```vader
public enum Shape {
    Circle(radius float)
    Rectangle(width float, height float)
    Point
}

public fn area(s Shape): float {
    return match s {
        Circle(r):       3.14159 * r * r
        Rectangle(w, h): w * h
        Point:           0.0
    }
}
```

### Collections

**Slices** — `[]T`:

```vader
[]int nums = [10, 20, 30]
print(nums[1])     // 20
print(len(nums))   // 3
```

**Maps** — `map[K]V` (keys are `int` or `string`):

```vader
map[string]int ages = newmap()
ages["ada"] = 36
ages["alan"] = 41
print(ages["ada"])   // 36
print(len(ages))     // 2
```

### Control flow

```vader
if b == 0 {
    return 0, error("division by zero")
} else if b < 0 {
    // ...
}

for {                 // infinite loop
}

for i < 10 {          // while-style
    i = i + 1
}

for i in 0..3 {       // exclusive range: 0, 1, 2
}

for i in 1..=n {      // inclusive range: 1 .. n
}

for x in nums {       // iterate a slice (or channel)
    total = total + x
}
```

### Concurrency

A goroutine-style model: `spawn` launches a lightweight task, channels (`chan[T]`)
communicate between them. `<-` sends/receives, `close` ends a channel.

```vader
fn worker(id int, jobs, results chan[int]) {
    for job in jobs {
        results <- job * 2          // send
    }
}

fn main() {
    chan[int] jobs    = chan[int](100)   // buffered channels
    chan[int] results = chan[int](100)

    for i in 0..3 {
        spawn worker(i, jobs, results)   // launch concurrent workers
    }

    for j in 1..5 {
        jobs <- j
    }
    close(jobs)

    for i in 1..5 {
        print(<-results)                 // receive
    }
}
```

### Error handling

Errors are **explicit values**, Go-style — no hidden exceptions. A function returns an
`error` alongside its result; the caller checks it against `nil`:

```vader
int result, error err = divide(10, 2)
if err != nil {
    print("error:", err)
    return
}
print("result:", result)
```

### Modules & imports

```vader
import "std/db"

DB conn = db.open("/tmp/app.db")   // referenced by the last path segment: std/db → db
```

Files in a directory compile together as one package; `public` symbols are visible across it.
The Clean/Hexagonal linter **forbids** illegal cross-layer imports (e.g. `domain` importing
`infra`) — see [`docs/architecture-rules.md`](docs/architecture-rules.md).

### Database access (built-in)

`import "std/db"` exposes embedded drivers — SQLite, PostgreSQL and MySQL — with no external
dependency. The right C driver is linked into your binary at compile time:

```vader
import "std/db"

public fn main() {
    DB conn = db.open("/tmp/vader_demo.db")   // or postgres://… / mysql://…

    db.exec(conn, "CREATE TABLE IF NOT EXISTS users (id INTEGER, name TEXT)")
    db.exec(conn, "INSERT INTO users VALUES (1, 'Marco')")

    Rows rows = db.query(conn, "SELECT id, name FROM users ORDER BY id")
    for db.next(rows) {
        int    id   = db.col_int(rows, 0)
        string name = db.col_text(rows, 1)
        print(id, name)
    }

    db.close(conn)
}
```

More: [`docs/persistence.md`](docs/persistence.md).

### Testing

Tests live next to the code in `test "…" { … }` blocks and run with `vader test`
(which also reports coverage and can gate on a minimum):

```vader
fn add(a, b int): int { return a + b }

test "add sums two numbers" {
    assert add(2, 3) == 5
}

test "add is commutative" {
    assert add(1, 2) == add(2, 1)
}
```

---

## Toolchain

| Command | What it does |
|---|---|
| `vader new <kind> <name> [--arch <a>]` | Scaffold a new project (Clean Architecture + TDD). |
| `vader new --template <t> <name>` | Scaffold from a saved template. |
| `vader template list \| save <t> <dir>` | Manage reusable templates. |
| `vader gen <fn\|struct\|usecase\|handler> <Name>` | Generate an artifact + its mirror test. |
| `vader build <path>` | Compile to a native binary (Go backend). |
| `vader run <path>` | Compile and run (Go backend). |
| `vader llvm <file.vd>` | Compile via LLVM IR + `clang`, then run (no Go). |
| `vader test <path> [--min-coverage n] [--no-gate]` | Run `test` blocks + coverage gate. |
| `vader fmt [-w] <file.vd>` | Format code (stdout, or `-w` to rewrite). |
| `vader lint <file.vd> [--arch <a>]` | Enforce architecture rules. |
| `vader check <file.vd>` | Type-check and report errors. |
| `vader add <git-url\|path>[@version] [name]` | Add a dependency (`vader remove <name>` to drop it). |
| `vader publish [--registry <dir\|git-url>]` | Register a package in a registry. |
| `vader migrate <new\|gen\|status\|up\|down>` | Manage SQL migrations. |
| `vader lsp` | Language server over stdio (used by the editor). |
| `vader lex \| parse <file.vd>` | Inspect the token stream / AST (debugging). |
| `vader version` | Print the version. |

Dependencies are declared in `vader.toml`'s `[dependencies]` (git URL or local path, optional
`@version`), resolved commits are pinned in `vader.lock`, and packages are cached under
`~/.vader/pkg/`. See [`docs/packages.md`](docs/packages.md).

---

## Repository layout

```
vader/
├── src/            # the compiler (Rust): lexer, parser, check, codegen, llvm, lsp, gen, …
├── runtime/        # embedded C: concurrency (vader_rt.c) + DB drivers + SQLite amalgamation
├── examples/       # runnable .vd programs (see below)
├── editors/vscode/ # VS Code extension (highlighting + LSP client + codegen)
├── docs/           # grammar, architectures, persistence, packages, distribution
├── scripts/        # build / test helpers
├── install.sh      # build from source and install
├── SPEC.md         # language specification + roadmap
└── Cargo.toml
```

## Examples

[`examples/`](examples/) — every program runs natively via `vader llvm`:
`hello`, `basics` (vars/functions/structs/errors), `shapes` (enum + match), `slices`,
`maps`, `interfaces`, `generics_demo`, `concurrency` (channels/goroutines),
`api_usecase` (a Clean Architecture slice), and `db_sqlite` / `db_postgres` / `db_mysql`.

## Documentation

- [`SPEC.md`](SPEC.md) — language specification + roadmap.
- [`docs/grammar.md`](docs/grammar.md) — full grammar.
- [`docs/architectures.md`](docs/architectures.md) · [`docs/architecture-rules.md`](docs/architecture-rules.md) — project layouts and the rules the compiler enforces.
- [`docs/persistence.md`](docs/persistence.md) — databases & migrations.
- [`docs/packages.md`](docs/packages.md) — the package manager.
- [`docs/distribution.md`](docs/distribution.md) — releases, Homebrew, winget, Docker.
- [`docs/llm-onboarding.md`](docs/llm-onboarding.md) — one-page primer for AI assistants.

## For LLMs / AI coding

Vader is a new language, so AI assistants won't know it out of the box. Instead of pointing
them at the whole docs, paste [`docs/llm-onboarding.md`](docs/llm-onboarding.md) — a dense,
self-contained one-pager (syntax, the standard library, idioms, gotchas, and a complete
REST example) that gets a model writing correct Vader immediately.

## License

[MIT](LICENSE) © 2026 Marcos Smeets.
