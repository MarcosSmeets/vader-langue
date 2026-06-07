#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
cargo build 2>&1 | grep -E '^error' | head

echo "=== (a) caching_sha2 scramble crypto vs Python reference ==="
cat > /tmp/scrt.c <<'EOF'
#include <stdio.h>
#include <string.h>
extern void vader_scram_sha256(const unsigned char *, int, unsigned char[32]);
int main(void){
    unsigned char pw[]="secret", nonce[20], h1[32],h2[32],h3[32],cat[52],out[32];
    for(int i=0;i<20;i++) nonce[i]=i+1;
    vader_scram_sha256(pw,6,h1);
    vader_scram_sha256(h1,32,h2);
    memcpy(cat,h2,32); memcpy(cat+32,nonce,20);
    vader_scram_sha256(cat,52,h3);
    for(int i=0;i<32;i++) out[i]=h1[i]^h3[i];
    for(int i=0;i<32;i++) printf("%02x",out[i]); printf("\n");
    return 0;
}
EOF
clang -O2 /tmp/scrt.c runtime/vader_scram.c -o /tmp/scrt && C_OUT=$(/tmp/scrt)
PY_OUT=$(python3 -c "
import hashlib
pw=b'secret'; nonce=bytes(range(1,21))
h1=hashlib.sha256(pw).digest(); h2=hashlib.sha256(h1).digest()
h3=hashlib.sha256(h2+nonce).digest()
print(bytes(a^b for a,b in zip(h1,h3)).hex())")
echo "  vader: $C_OUT"
echo "  python:$PY_OUT"
[ "$C_OUT" = "$PY_OUT" ] && echo "  MATCH ✓" || echo "  MISMATCH ✗"

echo "=== (b) auth-flow refactor against real MySQL 8 (native_password user) ==="
cat > /tmp/mynative.vd <<'EOF'
import "std/db"
public fn main() {
    DB c = db.open("mysql://vader:vpass@127.0.0.1:3306/vaderdb")
    db.exec(c, "DROP TABLE IF EXISTS users")
    db.exec(c, "CREATE TABLE users (id INT, name VARCHAR(64))")
    db.exec(c, "INSERT INTO users VALUES (1, 'Marco')")
    db.exec(c, "INSERT INTO users VALUES (2, 'Ada')")
    Rows r = db.query(c, "SELECT id, name FROM users ORDER BY id")
    for db.next(r) { print(db.col_int(r, 0), db.col_text(r, 1)) }
    db.close(c)
}
EOF
"$BIN" llvm /tmp/mynative.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built' | tail -5

echo "=== (c) caching_sha2 root WITHOUT --tls: graceful message expected ==="
cat > /tmp/mycache.vd <<'EOF'
import "std/db"
public fn main() {
    DB c = db.open("mysql://root:secret@127.0.0.1:3306/vaderdb")
    error e = db.exec(c, "SELECT 1")
    if e != nil { print("expected (needs tls):", e) }
    db.close(c)
}
EOF
"$BIN" llvm /tmp/mycache.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built' | tail -3