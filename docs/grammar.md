# Vader — Grammar and Syntax

> Status: `draft` — decisions marked **(proposal)** are open to change.
> Keywords in **English**. Go style: no `;`, no parentheses in `if`/`for`.
> See concrete examples in [`../examples/`](../examples/).

---

## 1. Comments

```vader
// line comment
/* block
   comment */
```

## 2. Variables

**Strong, explicit typing, always.** No `let`/`var`/`mut` — the type comes first,
C-style. There is no type inference on declaration.

```vader
int    count = 0
string name  = "Vader"
bool   active = true
float  ratio = 1.5

count = count + 1       // reassignment does not repeat the type
```

Constants use `const`:

```vader
const int MAX_RETRIES = 3
```

In multiple return, each variable carries its type; use `_` to discard:

```vader
int result, error err = divide(10, 2)
_, error e2 = divide(10, 0)
```

## 3. Types

| Category | Types |
|---|---|
| Primitives | `int`, `float`, `bool`, `string` |
| Composite  | `struct`, `[]T` (slice), `map[K]V`, `chan[T]` |
| Special    | `error`, `nil` |

## 4. Functions

The return type comes **after `:`**. Consecutive parameters of the same type can be
**grouped** (`a, b int`). No `:` = returns nothing.

**Visibility:** `public` or `private` before the declaration (applies to `fn` and `struct`,
and later `enum`/`interface`/`const`). With no modifier, the default is **`private`**.

```vader
public fn api(): int { return 1 }   // exported from the package
private fn helper() { }             // only inside the package (= the default)
```

```vader
fn add(a, b int): int {           // a and b are int; returns int
    return a + b
}

// Multiple returns (idiomatic for errors):
fn divide(a, b int): (int, error) {
    if b == 0 {
        return 0, error("division by zero")
    }
    return a / b, nil
}

fn main() { }                     // no return
```

## 5. Structs, methods, and interfaces

```vader
struct User {
    id   int
    name string
}

// Method: receiver between `fn` and the name.
fn (u User) greeting(): string {
    return "Hi, " + u.name
}

interface UserRepository {
    fn save(user User): (User, error)
}
```

## 6. Control flow

```vader
if x > 0 {
    ...
} else if x == 0 {
    ...
} else {
    ...
}
```

**Only `for` exists** — like Go, a single loop covers all cases (there is no
separate `while`/`do-while`/`foreach`):

```vader
for i in 0..10  { ... }   // exclusive range  → 0,1,...,9
for i in 0..=10 { ... }   // inclusive range  → 0,1,...,10
for item in list { ... }  // iteration over a collection
for x > 0 { ... }         // conditional (the "while" role)
for { ... }               // infinite loop
```

## 7. Pattern matching (`match`)

`match` matches a value against patterns: literals, value lists, guards (`if`), and the
`_` wildcard. It can be used as an **expression** (returns a value).

```vader
string label = match status {
    200:            "ok"
    400, 404:       "client error"
    n if n >= 500:  "server error"
    _:              "unknown"
}
```

> Natural companion of sum types (`enum`/union) — see open decisions.

## 8. Modules and imports

A **package** is a folder of `.vd` files in the same namespace. A **module** is the project,
with its name declared in `vader.toml`. You import by path; the stdlib lives under `std/`.
You use the package-qualified symbol (`fmt.println(...)`).

```vader
import "std/fmt"

// grouped:
import (
    "std/fmt"
    "std/db/postgres"     // built-in driver — no external lib to install
    "myapp/domain"        // the project's own package
)

fn main() {
    fmt.println("hi")
}
```

## 9. Errors

Explicit, as a return value. No exceptions.

```vader
int result, error err = divide(10, 2)
if err != nil {
    return err
}
```

## 10. Concurrency **(proposal)**

```vader
chan[int] jobs = chan[int](100)   // creates a buffered channel
spawn worker(jobs)                // launches concurrent (lightweight) execution
jobs <- 42                        // send
int v = <-jobs                    // receive
close(jobs)
```

## 11. Tests (first-class citizen)

Native `test` block. `*_test.vd` files are generated automatically as a mirror.

```vader
test "add returns the sum" {
    let got = add(2, 3)
    assert got == 5
}
```

## 12. Generics

