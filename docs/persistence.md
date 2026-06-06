# Vader — Persistence and Migrations

> **Batteries included:** the database drivers live in the **stdlib** and are already compiled into the
> binary. No `go get`, no `npm install`. Access still goes through the
> **Repository** port (the architecture governance doesn't break — the driver lives in infra).
> Status: `draft v0.1`. See [`architectures.md`](./architectures.md), [`architecture-rules.md`](./architecture-rules.md).

---

## 1. Databases supported out of the box

| Family | Databases | Import |
|---|---|---|
| SQL | **PostgreSQL**, **MySQL**, **SQLite** | `std/db/postgres`, `std/db/mysql`, `std/db/sqlite` |
| NoSQL | **MongoDB** | `std/db/mongo` |

> SQLite runs serverless — a great default for dev, tests, and embedded.

## 2. Connection configuration (`vader.toml`)

Named connections; the URL comes from an environment variable (secrets out of the code).

```toml
[database.main]
driver = "postgres"          # postgres | mysql | sqlite | mongo
url    = "env:DATABASE_URL"

[database.events]
driver = "mongo"
url    = "env:MONGO_URL"
```

## 3. Access — unified API (SQL family)

The SQL databases share the same interface; switching Postgres↔MySQL↔SQLite is changing the
`driver` in `vader.toml`. The Repository implementation (infra) uses `std/db`:

```vader
import "std/db"
import "myapp/domain"

// infra/db/user_repository_pg.vd — IMPL of the domain.UserRepository port
struct UserRepositoryPg {
    conn db.Conn
}

fn (r UserRepositoryPg) findById(id int): (domain.User, error) {
    domain.User u, error err = r.conn.queryOne[domain.User](
        "select id, name from users where id = $1", id)
    return u, err   // the row→entity mapping goes here, at the boundary
}
```

> `queryOne[T]` depends on the **generics** decision (open in the grammar). Persistence
> is the strongest case in favor of generics in v1.

> **Mongo** uses a document API (`collection`, `find`, `insert`) instead of SQL — same
> Repository port idea, its own document interface.

## 4. Migrations — hybrid mode (auto-generated, you review)

The default flow: you change the domain entity, Vader **generates the diff** as a migration,
and you **review it before applying**. Nothing goes to the database on its own.

### Commands
| Command | Does |
|---|---|
| `vader migrate gen <name>` | **Generates** a migration from the entity diff (review first) |
| `vader migrate new <name>` | Creates an **empty** migration (manual writing) |
| `vader migrate up` | Applies the pending ones |
| `vader migrate down` | Reverts the last one |
| `vader migrate status` | Shows applied vs pending |

### Format (SQL → versioned `.sql` files)
```
migrations/
├── 20260606_120000_create_users.up.sql
└── 20260606_120000_create_users.down.sql
```
```sql
-- ...up.sql  (generated from the diff, reviewable)
create table users (
    id   serial primary key,
    name text not null
);
```

### Mongo
Schemaless → a migration becomes a **Vader script** (`.vd`) to create indexes / transform data,
instead of DDL. Same `up`/`down`/`status` commands.

## 5. How this respects the architecture

- Driver and Repository implementation → **infra** layer.
- `usecase`/`domain` only know the **port** (interface) — the linter's R2/R3 rule still applies.
- Migrations are an infra/toolchain artifact, outside the domain.
