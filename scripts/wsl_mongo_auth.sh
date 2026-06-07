#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN=./target/debug/vader
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1
echo "=== standalone compile (scram + mongo) ==="
clang -c -O2 -Wall runtime/vader_scram.c -o /tmp/vs.o 2>/tmp/vs.log && echo "scram OK" || { echo FAIL; grep -E 'error|warning' /tmp/vs.log | head; }
clang -c -O2 -Wall runtime/vader_mongo.c -o /tmp/vm.o 2>/tmp/vm.log && echo "mongo OK" || { echo FAIL; grep -E 'error|warning' /tmp/vm.log | head; }

cat > /tmp/mauth.vd <<'EOF'
import "std/mongo"
import "std/json"
public fn main() {
    Mongo m = mongo.connect("mongodb://admin:secret@127.0.0.1:27017/vaderdb")
    Json d = json.object()
    json.set_str(d, "name", "Authed")
    json.set_int(d, "age", 42)
    error e = mongo.insert(m, "users", d)
    if e != nil {
        print("insert error:", e)
    }
    Json all = mongo.find(m, "users", json.object())
    print("found", json.count(all), "documents")
    int i = 0
    for i < json.count(all) {
        Json u = json.elem(all, i)
        print(json.as_str(json.field(u, "name")), json.as_int(json.field(u, "age")))
        i = i + 1
    }
    mongo.close(m)
}
EOF
echo "=== authenticated insert + find ==="
"$BIN" llvm /tmp/mauth.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built' | tail -8

cat > /tmp/mnoauth.vd <<'EOF'
import "std/mongo"
import "std/json"
public fn main() {
    Mongo m = mongo.connect("mongodb://127.0.0.1:27017/vaderdb")
    Json d = json.object()
    json.set_str(d, "name", "NoAuth")
    error e = mongo.insert(m, "users", d)
    if e != nil { print("rejected as expected:", e) } else { print("ERROR: insert without auth succeeded") }
    mongo.close(m)
}
EOF
echo "=== negative: insert WITHOUT credentials must be rejected ==="
"$BIN" llvm /tmp/mnoauth.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built' | tail -3