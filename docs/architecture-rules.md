# Vader — Regras de Arquitetura (governança embutida)

> A Vader **gera** projetos em arquiteturas opinativas e **fiscaliza** as convenções no
> compilador/linter. Este é o diferencial nº1 da linguagem.
> Este doc cobre os **conceitos de governança compartilhados** + o ruleset da **Clean
> Architecture** em detalhe. Catálogo das demais (Hexagonal, MVC, Minimal) em
> [`architectures.md`](./architectures.md).
> Status: `draft v0.1`.

---

## 1. Camadas e a Regra de Ouro

Dependências apontam **sempre para dentro**. O núcleo (`domain`) não conhece o mundo externo.

```
infra ──▶ adapter ──▶ usecase ──▶ domain
(frameworks, I/O)                  (núcleo puro, sem I/O)
```

| Camada | Pasta | O que vive aqui | Pode importar |
|---|---|---|---|
| **Domain** | `domain/` | Entidades, value objects, domain services, **portas** (interfaces) | nada |
| **Usecase** | `usecase/` | Orquestra regra de negócio via portas | `domain` |
| **Adapter** | `adapter/` | Handlers/controllers, presenters, mappers | `usecase`, `domain` |
| **Infra** | `infra/` | Implementações concretas: DB, HTTP, filas; **impl** de Repository/Gateway | todas |

> Ninguém importa `infra`. O `main`/injeção de dependência (composition root) é quem
> liga as implementações concretas às portas.

## 2. Persistência vs. mundo externo: Repository ≠ Gateway

Ambos são **portas** definidas no `domain` e **implementados** na `infra`. Ambos devolvem
**dado já consistente** (entidade de domínio) — o use_case não sabe a origem.

| | **Repository** | **Gateway** |
|---|---|---|
| Responsabilidade | Persistência dos **seus** dados (banco) | Falar com **sistemas externos** (APIs de terceiros, pagamento, e-mail) |
| Exemplos de método | `save`, `findById`, `delete`, `list` | `fetchAddress`, `charge`, `sendEmail` |
| Por que separado | Contrato, motivo de mudança e modo de falha diferentes — evita o *fat repository* |

### Pode morar dentro de Repository/Gateway (impl, na infra)
- I/O (SQL, HTTP), serialização, retries, cache, paginação.
- **Mapping**: traduzir linha crua / JSON → entidade de domínio. *Desejável* — o domínio nunca vê dado cru.

### NÃO pode (a Vader avisa)
- Regra de negócio, decisão, orquestração, cálculo de domínio → isso é `usecase`/`domain`.
- Regra prática: "normalizar/mapear formato" = ok aqui. "Aplicar regra de negócio" = não.

## 3. Regras fiscalizadas pelo compilador

Severidade: 🔴 **erro** barra o build · 🟡 **warning** não barra.

| # | Regra | Severidade |
|---|---|---|
| R1 | Camada interna importa externa (ex.: `domain` → `infra`) | 🔴 erro |
| R2 | `usecase` faz I/O direto (http/sql) em vez de usar porta | 🔴 erro |
| R3 | `domain` importa pacote de I/O (net/http, driver de banco) | 🔴 erro |
| R4 | `usecase` importa `adapter` ou `infra` | 🔴 erro |
| R5 | use_case/service declarado dentro de `domain/` | 🟡 warning |
| R6 | Repository/Gateway contém regra de negócio | 🟡 warning |
| R7 | Nomeação/local fora do padrão (ex.: arquivo em `usecase/` sem papel claro) | 🟡 warning |
| R8 | Entidade de domínio exposta com dado cru (sem mapping no boundary) | 🟡 warning |

> Severidades são o padrão; um futuro `vader.toml` poderá ajustar caso a caso.

## 4. Como a Vader sabe o "papel" de cada arquivo

Pela **convenção de pastas** que o `vader new` gera. A pasta declara a camada e as
regras se aplicam por camada. (Evolução futura possível: anotação explícita de papel.)

## 5. Árvore gerada por `vader new api <nome>` (preview)

```
meu-api/
├── vader.toml                  # config do projeto
├── cmd/
│   └── main.vd                 # composition root (liga portas → impl)
├── domain/
│   ├── user.vd                 # entidade
│   ├── user_test.vd            # teste (auto-gerado)
│   ├── user_repository.vd      # PORTA de persistência (interface)
│   └── address_gateway.vd      # PORTA de serviço externo (interface)
├── usecase/
│   ├── create_user.vd
│   └── create_user_test.vd     # teste (auto-gerado)
├── adapter/
│   └── http/
│       ├── user_handler.vd     # controller HTTP
│       └── user_handler_test.vd
└── infra/
    ├── db/
    │   └── user_repository_pg.vd   # IMPL do repository (Postgres)
    └── http/
        └── address_gateway_http.vd # IMPL do gateway (API externa)
```

Cada arquivo de função nasce com seu `*_test.vd` espelho — TDD por padrão.
