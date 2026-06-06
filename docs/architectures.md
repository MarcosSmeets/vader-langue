# Vader — Architecture Catalog

> Vader generates and **enforces** multiple architectures. Each one = **template** (folder tree
> + seed files) + **ruleset** (rules the linter applies).
> The project declares its own in `vader.toml`; the linter applies the matching ruleset.
> Status: `draft v0.1`. Shared governance concepts: [`architecture-rules.md`](./architecture-rules.md).

---

## Two independent axes

| Axis | Values | What changes |
|---|---|---|
| **Kind** | `api`, `worker`, `cli`, `lib` | only the entrypoint (HTTP server, worker loop, `main`) |
| **Architecture** | `clean`, `hexagonal`, `mvc`, `minimal` | folder structure + enforced ruleset |

```sh
vader new api my-api --arch hexagonal
```

Defaults by kind: `api` → `clean` · `worker` → `clean` · `cli` → `minimal` · `lib` → `minimal`.

```toml
# vader.toml
[project]
name         = "my-api"
kind         = "api"
architecture = "clean"   # defines which ruleset the linter applies
```

Severity: 🔴 error (blocks the build) · 🟡 warning. Detail in `architecture-rules.md`.

---

## 1. Clean Architecture  (`--arch clean`)

Dependencies point inward; `domain` is pure. Detailed in
[`architecture-rules.md`](./architecture-rules.md).

```
domain/ ◀── usecase/ ◀── adapter/ ◀── infra/
```

Ruleset summary: R1 inward import (🔴) · R2 use_case with no direct I/O (🔴) ·
R3 domain with no I/O (🔴) · R5 use_case in domain/ (🟡) · R6 repo/gateway with a business rule (🟡).

---

## 2. Hexagonal — Ports & Adapters  (`--arch hexagonal`)

The "hexagon" (core) in the center; the world comes in through **driving adapters** (input) and goes out through
**driven adapters** (output). Everything goes through **ports**.

```
my-api/
├── vader.toml
├── cmd/
│   └── main.vd                       # composition root
├── core/                             # the hexagon — pure, no I/O
│   ├── domain/                       # entities, value objects
│   │   ├── user.vd
│   │   └── user_test.vd
│   ├── port/
│   │   ├── inbound/                  # inbound ports (exposed use cases)
│   │   │   └── register_user.vd
│   │   └── outbound/                 # outbound ports (repo, gateway)
│   │       ├── user_repository.vd
│   │       └── address_gateway.vd
│   └── service/                      # implements inbound ports (business rule)
│       └── register_user_service.vd
└── adapter/
    ├── inbound/                      # driving: http, cli, queue consumer
    │   └── http/user_handler.vd
    └── outbound/                     # driven: db, external api
        ├── db/user_repository_pg.vd
        └── http/address_gateway_http.vd
```

**Ruleset:**
| # | Rule | Sev |
|---|---|---|
| H1 | `core` imports `adapter` | 🔴 error |
| H2 | `core/domain` does I/O or imports an I/O package | 🔴 error |
| H3 | `service` uses a concrete impl instead of an outbound port | 🔴 error |
| H4 | `adapter/inbound` calls `adapter/outbound` directly (must go through the core) | 🟡 warning |
| H5 | port declared outside `core/port/` | 🟡 warning |

---

## 3. MVC — Model / View / Controller  (`--arch mvc`)

For traditional web apps. Lighter governance.

```
my-app/
├── vader.toml
├── cmd/
│   └── main.vd
├── model/                            # data + business rule
│   ├── user.vd
│   └── user_test.vd
├── view/                            # presentation (serializers / output DTO)
│   └── user_view.vd
├── controller/                      # orchestrates request → model → view
│   ├── user_controller.vd
│   └── user_controller_test.vd
└── infra/                            # db, clients (optional)
    └── db/user_store.vd
```

**Ruleset:**
| # | Rule | Sev |
|---|---|---|
| M1 | `model` imports `controller` or `view` (the model is the core) | 🔴 error |
| M2 | `view` accesses `infra`/database directly | 🔴 error |
| M3 | `controller` carries heavy business logic (should be in the model) | 🟡 warning |
| M4 | `view` accesses `model` without going through the `controller` | 🟡 warning |

---

## 4. Minimal / flat  (`--arch minimal`)

For `lib`, `cli`, and scripts. **No architecture rules** — only the universal guarantees
(auto-generated mirror test, `fmt`, basic `lint`).

```
my-cli/
├── vader.toml
├── cmd/
│   └── main.vd
└── src/
    ├── foo.vd
    └── foo_test.vd
```

**Ruleset:** no layer rules. Total freedom of organization.

---

## Universal guarantees (apply in EVERY architecture)

- Each function is born with / updates its mirror `*_test.vd` (TDD by default).
- `vader fmt` — single formatting.
- `vader lint` — checks the declared architecture's ruleset + general best practices.
