# Vader

[![VS Code Marketplace](https://img.shields.io/visual-studio-marketplace/v/Vader.vader?label=VS%20Code%20Marketplace&color=e21d2e)](https://marketplace.visualstudio.com/items?itemName=Vader.vader)
[![License: MIT](https://img.shields.io/badge/License-MIT-14161b.svg)](LICENSE)

**A compiled, statically-typed language whose real edge is an opinionated toolchain.**

Most languages hand you a compiler and leave the rest — architecture, tests, database wiring,
deployment — as homework. Vader takes the opposite stance: a project is born production-shaped.
`vader new api` gives you a running REST service with a database, a router, a health check, a
test suite and a Dockerfile. The architecture you pick is then **enforced by the compiler** —
if a domain file imports infrastructure, the build fails. Good practices stop being a matter
of discipline and become the path of least resistance.

Underneath, Vader compiles to a single native binary through LLVM, so compute-bound code runs
within a few percent of C. The database drivers (SQLite, PostgreSQL, MySQL, MongoDB) and the
concurrency runtime are embedded in the compiler and linked into your binary only when you use
them — there is nothing for your users to install.

```bash
vader new api store          # pick a database, get a working REST API
cd store && vader run        # compiles natively and serves on :8080
```

---

## Table of contents

- [Why Vader](#why-vader)
- [Status](#status)
- [Benchmarks](#benchmarks)
- [Install](#install)
- [Quick start](#quick-start)
- [Project types & architectures](#project-types--architectures)
- [The language](#the-language)
- [Standard library](#standard-library)
- [Connecting to a database](#connecting-to-a-database)
- [Testing](#testing)
- [Architecture enforcement](#architecture-enforcement)
- [Building & deploying](#building--deploying)
- [Toolchain reference](#toolchain-reference)
- [VS Code extension](#vs-code-extension)
- [Repository layout](#repository-layout)
- [Documentation](#documentation)
- [For LLMs / AI coding](#for-llms--ai-coding)
- [License](#license)

---

## Why Vader

Three convictions shaped the language:

**The toolchain should have opinions.** A new service shouldn't begin with a blank file and a
week of decisions about folders, test layout and database plumbing. `vader new` scaffolds a
complete, layered, test-covered service for the architecture you name. You start by writing
business logic, not boilerplate.

**Architecture should be checked, not just documented.** A diagram in a wiki drifts the moment
someone is in a hurry. In Vader the dependency rules of Clean Architecture, Hexagonal, MVC and
DDD are part of the build: an inner layer that reaches for an outer one, or a pure domain that
performs I/O, is a compile error — not a code-review comment six weeks later. File names are
held to the same standard (`snake_case`, enforced).

**Speed shouldn't cost ergonomics.** Vader is explicitly typed and reads like Go, but it emits
LLVM IR and lets `clang -O2` optimize it — the same backend C uses. You get native performance
without writing C, and a single static-ish binary you can drop into a `FROM debian:slim` image.

---

## Status

A complete compiler written in **Rust** with a native backend: Vader → LLVM IR → `clang` →
native binary. It compiles the entire language — structs, methods, enums with `match`, slices,
maps, interfaces, generics, and channels + goroutines on a pthreads runtime.

The build/run/test pipeline is **fully native — Go is not required and not used.** The compiler
itself has **zero external Rust dependencies**; the database drivers and concurrency runtime are
embedded C, compiled into your binary only when your program references them.

Toolchain: `new · gen · build · run · test · fmt · lint · check · migrate · template · add · publish · llvm · lsp`.

**Requirements:** a POSIX toolchain with `clang` (Linux, macOS, or WSL on Windows). Native
Windows isn't a target yet — develop under WSL.

---

## Benchmarks

Vader emits LLVM IR and lets `clang -O2` optimize it, so compute-bound code runs at native
speed. Single-threaded micro-benchmarks, best of 3 runs, everything built at `-O2` / release on
the same machine. Sources and the runner live in [`benchmarks/`](benchmarks/)
(`bash benchmarks/run.sh`):

| Benchmark      |  Vader |      C |    C++ |   Rust |     Go |
|----------------|-------:|-------:|-------:|-------:|-------:|
| `fib(40)`      | 0.341s | 0.323s | 0.316s | 0.331s | 0.596s |
| primes &lt; 2M | 0.344s | 0.333s | 0.331s | 0.344s | 0.519s |

Vader lands within a few percent of C/C++/Rust and comfortably ahead of Go. These are tight
compute loops with no allocation or I/O; absolute numbers vary by machine — reproduce them with
`benchmarks/run.sh`.

---

## Install

**One line — Linux / macOS** (downloads a prebuilt binary and adds it to your PATH):

```bash
curl -fsSL https://raw.githubusercontent.com/MarcosSmeets/vader-langue/main/install.sh | sh
```

You also need **`clang`** on your PATH (it links the native binaries Vader produces). It ships
with Xcode Command Line Tools on macOS and `apt install clang` / `dnf install clang` on Linux.

Pin a version with `VADER_VERSION=v1.0.0`, change the target directory with `VADER_BINDIR`, or
skip the PATH edit with `VADER_NO_MODIFY_PATH=1`. The installer puts the binary in
`~/.local/bin` by default.

**From source** — needs [Rust](https://rustup.rs) and `clang`:

```bash
git clone https://github.com/MarcosSmeets/vader-langue.git
cd vader-langue
cargo build --release
install -m755 target/release/vader ~/.local/bin/vader   # or anywhere on your PATH
vader version
```

**Windows:** the prebuilt binary is a Linux ELF. Run the `curl | sh` installer **inside WSL**
and use the Remote-WSL extension in VS Code so the editor and compiler share one environment.
See [`editors/vscode/README.md`](editors/vscode/README.md).

Other channels (Homebrew, winget, Docker) are documented in
[`docs/distribution.md`](docs/distribution.md).

---

## Quick start

**1. Scaffold a REST API.** `vader new api` asks which database you want — SQLite, Postgres,
MySQL or MongoDB — and generates a project that already has a router, a `/health` route, a
`User` CRUD slice, a test, a Dockerfile and a `docker-compose.yml`:

```bash
vader new api store          # choose SQLite when prompted (no server to run)
cd store
cp .env.example .env         # sets DATABASE_URL
vader run                    # compiles natively and serves on :8080
```

In another terminal — use `127.0.0.1` (the server listens on IPv4):

```bash
curl 127.0.0.1:8080/health                                   # {"status":"ok"}
curl -X POST 127.0.0.1:8080/users -d '{"name":"Ada"}'        # {"status":"created"}
curl 127.0.0.1:8080/users                                    # [{"id":1,"name":"Ada"}]
```

**2. Or bring up the whole stack in containers** — the generated compose file wires the app to
the database you picked:

```bash
docker compose up
```

**3. Or just run a file:**

```bash
echo 'public fn main() { print("hello from Vader") }' > hello.vd
vader run hello.vd
```

**Day to day:**

```bash
vader test                       # run test blocks + coverage (fully native, no extra tooling)
vader build --out server .       # produce a deployable binary without running it
vader gen usecase CreateOrder    # generate an artifact + its mirror test
vader fmt . && vader check .     # format, then type-check + enforce the architecture rules
```

---

## Project types & architectures

Scaffold with `vader new <kind> <name> [--arch <arch>] [--db <engine>]`.

**Kinds** — what you're building:

| Kind     | Default arch | For                              |
|----------|--------------|----------------------------------|
| `api`    | `tdd`        | HTTP / REST services             |
| `worker` | `clean`      | Background jobs / consumers      |
| `cli`    | `minimal`    | Command-line tools               |
| `lib`    | `minimal`    | Reusable libraries (no Docker)   |

**API architectures** — `vader new api --arch <arch>` builds a complete, runnable `User`
vertical slice (HTTP + database + router + tests + Docker) laid out for that architecture. The
dependency rules below are then enforced on every `vader build` / `run` / `check`:

| `--arch`     | Layout                                                                  | Rule of thumb                                  |
|--------------|-------------------------------------------------------------------------|------------------------------------------------|
| `tdd`        | `cmd / routes / handlers / infra / domain / test` (flat, pragmatic)     | The fast default; ship a service today.        |
| `clean`      | `domain / application / infrastructure / interfaces`                    | Dependencies point inward; domain is pure.     |
| `hexagonal`  | `core (domain/ports/services) / adapters (inbound/outbound)`            | The core knows nothing about the outside.      |
| `mvc`        | `models / repositories / services / controllers / routes`              | Controllers delegate; services hold the logic. |
| `ddd`        | `contexts/<context>/{domain,application,infrastructure}` + `shared`     | Organise by bounded context, not by type.      |

```bash
vader new api store                    # api + tdd (default), prompts for the database
vader new api store --arch clean --db postgres
vader new cli backup-tool              # cli + minimal
```

> The layered API architectures (`clean`/`hexagonal`/`mvc`/`ddd`) use a SQL database via
> `std/db`. For MongoDB, use `--arch tdd`.

A generated `clean` API looks like this — the repository is an **interface in the domain**,
implemented concretely in `infrastructure`, and the use case depends only on the abstraction:

```
store/
├── vader.toml                     # manifest (records the architecture)
├── .env.example                   # DATABASE_URL
├── Dockerfile                     # native multi-stage build
├── docker-compose.yml             # app + database
├── cmd/
│   └── main.vd                    # entry point: migrate(), then serve
├── domain/
│   ├── user.vd                    # entity (pure, no imports)
│   └── user_repository.vd         # the repository contract (interface)
├── application/
│   └── create_user.vd            # use cases, depend on the interface
├── infrastructure/
│   └── user_repository_pg.vd      # the concrete repository (does the I/O)
├── interfaces/
│   ├── user_handler.vd            # HTTP handlers
│   └── router.vd                  # route wiring
└── test/
    └── user_test.vd
```

The manifest, `vader.toml`:

```toml
[project]
name         = "store"
version      = "0.1.0"
kind         = "api"
architecture = "clean"

[database]
engine = "sqlite"        # the DSN itself comes from DATABASE_URL

[test]
coverage_gate = false    # flip to true to fail the build below min_coverage
min_coverage  = 80

[dependencies]
# greeter = "https://github.com/user/greeter@v1.0"
```

**Code generation** — every artifact comes with a mirror `*_test.vd`:

```bash
vader gen fn      Calculate     # calculate.vd + calculate_test.vd
vader gen struct  Invoice       # invoice.vd   + invoice_test.vd
vader gen usecase CreateOrder   # create_order.vd + test
vader gen handler UserCreate    # user_create.vd  + test
```

**Custom templates** — save any folder and scaffold from it later:

```bash
vader template save my-stack ./some-project
vader template list
vader new --template my-stack new-service
```

---

## The language

Vader source files use the `.vd` extension. Types are **explicit and written first** (C-style)
— there is no `let` / `var` / `mut` and no type inference. `public` exports a symbol; the
default is `private`. Statements end at the newline; conditions take no parentheses.

### Hello, world

```vader
// hello.vd
public fn main() {
    print("Hello, Vader!")
}
```

```bash
vader run hello.vd
```

### Variables, types and constants

Primitive types: `int` (64-bit signed), `float` (64-bit), `bool`, `string` (UTF-8), `error`,
plus `nil`.

```vader
string name   = "Vader"
int    count  = 0
bool   active = true
float  ratio  = 1.5

count = count + 1            // reassignment does not repeat the type
const int MAX_RETRIES = 3    // compile-time constant

string s = "a" + "b"         // strings concatenate with +
```

### Functions

Parameters are grouped by type, and return types come after a colon. Functions can return
multiple values — the idiomatic way to surface errors.

```vader
fn add(a, b int): int {          // a and b are both int
    return a + b
}

public fn divide(a, b int): (int, error) {
    if b == 0 {
        return 0, error("division by zero")
    }
    return a / b, nil
}

public fn main() {
    int q, error err = divide(10, 2)
    if err != nil {
        print("error:", err)
        return
    }
    print(q)
}
```

Functions are first-class values: a handler is passed to the router by name. Generics use
square brackets:

```vader
fn id[T](x T): T       { return x }
fn first[T](xs []T): T { return xs[0] }
```

### Structs, methods and interfaces

```vader
public struct User {
    id   int
    name string
}

public fn (u User) greeting(): string {     // method via a receiver
    return "Hi, " + u.name
}

User user = User{ id: 1, name: "Ada" }       // struct literal
print(user.greeting())

interface Animal {                            // structural: any type with the
    fn sound(): string                        // method implements the interface
}
struct Dog { name string }
fn (d Dog) sound(): string { return "woof" }

fn describe(a Animal): string { return a.sound() }
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

```vader
[]int nums = [10, 20, 30]        // slice literal
print(nums[1], len(nums))        // 20 3  (indexing is bounds-checked)

map[string]int ages = newmap()   // keys are int or string
ages["ada"] = 36
print(ages["ada"], len(ages))    // 36 1
```

### Control flow

`for` is the only loop — it covers `while`, ranges and infinite loops.

```vader
for i < 10 { i = i + 1 }          // while-style
for n in 0..3 { print(n) }        // exclusive range: 0 1 2
for n in 1..=5 { print(n) }       // inclusive range: 1 .. 5
for x in nums { total = total + x }  // iterate a slice (or channel)
for { }                           // infinite

if x > 0 {
    // ...
} else if x < 0 {
    // ...
}
```

### Concurrency

A goroutine-style model: `spawn` launches a lightweight task; channels (`chan[T]`) communicate
between them. `<-` sends and receives, `close` ends a channel.

```vader
fn worker(jobs, results chan[int]) {
    for job in jobs {
        results <- job * 2           // send
    }
}

public fn main() {
    chan[int] jobs    = chan[int](100)   // buffered channel
    chan[int] results = chan[int](100)

    for i in 0..3 { spawn worker(jobs, results) }
    for j in 1..=4 { jobs <- j }
    close(jobs)
    for i in 1..=4 { print(<-results) }  // receive
}
```

### Error handling

Errors are explicit values, Go-style — no hidden exceptions. A function returns an `error`
beside its result and the caller checks it against `nil`. `nil` is assignable to `error`,
structs, slices, channels and enums.

```vader
int result, error err = divide(10, 0)
if err != nil {
    print("error:", err)
    return
}
```

### Modules & imports

```vader
import "std/db"
DB conn = db.open("/tmp/app.db")    // calls use the last path segment: std/db → db
```

Files in a directory compile together as one program; `public` symbols are visible across it.
Type names must be unique across a project. Cross-layer imports that break the architecture are
a compile error — see [Architecture enforcement](#architecture-enforcement).

---

## Standard library

Import with `import "std/<pkg>"`. Calls are written qualified (`http.listen`, `db.open`); the
package prefix is normalized away by the compiler. Opaque handle types you'll see:
`Server`, `Router`, `DB`, `Rows`, `Stmt`, `Json`, `Mongo`, `Arena`.

| Package        | What you get                                                                                  |
|----------------|-----------------------------------------------------------------------------------------------|
| `std/http`     | HTTP server, router and client. `newRouter()`, `r.get/post/put/delete`, `serve`, `http.json/text/respond`, `http.method/path/body/header`, `http.get/post`. |
| `std/json`     | Build and parse JSON. `object/array`, `set_str/set_int/set_float/set_bool/set`, `add_str/add_int/add`, `field/elem/count`, `as_str/as_int/as_float/as_bool`, `parse/encode`. |
| `std/db`       | One API for SQLite / PostgreSQL / MySQL. `open/exec/must/query/next/col_int/col_text/col_float/close`, plus parameterized `prepare/bind_*/run/query_stmt`. |
| `std/mongo`    | MongoDB document store. `connect/insert/find/aggregate/update/delete/close` (documents are `Json`). |
| `std/strings`  | `length`, `upper/lower/trim`, `contains/index_of/starts_with/ends_with`, `substring/repeat/replace`, `to_int/to_float`, `split → []string`, `join`. |
| `std/math`     | `sqrt/abs/floor/ceil/round/sin/cos/tan/log/exp`, `pow`, `min/max` (int), `fmin/fmax` (float), `pi`, `random/random_int`. |
| `std/fmt`      | `from_int/from_float/from_bool` → string, `pad_left`. Build strings: `"count: " + fmt.from_int(n)`. |
| `std/time`     | `now/now_ms` (unix), `sleep(ms)`, `format`, `year/month/day/hour/minute/second`. |
| `std/fs`       | `read_file/write_file/append_file`, `exists/remove`, `read_line` (stdin). |
| `std/env`      | `env.read("NAME")` → string (empty if unset). |
| `std/mem`      | Arena memory for long-running workers: `mem.scope()` … `mem.release(a)` (servers do this per request automatically). |

A complete HTTP handler that reads the database and returns JSON:

```vader
import "std/http"
import "std/db"
import "std/env"
import "std/json"

public fn listUsers(s Server) {
    DB conn = db.open(env.read("DATABASE_URL"))
    Rows rows = db.query(conn, "SELECT id, name FROM users ORDER BY id")
    Json arr = json.array()
    for db.next(rows) {
        Json u = json.object()
        json.set_int(u, "id", db.col_int(rows, 0))
        json.set_str(u, "name", db.col_text(rows, 1))
        json.add(arr, u)
    }
    db.close(conn)
    http.json(s, 200, json.encode(arr))     // JSON is the default content type
}

public fn main() {
    Router r = newRouter()
    r.get("/users", listUsers)
    serve(8080, r)                          // unmatched routes return 404
}
```

---

## Connecting to a database

`import "std/db"` exposes embedded drivers for **SQLite, PostgreSQL and MySQL** behind one API.
The engine is chosen by the DSN, and the relevant C driver is linked into your binary at compile
time — there is no separate client library to install.

| Engine     | DSN example                                              | Notes                                            |
|------------|----------------------------------------------------------|--------------------------------------------------|
| SQLite     | `/var/data/app.db`  (a file path)                        | Embedded, zero setup.                            |
| PostgreSQL | `postgres://user:pass@host:5432/dbname`                  | MD5 and SCRAM-SHA-256 authentication.            |
| MySQL      | `mysql://user:pass@host:3306/dbname`                     | `mysql_native_password` and `caching_sha2`.      |

The scaffolds read the DSN from `DATABASE_URL`, so the same code runs against any engine — you
change one environment variable, not the source:

```vader
import "std/db"
import "std/env"

public fn main() {
    DB conn = db.open(env.read("DATABASE_URL"))
    db.must(conn, "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")

    Rows rows = db.query(conn, "SELECT id, name FROM users ORDER BY id")
    for db.next(rows) {
        print(db.col_int(rows, 0), db.col_text(rows, 1))   // also db.col_float
    }
    db.close(conn)
}
```

- `db.exec(conn, sql)` runs a statement and returns an `error`.
- `db.must(conn, sql)` runs it and aborts the process if it fails (use for startup migrations).
- `db.query(conn, sql)` returns `Rows`; iterate with `for db.next(rows)` and read columns by index.

**Parameterized queries** — never concatenate user input into SQL. Use `?` placeholders and bind
(safe and identical across engines):

```vader
Stmt st = db.prepare(conn, "INSERT INTO users (name) VALUES (?)")
db.bind_str(st, name)        // also db.bind_int / db.bind_float
db.run(st)                   // or db.query_stmt(st): Rows
```

**MongoDB** uses a separate package, with `Json` documents:

```vader
import "std/mongo"
import "std/json"

Mongo m = mongo.connect("mongodb://user:pass@127.0.0.1:27017/mydb")   // SCRAM-SHA-256
Json doc = json.object()
json.set_str(doc, "name", "Ada")
mongo.insert(m, "users", doc)                          // returns error
Json all = mongo.find(m, "users", json.object())       // filter {} = everything
mongo.update(m, "users", filter, changes)              // $set
mongo.delete(m, "users", filter)
mongo.close(m)
```

**Migrations** are managed with `vader migrate`:

```bash
vader migrate new add_users          # create a timestamped migration
vader migrate up   --db "$DATABASE_URL"
vader migrate status
vader migrate down --db "$DATABASE_URL"
```

More detail: [`docs/persistence.md`](docs/persistence.md).

---

## Testing

Tests live next to the code in `test "…" { … }` blocks and run with `vader test`. The runner is
**native** — it compiles your tests and runs them directly, with no other tooling required:

```vader
fn add(a, b int): int { return a + b }

test "add sums two numbers" {
    assert add(2, 3) == 5
}

test "add is commutative" {
    assert add(1, 2) == add(2, 1)
}
```

```bash
vader test                       # run every test in the project + report coverage
vader test --min-coverage 90     # raise the bar for this run
vader test --no-gate             # report coverage but don't fail on it
```

Each test is isolated: a failing `assert` fails only that test, the rest keep running, and the
process exits non-zero so CI catches it. Coverage is measured per function; set
`coverage_gate = true` in `vader.toml` to fail the build when it drops below `min_coverage`.

---

## Architecture enforcement

The architecture you scaffold isn't a suggestion. `vader build`, `vader run` and `vader check`
read the `architecture` from `vader.toml` and **fail the build** on a violation, pointing at the
offending file and rule:

- **Layering** — an inner layer cannot import an outer one (`domain` importing `infrastructure`
  is rejected), and a pure layer cannot perform I/O directly (importing `std/db` or `std/http`
  from `domain` is rejected). Each architecture defines its own layer order.
- **Naming** — every `.vd` file must be `snake_case`; `UserHandler.vd` is rejected in favour of
  `user_handler.vd`.

```text
$ vader build .
🔴 error [R1] domain/user.vd: `domain` (inner) cannot import `infrastructure` (outer) — the dependency must point inward
🔴 error [N1] interfaces/UserHandler.vd: file name is not snake_case (use lowercase letters, digits and `_`)
convention/architecture violation(s) — aborting.
```

The full rule set per architecture is in [`docs/architecture-rules.md`](docs/architecture-rules.md).
You can also run the checks on their own with `vader lint <file> [--arch <a>]`.

---

## Building & deploying

`vader build` produces a native binary; `--out` writes it without running, which is what the
generated Dockerfile uses:

```bash
vader build --out server .       # ./server, ready to ship
```

Scaffolded services come with a multi-stage **Dockerfile** (build natively, then run on a slim
libc base) and a **docker-compose.yml** that brings up the app together with the database you
chose, with `DATABASE_URL` already wired across the network:

```bash
docker compose up                # app on :8080 + its database
```

For DB drivers that need TLS / RSA (PostgreSQL over TLS, MySQL `caching_sha2` cold-cache auth),
build with `vader build --tls` to link OpenSSL.

---

## Toolchain reference

| Command | What it does |
|---|---|
| `vader new <kind> <name> [--arch <a>] [--db <e>]` | Scaffold a project (layered + test-covered). |
| `vader new --template <t> <name>` | Scaffold from a saved template. |
| `vader template list \| save <t> <dir>` | Manage reusable templates. |
| `vader gen <fn\|struct\|usecase\|handler> <Name>` | Generate an artifact + its mirror test. |
| `vader build [path] [--out <p>] [--tls]` | Compile to a native binary (defaults to the project / `main.vd`). |
| `vader run [path] [--tls]` | Compile and run. |
| `vader test [path] [--min-coverage n] [--no-gate]` | Run `test` blocks + coverage gate (native). |
| `vader check [path]` | Type-check + enforce the architecture rules. |
| `vader fmt [-w] <file.vd>` | Format code (stdout, or `-w` to rewrite). |
| `vader lint <file.vd> [--arch <a>]` | Run the architecture rules on a file. |
| `vader migrate <new\|gen\|status\|up\|down>` | Manage SQL migrations. |
| `vader add <git-url\|path>[@version] [name]` | Add a dependency (`vader remove <name>` to drop it). |
| `vader publish [--registry <dir\|git-url>]` | Register a package in a registry. |
| `vader llvm <file.vd> [--out <p>] [--tls]` | Low-level alias: compile via LLVM IR + `clang` and run. |
| `vader lsp` | Language server over stdio (used by the editor). |
| `vader lex \| parse <file.vd>` | Inspect the token stream / AST (debugging). |
| `vader version` | Print the version. |

Dependencies go in `vader.toml`'s `[dependencies]` (git URL or local path, optional `@version`),
resolved commits are pinned in `vader.lock`, and packages are cached under `~/.vader/pkg/`. Each
dependency is its own namespace, so two packages can use the same type name safely. See
[`docs/packages.md`](docs/packages.md).

---

## VS Code extension

**[Vader Language on the VS Code Marketplace →](https://marketplace.visualstudio.com/items?itemName=Vader.vader)**

```bash
code --install-extension Vader.vader
```

Syntax highlighting for `.vd`, real-time parse and type errors via the language server
(`vader lsp` — the compiler itself), and right-click code generation. Under WSL, install the
extension in the WSL window so it uses the same `vader` binary as your shell. Source and setup:
[`editors/vscode/`](editors/vscode/).

---

## Repository layout

```
vader/
├── src/            # the compiler (Rust): lexer, parser, check, llvm, lsp, gen, scaffold, …
├── runtime/        # embedded C: concurrency, HTTP, JSON, DB drivers, test harness, arena
├── examples/       # runnable .vd programs
├── editors/vscode/ # VS Code extension (highlighting + LSP client + codegen)
├── docs/           # grammar, architectures, persistence, packages, distribution
├── benchmarks/     # the benchmark sources + runner
├── tests/          # conformance corpus (golden-output integration tests)
├── SPEC.md         # language specification + roadmap
└── Cargo.toml
```

Every program in [`examples/`](examples/) runs with `vader run`: `hello`, `basics`, `shapes`
(enum + match), `slices`, `maps`, `interfaces`, `generics_demo`, `concurrency`, `api_usecase`,
and `db_sqlite` / `db_postgres` / `db_mysql`.

---

## Documentation

- [`SPEC.md`](SPEC.md) — language specification + roadmap.
- [`docs/grammar.md`](docs/grammar.md) — the full grammar.
- [`docs/architectures.md`](docs/architectures.md) · [`docs/architecture-rules.md`](docs/architecture-rules.md) — project layouts and the enforced rules.
- [`docs/persistence.md`](docs/persistence.md) — databases & migrations.
- [`docs/packages.md`](docs/packages.md) — the package manager.
- [`docs/distribution.md`](docs/distribution.md) — releases, Homebrew, winget, Docker.
- [`docs/llm-onboarding.md`](docs/llm-onboarding.md) — one-page primer for AI assistants.

---

## For LLMs / AI coding

Vader is new, so AI assistants don't know it out of the box. Rather than feeding them the whole
docs, paste [`docs/llm-onboarding.md`](docs/llm-onboarding.md) — a dense, self-contained
one-pager (syntax, standard library, idioms, gotchas and a complete REST example) that gets a
model writing correct Vader immediately.

---

## License

[MIT](LICENSE) © 2026 Marcos Smeets.
