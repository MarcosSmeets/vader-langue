/* Driver SQLite do runtime da Vader.
 *
 * Linkado pelo clang junto com o sqlite3.c (amalgamation, domínio público)
 * quando o programa importa `std/db`. Expõe uma API opaca (ponteiros como i8*).
 * Sem-GC: strings retornadas são strdup'adas e vazam, alinhado com o runtime. */
#include "sqlite3.h"
#include <stdlib.h>
#include <string.h>

/* Abre (ou cria) o banco no caminho dado. Retorna o handle, ou NULL em falha. */
void *vader_db_open(const char *path) {
    sqlite3 *db = 0;
    if (sqlite3_open(path, &db) != SQLITE_OK) {
        if (db) sqlite3_close(db);
        return 0;
    }
    return db;
}

/* Executa SQL sem resultados. Retorna NULL em sucesso, ou a mensagem de erro. */
const char *vader_db_exec(void *db, const char *sql) {
    char *err = 0;
    int rc = sqlite3_exec((sqlite3 *)db, sql, 0, 0, &err);
    if (rc != SQLITE_OK) {
        const char *msg = strdup(err ? err : "erro SQL desconhecido");
        if (err) sqlite3_free(err);
        return msg;
    }
    return 0;
}

/* Prepara uma consulta e devolve o cursor (statement). NULL em erro. */
void *vader_db_query(void *db, const char *sql) {
    sqlite3_stmt *stmt = 0;
    if (sqlite3_prepare_v2((sqlite3 *)db, sql, -1, &stmt, 0) != SQLITE_OK) {
        return 0;
    }
    return stmt;
}

/* Avança pro próximo registro. 1 se há linha; 0 se acabou (e finaliza o cursor). */
int vader_db_next(void *stmt) {
    int rc = sqlite3_step((sqlite3_stmt *)stmt);
    if (rc == SQLITE_ROW) return 1;
    sqlite3_finalize((sqlite3_stmt *)stmt);
    return 0;
}

long long vader_db_col_int(void *stmt, int col) {
    return sqlite3_column_int64((sqlite3_stmt *)stmt, col);
}

double vader_db_col_float(void *stmt, int col) {
    return sqlite3_column_double((sqlite3_stmt *)stmt, col);
}

/* Texto da coluna, copiado (o sqlite reusa o buffer no próximo step). */
const char *vader_db_col_text(void *stmt, int col) {
    const unsigned char *t = sqlite3_column_text((sqlite3_stmt *)stmt, col);
    return strdup(t ? (const char *)t : "");
}

void vader_db_close(void *db) {
    sqlite3_close((sqlite3 *)db);
}
