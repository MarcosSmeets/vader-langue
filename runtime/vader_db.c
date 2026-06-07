/* Vader runtime `std/db` layer: single API (open, exec, query, next,
 * col_int/col_text/col_float, close) that dispatches to the right driver based on the DSN:
 *   - "postgres://..."/"postgresql://..."  -> Postgres driver (runtime/vader_pg.c)
 *   - anything else (path/file) -> embedded SQLite (runtime/sqlite/)
 *
 * Handles carry an internal tag; to the IR they remain opaque i8*.
 * No GC: returned strings leak, in line with the runtime. */
#include "sqlite3.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* arena allocator (vader_mem.c): database reads are per-request scratch */
extern char *vader_strdup(const char *s);
extern void *vader_alloc(long n);
extern void *vader_realloc(void *old, long oldn, long newn);

/* Postgres driver (vader_pg.c) — seen as opaque pointers here */
extern void *vader_pg_connect(const char *dsn);
extern const char *vader_pg_exec(void *c, const char *sql);
extern void *vader_pg_query(void *c, const char *sql);
extern int vader_pg_next(void *r);
extern const char *vader_pg_text(void *r, int col);
extern void vader_pg_close(void *c);

/* MySQL driver (vader_mysql.c) */
extern void *vader_my_connect(const char *dsn);
extern const char *vader_my_exec(void *c, const char *sql);
extern void *vader_my_query(void *c, const char *sql);
extern int vader_my_next(void *r);
extern const char *vader_my_text(void *r, int col);
extern void vader_my_close(void *c);

enum { K_SQLITE = 0, K_PG = 1, K_MYSQL = 2 };
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
static int is_mysql(const char *dsn) {
    return strncmp(dsn, "mysql://", 8) == 0 || strncmp(dsn, "mariadb://", 10) == 0;
}

void *vader_db_open(const char *dsn) {
    VDB *d = malloc(sizeof(VDB));
    if (is_pg(dsn)) {
        d->kind = K_PG;
        d->h = vader_pg_connect(dsn);
        return d;
    }
    if (is_mysql(dsn)) {
        d->kind = K_MYSQL;
        d->h = vader_my_connect(dsn);
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
    if (!d) return vader_strdup("null connection");
    if (d->kind == K_PG) return vader_pg_exec(d->h, sql);
    if (d->kind == K_MYSQL) return vader_my_exec(d->h, sql);
    char *err = 0;
    int rc = sqlite3_exec((sqlite3 *)d->h, sql, 0, 0, &err);
    if (rc != SQLITE_OK) {
        const char *msg = vader_strdup(err ? err : "unknown SQL error");
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
    if (d->kind == K_MYSQL) {
        r->h = vader_my_query(d->h, sql);
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
    if (r->kind == K_MYSQL) return vader_my_next(r->h);
    int rc = sqlite3_step((sqlite3_stmt *)r->h);
    if (rc == SQLITE_ROW) return 1;
    sqlite3_finalize((sqlite3_stmt *)r->h);
    return 0;
}

long long vader_db_col_int(void *rowsh, int col) {
    VRows *r = rowsh;
    if (!r) return 0;
    if (r->kind == K_PG) return atoll(vader_pg_text(r->h, col));
    if (r->kind == K_MYSQL) return atoll(vader_my_text(r->h, col));
    return sqlite3_column_int64((sqlite3_stmt *)r->h, col);
}

double vader_db_col_float(void *rowsh, int col) {
    VRows *r = rowsh;
    if (!r) return 0;
    if (r->kind == K_PG) return atof(vader_pg_text(r->h, col));
    if (r->kind == K_MYSQL) return atof(vader_my_text(r->h, col));
    return sqlite3_column_double((sqlite3_stmt *)r->h, col);
}

const char *vader_db_col_text(void *rowsh, int col) {
    VRows *r = rowsh;
    if (!r) return vader_strdup("");
    if (r->kind == K_PG) return vader_pg_text(r->h, col);
    if (r->kind == K_MYSQL) return vader_my_text(r->h, col);
    const unsigned char *t = sqlite3_column_text((sqlite3_stmt *)r->h, col);
    return vader_strdup(t ? (const char *)t : "");
}

/* exec that aborts (exit 1) if the SQL fails — useful for migrations and scripts. */
void vader_db_must(void *handle, const char *sql) {
    const char *err = vader_db_exec(handle, sql);
    if (err) {
        fprintf(stderr, "SQL error: %s\n", err);
        exit(1);
    }
}

/* ---- parameterized queries: `?` placeholders + bind, safe client-side substitution ---- */
typedef struct {
    void *db;
    char *sql;
    char **vals;
    int nvals, cap;
} VStmt;

void *vader_db_prepare(void *dbh, const char *sql) {
    VStmt *st = vader_alloc(sizeof(VStmt));
    st->db = dbh;
    st->sql = vader_strdup(sql);
    st->vals = 0;
    st->nvals = 0;
    st->cap = 0;
    return st;
}

static void stmt_push(VStmt *st, char *v) {
    if (st->nvals >= st->cap) {
        int oc = st->cap;
        st->cap = oc ? oc * 2 : 4;
        st->vals = vader_realloc(st->vals, oc * sizeof(char *), st->cap * sizeof(char *));
    }
    st->vals[st->nvals++] = v;
}

/* a string param is SQL-escaped (single quotes doubled) and wrapped in quotes. */
void vader_db_bind_str(void *sth, const char *v) {
    int len = (int)strlen(v);
    char *buf = vader_alloc(len * 2 + 3);
    int o = 0;
    buf[o++] = '\'';
    for (int i = 0; i < len; i++) {
        if (v[i] == '\'') buf[o++] = '\'';
        buf[o++] = v[i];
    }
    buf[o++] = '\'';
    buf[o] = 0;
    stmt_push(sth, buf);
}
void vader_db_bind_int(void *sth, long long v) {
    char *buf = vader_alloc(32);
    snprintf(buf, 32, "%lld", v);
    stmt_push(sth, buf);
}
void vader_db_bind_float(void *sth, double v) {
    char *buf = vader_alloc(32);
    snprintf(buf, 32, "%.17g", v);
    stmt_push(sth, buf);
}

/* builds the final SQL by replacing each `?` with the next bound value. */
static char *stmt_build(VStmt *st) {
    int total = (int)strlen(st->sql) + 1;
    for (int i = 0; i < st->nvals; i++) total += (int)strlen(st->vals[i]);
    char *out = vader_alloc(total + 1);
    int o = 0, vi = 0;
    for (int i = 0; st->sql[i]; i++) {
        if (st->sql[i] == '?' && vi < st->nvals) {
            const char *v = st->vals[vi++];
            while (*v) out[o++] = *v++;
        } else {
            out[o++] = st->sql[i];
        }
    }
    out[o] = 0;
    return out;
}

const char *vader_db_run(void *sth) {
    return vader_db_exec(((VStmt *)sth)->db, stmt_build(sth));
}
void *vader_db_query_stmt(void *sth) {
    return vader_db_query(((VStmt *)sth)->db, stmt_build(sth));
}

void vader_db_close(void *handle) {
    VDB *d = handle;
    if (!d) return;
    if (d->kind == K_PG) {
        vader_pg_close(d->h);
    } else if (d->kind == K_MYSQL) {
        vader_my_close(d->h);
    } else {
        sqlite3_close((sqlite3 *)d->h);
    }
}
