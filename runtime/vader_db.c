/* Camada `std/db` do runtime da Vader: API única (open, exec, query, next,
 * col_int/col_text/col_float, close) que despacha pro driver certo conforme o DSN:
 *   - "postgres://..."/"postgresql://..."  -> driver Postgres (runtime/vader_pg.c)
 *   - qualquer outra coisa (caminho/arquivo) -> SQLite embarcado (runtime/sqlite/)
 *
 * Os handles carregam uma tag interna; pro IR continuam sendo i8* opacos.
 * Sem-GC: strings retornadas vazam, alinhado com o runtime. */
#include "sqlite3.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* driver Postgres (vader_pg.c) — visto como ponteiros opacos aqui */
extern void *vader_pg_connect(const char *dsn);
extern const char *vader_pg_exec(void *c, const char *sql);
extern void *vader_pg_query(void *c, const char *sql);
extern int vader_pg_next(void *r);
extern const char *vader_pg_text(void *r, int col);
extern void vader_pg_close(void *c);

enum { K_SQLITE = 0, K_PG = 1 };
typedef struct {
    int kind;
    void *h;
} VDB;
typedef struct {
    int kind;
    void *h;
} VRows;

static int is_pg(const char *dsn) {
    return strncmp(dsn, "postgres://", 11) == 0 ||
           strncmp(dsn, "postgresql://", 13) == 0;
}

void *vader_db_open(const char *dsn) {
    VDB *d = malloc(sizeof(VDB));
    if (is_pg(dsn)) {
        d->kind = K_PG;
        d->h = vader_pg_connect(dsn);
        return d;
    }
    sqlite3 *db = 0;
    if (sqlite3_open(dsn, &db) != SQLITE_OK) {
        if (db) sqlite3_close(db);
        free(d);
        return 0;
    }
    d->kind = K_SQLITE;
    d->h = db;
    return d;
}

const char *vader_db_exec(void *handle, const char *sql) {
    VDB *d = handle;
    if (!d) return strdup("conexão nula");
    if (d->kind == K_PG) return vader_pg_exec(d->h, sql);
    char *err = 0;
    int rc = sqlite3_exec((sqlite3 *)d->h, sql, 0, 0, &err);
    if (rc != SQLITE_OK) {
        const char *msg = strdup(err ? err : "erro SQL desconhecido");
        if (err) sqlite3_free(err);
        return msg;
    }
    return 0;
}

void *vader_db_query(void *handle, const char *sql) {
    VDB *d = handle;
    if (!d) return 0;
    VRows *r = malloc(sizeof(VRows));
    r->kind = d->kind;
    if (d->kind == K_PG) {
        r->h = vader_pg_query(d->h, sql);
        return r;
    }
    sqlite3_stmt *stmt = 0;
    if (sqlite3_prepare_v2((sqlite3 *)d->h, sql, -1, &stmt, 0) != SQLITE_OK) {
        free(r);
        return 0;
    }
    r->h = stmt;
    return r;
}

int vader_db_next(void *rowsh) {
    VRows *r = rowsh;
    if (!r) return 0;
    if (r->kind == K_PG) return vader_pg_next(r->h);
    int rc = sqlite3_step((sqlite3_stmt *)r->h);
    if (rc == SQLITE_ROW) return 1;
    sqlite3_finalize((sqlite3_stmt *)r->h);
    return 0;
}

long long vader_db_col_int(void *rowsh, int col) {
    VRows *r = rowsh;
    if (!r) return 0;
    if (r->kind == K_PG) return atoll(vader_pg_text(r->h, col));
    return sqlite3_column_int64((sqlite3_stmt *)r->h, col);
}

double vader_db_col_float(void *rowsh, int col) {
    VRows *r = rowsh;
    if (!r) return 0;
    if (r->kind == K_PG) return atof(vader_pg_text(r->h, col));
    return sqlite3_column_double((sqlite3_stmt *)r->h, col);
}

const char *vader_db_col_text(void *rowsh, int col) {
    VRows *r = rowsh;
    if (!r) return strdup("");
    if (r->kind == K_PG) return vader_pg_text(r->h, col);
    const unsigned char *t = sqlite3_column_text((sqlite3_stmt *)r->h, col);
    return strdup(t ? (const char *)t : "");
}

/* exec que aborta (exit 1) se o SQL falhar — útil pra migrations e scripts. */
void vader_db_must(void *handle, const char *sql) {
    const char *err = vader_db_exec(handle, sql);
    if (err) {
        fprintf(stderr, "erro de SQL: %s\n", err);
        exit(1);
    }
}

void vader_db_close(void *handle) {
    VDB *d = handle;
    if (!d) return;
    if (d->kind == K_PG) {
        vader_pg_close(d->h);
    } else {
        sqlite3_close((sqlite3 *)d->h);
    }
}
