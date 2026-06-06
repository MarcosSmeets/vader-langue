# Vader — Gramática e Sintaxe

> Status: `draft` — decisões marcadas como **(proposta)** estão abertas pra mudar.
> Keywords em **inglês**. Estilo Go: sem `;`, sem parênteses em `if`/`for`.
> Veja exemplos concretos em [`../examples/`](../examples/).

---

## 1. Comentários

```vader
// comentário de linha
/* comentário
   de bloco */
```

## 2. Variáveis

**Tipagem forte e explícita, sempre.** Sem `let`/`var`/`mut` — o tipo vem na frente,
estilo C. Não há inferência de tipo na declaração.

```vader
int    count = 0
string name  = "Vader"
bool   active = true
float  ratio = 1.5

count = count + 1       // reatribuição não repete o tipo
```

Constantes usam `const`:

```vader
const int MAX_RETRIES = 3
```

Em retorno múltiplo, cada variável leva seu tipo; use `_` pra descartar:

```vader
int result, error err = divide(10, 2)
_, error e2 = divide(10, 0)
```

## 3. Tipos

| Categoria | Tipos |
|---|---|
| Primitivos | `int`, `float`, `bool`, `string` |
| Compostos  | `struct`, `[]T` (slice), `map[K]V`, `chan[T]` |
| Especiais  | `error`, `nil` |

## 4. Funções

Tipo de retorno vem **depois de `:`**. Parâmetros consecutivos do mesmo tipo podem ser
**agrupados** (`a, b int`). Sem `:` = não retorna nada.

**Visibilidade:** `public` ou `private` antes da declaração (vale para `fn` e `struct`,
e futuramente `enum`/`interface`/`const`). Sem modificador, o padrão é **`private`**.

```vader
public fn api(): int { return 1 }   // exportada do pacote
private fn helper() { }             // só dentro do pacote (= o padrão)
```

```vader
fn add(a, b int): int {           // a e b são int; retorna int
    return a + b
}

// Múltiplos retornos (idiomático pra erro):
fn divide(a, b int): (int, error) {
    if b == 0 {
        return 0, error("division by zero")
    }
    return a / b, nil
}

fn main() { }                     // sem retorno
```

## 5. Structs, métodos e interfaces

```vader
struct User {
    id   int
    name string
}

// Método: receiver entre `fn` e o nome.
fn (u User) greeting(): string {
    return "Hi, " + u.name
}

interface UserRepository {
    fn save(user User): (User, error)
}
```

## 6. Controle de fluxo

```vader
if x > 0 {
    ...
} else if x == 0 {
    ...
} else {
    ...
}
```

**Só existe `for`** — igual Go, um único laço cobre todos os casos (não há
`while`/`do-while`/`foreach` separados):

```vader
for i in 0..10  { ... }   // range exclusivo  → 0,1,...,9
for i in 0..=10 { ... }   // range inclusivo  → 0,1,...,10
for item in list { ... }  // iteração sobre coleção
for x > 0 { ... }         // condicional (papel do "while")
for { ... }               // laço infinito
```

## 7. Pattern matching (`match`)

`match` casa um valor contra padrões: literais, listas de valores, guardas (`if`) e o
curinga `_`. Pode ser usado como **expressão** (retorna valor).

```vader
string label = match status {
    200:            "ok"
    400, 404:       "client error"
    n if n >= 500:  "server error"
    _:              "unknown"
}
```

> Companheiro natural de tipos-soma (`enum`/union) — ver decisões abertas.

## 8. Módulos e imports

Um **pacote** é uma pasta de arquivos `.vd` no mesmo namespace. Um **módulo** é o projeto,
com nome declarado no `vader.toml`. Importa-se por caminho; a stdlib mora em `std/`.
Usa-se o símbolo qualificado pelo pacote (`fmt.println(...)`).

```vader
import "std/fmt"

// agrupado:
import (
    "std/fmt"
    "std/db/postgres"     // driver embutido — sem instalar lib externa
    "myapp/domain"        // pacote do próprio projeto
)

fn main() {
    fmt.println("hi")
}
```

