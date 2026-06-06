# Vader — Catálogo de Arquiteturas

> A Vader gera e **fiscaliza** múltiplas arquiteturas. Cada uma = **template** (árvore de
> pastas + arquivos-semente) + **ruleset** (regras que o linter aplica).
> O projeto declara a sua em `vader.toml`; o linter aplica o ruleset correspondente.
> Status: `draft v0.1`. Conceitos de governança compartilhados: [`architecture-rules.md`](./architecture-rules.md).

---

## Dois eixos independentes

| Eixo | Valores | O que muda |
|---|---|---|
| **Kind** | `api`, `worker`, `cli`, `lib` | só o entrypoint (servidor HTTP, loop de worker, `main`) |
| **Architecture** | `clean`, `hexagonal`, `mvc`, `minimal` | estrutura de pastas + ruleset fiscalizado |

```sh
vader new api minha-api --arch hexagonal
```

Padrões por kind: `api` → `clean` · `worker` → `clean` · `cli` → `minimal` · `lib` → `minimal`.

```toml
# vader.toml
[project]
name         = "minha-api"
kind         = "api"
architecture = "clean"   # define qual ruleset o linter aplica
```

Severidade: 🔴 erro (barra o build) · 🟡 warning. Detalhe em `architecture-rules.md`.

---

## 1. Clean Architecture  (`--arch clean`)

Dependência aponta pra dentro; `domain` puro. Detalhada em
[`architecture-rules.md`](./architecture-rules.md).

```
domain/ ◀── usecase/ ◀── adapter/ ◀── infra/
```

Resumo do ruleset: R1 import pra dentro (🔴) · R2 use_case sem I/O direto (🔴) ·
R3 domain sem I/O (🔴) · R5 use_case em domain/ (🟡) · R6 repo/gateway com regra de negócio (🟡).

---

## 2. Hexagonal — Ports & Adapters  (`--arch hexagonal`)

O "hexágono" (core) no centro; o mundo entra por **driving adapters** (entrada) e sai por
**driven adapters** (saída). Tudo passa por **portas**.

```
meu-api/
├── vader.toml
├── cmd/
│   └── main.vd                       # composition root
├── core/                             # o hexágono — puro, sem I/O
│   ├── domain/                       # entidades, value objects
│   │   ├── user.vd
│   │   └── user_test.vd
│   ├── port/
│   │   ├── inbound/                  # portas de entrada (casos de uso expostos)
│   │   │   └── register_user.vd
│   │   └── outbound/                 # portas de saída (repo, gateway)
│   │       ├── user_repository.vd
│   │       └── address_gateway.vd
│   └── service/                      # implementa portas inbound (regra de negócio)
│       └── register_user_service.vd
└── adapter/
    ├── inbound/                      # driving: http, cli, consumer de fila
    │   └── http/user_handler.vd
    └── outbound/                     # driven: db, api externa
        ├── db/user_repository_pg.vd
        └── http/address_gateway_http.vd
```

**Ruleset:**
| # | Regra | Sev |
|---|---|---|
| H1 | `core` importa `adapter` | 🔴 erro |
| H2 | `core/domain` faz I/O ou importa pacote de I/O | 🔴 erro |
| H3 | `service` usa impl concreta em vez de porta outbound | 🔴 erro |
| H4 | `adapter/inbound` chama `adapter/outbound` direto (deve passar pelo core) | 🟡 warning |
| H5 | porta declarada fora de `core/port/` | 🟡 warning |

---

## 3. MVC — Model / View / Controller  (`--arch mvc`)

Para apps web tradicionais. Governança mais leve.

```
meu-app/
├── vader.toml
├── cmd/
│   └── main.vd
├── model/                            # dados + regra de negócio
│   ├── user.vd
│   └── user_test.vd
├── view/                             # apresentação (serializers / DTO de saída)
│   └── user_view.vd
├── controller/                       # orquestra request → model → view
│   ├── user_controller.vd
│   └── user_controller_test.vd
└── infra/                            # db, clients (opcional)
    └── db/user_store.vd
```

**Ruleset:**
| # | Regra | Sev |
|---|---|---|
| M1 | `model` importa `controller` ou `view` (o model é o núcleo) | 🔴 erro |
| M2 | `view` acessa `infra`/banco direto | 🔴 erro |
| M3 | `controller` carrega regra de negócio pesada (devia estar no model) | 🟡 warning |
| M4 | `view` acessa `model` sem passar pelo `controller` | 🟡 warning |

---

## 4. Minimal / flat  (`--arch minimal`)

Para `lib`, `cli` e scripts. **Sem regras de arquitetura** — só as garantias universais
(teste espelho auto-gerado, `fmt`, `lint` básico).

```
meu-cli/
├── vader.toml
├── cmd/
│   └── main.vd
└── src/
    ├── foo.vd
    └── foo_test.vd
```

**Ruleset:** nenhuma regra de camada. Liberdade total de organização.

---

## Garantias universais (valem em TODA arquitetura)

- Cada função nasce/atualiza seu `*_test.vd` espelho (TDD por padrão).
- `vader fmt` — formatação única.
- `vader lint` — checa o ruleset da arquitetura declarada + boas práticas gerais.
