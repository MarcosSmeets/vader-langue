# Vader for LLMs — one-page primer

Paste this whole file to an LLM and it can write correct Vader without reading the rest of
the docs. Vader is a compiled, statically- and explicitly-typed language (`.vd` files) that
emits LLVM IR and links a native binary. Semantics are close to Go; ergonomics are its own.

## Build & run

```bash
vader llvm file.vd          # compile a single file natively and run it
vader llvm ./myproject      # compile a whole project directory and run it
vader llvm --out app .      # compile to ./app without running (for Docker/deploy)
vader run file.vd           # alternative: transpile to Go and run
vader test                  # run test blocks + coverage
vader new api myapi --arch tdd   # scaffold a ready-to-run REST API (prompts for the DB)
```

## Core rules (read these first)

- **Types are explicit and written first** (C-style). There is **no** `let`/`var`/`mut` and
  **no** type inference: `int x = 0`, `string name = "Vader"`, `bool ok = true`.
- **Reassignment omits the type**: `x = x + 1`.
- `public` exports a symbol; the default is **private**. `const int MAX = 3` for constants.
- The only loop is **`for`** (covers while / range / infinite). No parentheses around
  conditions. `if`/`else` likewise take no parentheses.
- Statements end at the newline (no semicolons). Blocks use `{ }`.
- Primitive types: `int` (64-bit), `float` (64-bit), `bool`, `string` (UTF-8), `error`, `nil`.
- Output: `print(a, b, ...)`.

## Syntax by example

```vader
// function: params grouped by type, return type(s) after the colon
fn add(a, b int): int {
    return a + b
}

// multiple return values + explicit errors
fn divide(a, b int): (int, error) {
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

    // for as while / range / infinite
    int i = 0
    for i < 3 { i = i + 1 }
    for n in 0..5 { print(n) }       // 0..5 exclusive, 0..=5 inclusive
    // for { ... }                   // infinite

    string s = "a" + "b"             // string concat with +
}
```

Structs, methods, enums + match, interfaces, generics, collections, concurrency:

```vader
public struct User { id int  name string }
fn (u User) greeting(): string { return "hi " + u.name }

enum Shape { Circle(float)  Rectangle(float, float)  Point }
fn area(s Shape): float {
    return match s {
        Circle(r): 3.14159 * r * r
        Rectangle(w, h): w * h
        Point: 0.0
    }
}

interface Animal { fn sound(): string }     // structs implement it by having the method

fn id[T](x T): T { return x }                // generics with [T]

[]int xs = []int{10, 20, 30}                 // slice literal; xs[0], len(xs)
map[string]int m = newmap()                  // map; m["k"] = 1
chan[int] ch = chan[int](0)                  // channel
spawn worker(ch)                             // goroutine
int v = <-ch                                 // receive;  ch <- 42  to send
```

`User{ id: 1, name: "Ada" }` constructs a struct. `Circle(2.0)` constructs an enum variant.

## Standard library

Imported with `import "std/x"`. Calls are written qualified (`http.listen`, `db.open`); the
package prefix is normalized away by the compiler, so don't worry about it. Opaque handle
types: `Server`, `DB`, `Rows`, `Json`, `Router`, `Arena`, `Stmt`, `Mongo`.

**std/http** — server, router and client:
```vader
import "std/http"
public fn hello(s Server) {
    http.respond(s, 200, "application/json", "{\"ok\":true}")
}
public fn main() {
    Router r = newRouter()
    r.get("/hello", hello)              // also r.post/r.put/r.delete(path, handler)
    serve(8080, r)                      // listen + dispatch; unmatched -> 404
}
// inside a handler: http.method(s), http.path(s), http.body(s), http.header(s, "Name")
// client: string body = http.get(url)   /   http.post(url, ctype, body)
```
Handlers are plain functions `fn name(s Server)` passed by name (Vader has first-class
function values). A request that matches no route returns 404 automatically.