## 9. Erros

Explícitos, como valor de retorno. Sem exceptions.

```vader
int result, error err = divide(10, 2)
if err != nil {
    return err
}
```

## 10. Concorrência **(proposta)**

```vader
chan[int] jobs = chan[int](100)   // cria canal com buffer
spawn worker(jobs)                // dispara execução concorrente (leve)
jobs <- 42                        // envia
int v = <-jobs                    // recebe
close(jobs)
```

## 11. Testes (cidadão de primeira classe)

Bloco `test` nativo. Arquivos `*_test.vd` são gerados automaticamente como espelho.

```vader
test "add returns the sum" {
    let got = add(2, 3)
    assert got == 5
}
```

## 12. Genéricos

Tipos e funções podem ser parametrizados com `[T]`. Restrições (constraints) por interface.

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

// constraint: T precisa satisfazer a interface Ordered
fn max[T Ordered](a, b T): T {
    if a > b { return a }
    return b
}

// a porta Repository deixa de repetir código por tipo:
interface Repository[T] {
    fn save(item T): (T, error)
    fn findById(id int): (T, error)
}
```

## 13. Tipos-soma (`enum`)

`enum` modela um valor que é "um entre vários", opcionalmente carregando dados. O `match`
sobre um enum é **exaustivo**: o compilador obriga tratar todos os casos (ou usar `_`).

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
        // se faltar um caso e não houver `_`, é ERRO de compilação
    }
}
```

> Com genéricos, dá pra modelar `Option[T]` e `Result[T]` na própria linguagem.

## 14. Esboço de gramática (EBNF simplificado)

```ebnf
program    = { import } { declaration } ;
import     = "import" ( string | "(" { string } ")" ) ;
declaration= [ visibility ] ( funcDecl | structDecl | interfaceDecl | enumDecl ) | testDecl ;
visibility = "public" | "private" ;                            // padrão: private
typeParams = "[" ident [ type ] { "," ident [ type ] } "]" ;   // [T], [T Ordered]

funcDecl   = "fn" [ receiver ] ident [ typeParams ] "(" [ params ] ")" [ ":" retType ] block ;
receiver   = "(" ident type ")" ;
params     = paramGroup { "," paramGroup } ;
paramGroup = ident { "," ident } type ;            // tipos agrupados: a, b int
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
forStmt    = "for" [ ident "in" expr | expr ] block ;   // único laço da linguagem
returnStmt = "return" [ expr { "," expr } ] ;
matchExpr  = "match" expr "{" { matchArm } "}" ;
matchArm   = pattern [ "if" expr ] ":" ( expr | block ) ;
pattern    = literal { "," literal } | ident | "_" ;
range      = expr ( ".." | "..=" ) expr ;               // exclusivo | inclusivo
```

> Gramática completa e sem ambiguidade será derivada disto na Fase 1, junto com o parser.

## 15. Decisões de sintaxe ainda abertas

Nenhuma decisão estrutural pendente para a v1. Pontos finos a detalhar junto com o parser
(Fase 1): literais de slice/map, visibilidade/export de símbolos entre pacotes,
detalhes do modelo de concorrência.

## Decisões já fechadas

- **Tipagem forte e explícita**, declaração estilo C (`int x = 0`). Sem `let`/`var`/`mut`; sem inferência na declaração.
- **Só `for`** como laço (sem `while`/`do`).
- **Funções:** retorno depois de `:` (`fn f(): int`), parâmetros agrupáveis (`a, b int`). Múltiplo retorno explícito estilo tupla (`(int, error)`).
- **Canais:** `chan[int]`; criação `chan[int](buffer)`.
- **Range:** `0..10` exclusivo **e** `0..=10` inclusivo.
- **`match`** (pattern matching) na v1, **exaustivo** sobre `enum`.
- **Genéricos** (`[T]`, constraints por interface) na v1.
- **`enum`/tipos-soma** na v1.
- **Módulos/imports:** pacote por pasta, import por caminho, stdlib em `std/`.
- **Visibilidade:** `public`/`private` antes da declaração; padrão **`private`**.
- Keywords em inglês.