Types and functions can be parameterized with `[T]`. Constraints via interface.

```vader
struct List[T] {
    items []T
}

fn map[T, U](xs []T, f fn(T): U): []U {
    []U out = []
    for x in xs {
        out = append(out, f(x))
    }
    return out
}

// constraint: T must satisfy the Ordered interface
fn max[T Ordered](a, b T): T {
    if a > b { return a }
    return b
}

// the Repository port stops repeating code per type:
interface Repository[T] {
    fn save(item T): (T, error)
    fn findById(id int): (T, error)
}
```

## 13. Sum types (`enum`)

`enum` models a value that is "one of several", optionally carrying data. A `match`
over an enum is **exhaustive**: the compiler forces you to handle every case (or use `_`).

```vader
enum Shape {
    Circle(radius float)
    Rectangle(width float, height float)
    Point
}

fn area(s Shape): float {
    return match s {
        Circle(r):       3.14159 * r * r
        Rectangle(w, h): w * h
        Point:           0.0
        // if a case is missing and there is no `_`, it's a compile ERROR
    }
}
```

> With generics, you can model `Option[T]` and `Result[T]` in the language itself.

## 14. Grammar sketch (simplified EBNF)

```ebnf
program    = { import } { declaration } ;
import     = "import" ( string | "(" { string } ")" ) ;
declaration= [ visibility ] ( funcDecl | structDecl | interfaceDecl | enumDecl ) | testDecl ;
visibility = "public" | "private" ;                            // default: private
typeParams = "[" ident [ type ] { "," ident [ type ] } "]" ;   // [T], [T Ordered]

funcDecl   = "fn" [ receiver ] ident [ typeParams ] "(" [ params ] ")" [ ":" retType ] block ;
receiver   = "(" ident type ")" ;
params     = paramGroup { "," paramGroup } ;
paramGroup = ident { "," ident } type ;            // grouped types: a, b int
retType    = type | "(" type { "," type } ")" ;

structDecl = "struct" ident [ typeParams ] "{" { ident type } "}" ;
interfaceDecl = "interface" ident [ typeParams ] "{" { funcSig } "}" ;
enumDecl   = "enum" ident [ typeParams ] "{" { variant } "}" ;
variant    = ident [ "(" params ")" ] ;
testDecl   = "test" string block ;

block      = "{" { statement } "}" ;
statement  = varDecl | assign | ifStmt | forStmt
           | returnStmt | matchExpr | exprStmt ;
varDecl    = [ "const" ] type ident { "," type ident } "=" expr { "," expr } ;
assign     = ident "=" expr ;
ifStmt     = "if" expr block [ "else" ( ifStmt | block ) ] ;
forStmt    = "for" [ ident "in" expr | expr ] block ;   // the language's only loop
returnStmt = "return" [ expr { "," expr } ] ;
matchExpr  = "match" expr "{" { matchArm } "}" ;
matchArm   = pattern [ "if" expr ] ":" ( expr | block ) ;
pattern    = literal { "," literal } | ident | "_" ;
range      = expr ( ".." | "..=" ) expr ;               // exclusive | inclusive
```

> The complete, unambiguous grammar will be derived from this in Phase 1, along with the parser.

## 15. Syntax decisions still open

No structural decision pending for v1. Fine points to detail along with the parser
(Phase 1): slice/map literals, symbol visibility/export across packages,
concurrency model details.

## Decisions already settled

- **Strong, explicit typing**, C-style declaration (`int x = 0`). No `let`/`var`/`mut`; no inference on declaration.
- **Only `for`** as a loop (no `while`/`do`).
- **Functions:** return after `:` (`fn f(): int`), groupable parameters (`a, b int`). Explicit multiple return, tuple-style (`(int, error)`).
- **Channels:** `chan[int]`; creation `chan[int](buffer)`.
- **Range:** `0..10` exclusive **and** `0..=10` inclusive.
- **`match`** (pattern matching) in v1, **exhaustive** over `enum`.
- **Generics** (`[T]`, interface constraints) in v1.
- **`enum`/sum types** in v1.
- **Modules/imports:** package per folder, import by path, stdlib under `std/`.
- **Visibility:** `public`/`private` before the declaration; default **`private`**.
- Keywords in English.
