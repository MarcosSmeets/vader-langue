# Vader — Persistência e Migrations

> **Batteries included:** os drivers de banco moram na **stdlib** e já vão compilados no
> binário. Sem `go get`, sem `npm install`. O acesso continua passando pela porta
> **Repository** (a governança de arquitetura não quebra — driver vive na infra).
> Status: `draft v0.1`. Ver [`architectures.md`](./architectures.md), [`architecture-rules.md`](./architecture-rules.md).

---

## 1. Bancos suportados de fábrica

| Família | Bancos | Import |
|---|---|---|
| SQL | **PostgreSQL**, **MySQL**, **SQLite** | `std/db/postgres`, `std/db/mysql`, `std/db/sqlite` |
| NoSQL | **MongoDB** | `std/db/mongo` |

> SQLite roda sem servidor — padrão ótimo pra dev, testes e embarcado.

## 2. Configuração da conexão (`vader.toml`)

Conexões nomeadas; a URL vem de variável de ambiente (segredos fora do código).

```toml
[database.main]
driver = "postgres"          # postgres | mysql | sqlite | mongo
url    = "env:DATABASE_URL"

[database.events]
driver = "mongo"
url    = "env:MONGO_URL"
```

## 3. Acesso — API unificada (família SQL)

Os bancos SQL compartilham a mesma interface; trocar Postgres↔MySQL↔SQLite é trocar o
`driver` no `vader.toml`. A implementação do Repository (infra) usa `std/db`:

```vader
import "std/db"
import "myapp/domain"

// infra/db/user_repository_pg.vd — IMPL da porta domain.UserRepository
struct UserRepositoryPg {
    conn db.Conn
}

fn (r UserRepositoryPg) findById(id int): (domain.User, error) {
    domain.User u, error err = r.conn.queryOne[domain.User](
        "select id, name from users where id = $1", id)
    return u, err   // mapping linha→entidade fica aqui, no boundary
}
```

> `queryOne[T]` depende da decisão de **genéricos** (em aberto na gramática). Persistência
> é o caso mais forte a favor de genéricos na v1.

> **Mongo** usa API de documentos (`collection`, `find`, `insert`) em vez de SQL — mesma
> ideia de porta Repository, interface própria de documento.

## 4. Migrations — modo híbrido (auto-gera, você revisa)

O fluxo padrão: você muda a entidade de domínio, a Vader **gera o diff** como migration,
e você **revisa antes de aplicar**. Nada vai pro banco sozinho.

### Comandos
| Comando | Faz |
|---|---|
| `vader migrate gen <nome>` | **Gera** migration a partir do diff das entidades (revisar antes) |
| `vader migrate new <nome>` | Cria migration **vazia** (escrita manual) |
| `vader migrate up` | Aplica as pendentes |
| `vader migrate down` | Reverte a última |
| `vader migrate status` | Mostra aplicadas vs pendentes |

### Formato (SQL → arquivos `.sql` versionados)
```
migrations/
├── 20260606_120000_create_users.up.sql
└── 20260606_120000_create_users.down.sql
```
```sql
-- ...up.sql  (gerado do diff, revisável)
create table users (
    id   serial primary key,
    name text not null
);
```

### Mongo
Schemaless → migration vira **script Vader** (`.vd`) pra criar índices / transformar dados,
em vez de DDL. Mesmos comandos `up`/`down`/`status`.

## 5. Como isso respeita a arquitetura

- Driver e implementação de Repository → camada **infra**.
- `usecase`/`domain` só conhecem a **porta** (interface) — regra R2/R3 do linter continua valendo.
- Migrations são artefato de infra/toolchain, fora do domínio.
