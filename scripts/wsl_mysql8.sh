#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1
echo "=== standalone compile (mysql: plain + TLS) ==="
clang -c -O2 -Wall runtime/vader_mysql.c -o /tmp/vmy.o 2>/tmp/vmy.log && echo "mysql (no tls) OK" || { echo FAIL; grep -E 'error' /tmp/vmy.log | head; }
clang -c -O2 -Wall -DVADER_TLS runtime/vader_mysql.c -o /tmp/vmyt.o 2>/tmp/vmyt.log && echo "mysql (tls) OK" || { echo FAIL; grep -E 'error|fatal' /tmp/vmyt.log | head; }

cat > /tmp/mytest.vd <<'EOF'
import "std/db"
public fn main() {
    DB c = db.open("mysql://root:secret@127.0.0.1:3306/vaderdb")
    db.exec(c, "DROP TABLE IF EXISTS users")
    db.exec(c, "CREATE TABLE users (id INT, name VARCHAR(64))")
    db.exec(c, "INSERT INTO users VALUES (1, 'Marco')")
    db.exec(c, "INSERT INTO users VALUES (2, 'Ada')")
    Rows r = db.query(c, "SELECT id, name FROM users ORDER BY id")
    for db.next(r) {
        print(db.col_int(r, 0), db.col_text(r, 1))
    }
    db.close(c)
}
EOF
echo "=== MySQL 8 caching_sha2 (build with --tls for the RSA full auth) ==="
"$BIN" llvm --tls /tmp/mytest.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built' | tail -8