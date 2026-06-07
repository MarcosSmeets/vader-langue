#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1

echo "=== MD5 crypto vs reference (md5('abc')) ==="
cat > /tmp/md5t.c <<'EOF'
#include <stdio.h>
#include <string.h>
/* pull in the pg runtime's md5 by including the source (static funcs) */
#define main pg_main_unused
#include "runtime/vader_pg.c"
#undef main
int main(void){
    unsigned char dg[16]; char hex[33];
    MD5_CTX m; md5_init(&m); md5_update(&m,(const unsigned char*)"abc",3); md5_final(&m,dg);
    md5_hex(dg,hex); printf("%s\n",hex);
    return 0;
}
EOF
clang -O2 -w -I. /tmp/md5t.c -o /tmp/md5t 2>/tmp/md5t.log && echo "  vader: $(/tmp/md5t)" || { echo "  (compile via include failed; skipping vector)"; grep error /tmp/md5t.log|head -3; }
echo "  expect 900150983cd24fb0d6963f7d28e17f72"

echo "=== live MD5 auth against Postgres ==="
cat > /tmp/pgmd5.vd <<'EOF'
import "std/db"
public fn main() {
    DB c = db.open("postgres://postgres:secret@127.0.0.1:5432/vaderdb")
    db.exec(c, "DROP TABLE IF EXISTS users")
    db.exec(c, "CREATE TABLE users (id INT, name TEXT)")
    db.exec(c, "INSERT INTO users VALUES (1, 'Marco')")
    db.exec(c, "INSERT INTO users VALUES (2, 'Ada')")
    Rows r = db.query(c, "SELECT id, name FROM users ORDER BY id")
    for db.next(r) { print(db.col_int(r, 0), db.col_text(r, 1)) }
    db.close(c)
}
EOF
"$BIN" llvm /tmp/pgmd5.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built' | tail -6