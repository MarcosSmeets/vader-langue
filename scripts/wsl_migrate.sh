#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
ROOT=/mnt/c/Users/marco/Documents/workspace/side_projects/vader
cd "$ROOT" || exit 1
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -30
cargo test --quiet 2>&1 | grep -E 'test result' | head -1
BIN="$ROOT/target/debug/vader"

echo "=== sets up project with migrations ==="
rm -rf /tmp/mproj /tmp/m.db
mkdir -p /tmp/mproj/migrations
cd /tmp/mproj
cat > vader.toml <<'EOF'
name = "mproj"

[database]
url = "/tmp/m.db"
EOF
cat > migrations/0001_users.up.sql <<'EOF'
create table users (id integer, name text);
insert into users values (1, 'Marco');
insert into users values (2, 'Vader');
EOF
cat > migrations/0001_users.down.sql <<'EOF'
drop table users;
EOF

echo "=== status (before) ==="; "$BIN" migrate status
echo "=== migrate up ==="; "$BIN" migrate up
echo "=== status (after) ==="; "$BIN" migrate status
echo "=== confirms the data written to /tmp/m.db ==="
cat > /tmp/check.vd <<'EOF'
import "std/db"
public fn main() {
    DB c = db.open("/tmp/m.db")
    Rows r = db.query(c, "select id, name from users order by id")
    for db.next(r) {
        int id = db.col_int(r, 0)
        string name = db.col_text(r, 1)
        print(id, name)
    }
    db.close(c)
}
EOF
"$BIN" llvm /tmp/check.vd 2>&1 | grep -vE 'emitted|compiled|linkando|compilando' | tail -5
echo "=== migrate down (reverts) ==="; "$BIN" migrate down
echo "=== status (after down) ==="; "$BIN" migrate status