**std/json**:
```vader
import "std/json"
Json o = json.object()
json.set_str(o, "name", "Ada")          // set_int/set_float/set_bool/set(o,key,childJson)
string out = json.encode(o)
Json p = json.parse("{\"age\": 30}")
int age = json.as_int(json.field(p, "age"))   // field/elem/as_str/as_int/as_float/as_bool/count
Json arr = json.array()
json.add_str(arr, "x")                  // add_int / add(arr, childJson)
```

**std/db** — one API for SQLite / Postgres / MySQL, selected by the DSN
(`postgres://...`, `mysql://...`, or a file path for SQLite):
```vader
import "std/db"
DB c = db.open("/tmp/app.db")
db.must(c, "CREATE TABLE IF NOT EXISTS users (id INTEGER, name TEXT)")  // exec-or-abort
db.exec(c, "INSERT INTO users VALUES (1, 'Ada')")                       // returns error
Rows rows = db.query(c, "SELECT id, name FROM users")
for db.next(rows) {
    int id = db.col_int(rows, 0)
    string name = db.col_text(rows, 1)     // also db.col_float
    print(id, name)
}
db.close(c)
```

Parameterized queries (safe, cross-DB) use `?` placeholders + bind:
```vader
Stmt st = db.prepare(c, "INSERT INTO users (name) VALUES (?)")
db.bind_str(st, name)        // also bind_int / bind_float
db.run(st)                   // or db.query_stmt(st): Rows
```

**std/mongo** — MongoDB document store (no auth; documents are `Json`):
```vader
import "std/mongo"
Mongo m = mongo.connect("mongodb://127.0.0.1:27017/mydb")        // or mongodb://user:pass@host/db (SCRAM-SHA-256)
Json doc = json.object()
json.set_str(doc, "name", "Ada")
mongo.insert(m, "users", doc)                    // returns error
Json results = mongo.find(m, "users", json.object())  // filter {} = all; returns a Json array
mongo.update(m, "users", filter, changes)        // $set `changes` on docs matching `filter`
mongo.delete(m, "users", filter)                 // delete docs matching `filter` ({} = all)
mongo.close(m)
```

**std/env** — `string v = env.read("DATABASE_URL")` (empty string if unset).

**std/mem** — arena memory for long-running workers (servers do this automatically):
```vader
import "std/mem"
Arena a = mem.scope()      // allocations until release() are bump-allocated in the arena
// ... do work ...
mem.release(a)             // free everything from this job at once (no GC)
```

## Gotchas

- No `let`/`var`/`auto` and no inference — always write the type on declaration.
- Conditions take **no** parentheses: `if x > 0 {`, `for i < n {`.
- Errors are values, not exceptions: return `(T, error)` and check `if err != nil`.
- `nil` is assignable to `error`, structs, slices, channels, enums.
- A program using the native stdlib (http/db/json/env/mem/router) must build with
  `vader llvm` (not the Go backend).
- Type names must be unique across a project (the module system flattens files).

## A complete REST endpoint (HTTP + DB + JSON)

```vader
import "std/http"
import "std/db"
import "std/env"
import "std/json"

public fn listUsers(s Server) {
    DB c = db.open(env.read("DATABASE_URL"))
    Rows rows = db.query(c, "SELECT id, name FROM users ORDER BY id")
    Json arr = json.array()
    for db.next(rows) {
        Json u = json.object()
        json.set_int(u, "id", db.col_int(rows, 0))
        json.set_str(u, "name", db.col_text(rows, 1))
        json.add(arr, u)
    }
    http.respond(s, 200, "application/json", json.encode(arr))
    db.close(c)
}

public fn health(s Server) {
    http.respond(s, 200, "application/json", "{\"status\":\"ok\"}")
}

public fn main() {
    Router r = newRouter()
    r.get("/health", health)
    r.get("/users", listUsers)
    serve(8080, r)
}
```
