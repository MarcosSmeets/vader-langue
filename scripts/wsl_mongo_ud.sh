#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1
cat > /tmp/mud.vd <<'EOF'
import "std/mongo"
import "std/json"
public fn main() {
    Mongo m = mongo.connect("mongodb://admin:secret@127.0.0.1:27017/vaderdb")
    mongo.delete(m, "people", json.object())   // start clean

    Json a = json.object()
    json.set_str(a, "name", "Marco")
    json.set_int(a, "age", 30)
    mongo.insert(m, "people", a)
    Json b = json.object()
    json.set_str(b, "name", "Ada")
    json.set_int(b, "age", 36)
    mongo.insert(m, "people", b)

    Json filter = json.object()
    json.set_str(filter, "name", "Marco")
    Json changes = json.object()
    json.set_int(changes, "age", 31)
    error e = mongo.update(m, "people", filter, changes)
    if e != nil { print("update error:", e) }

    Json df = json.object()
    json.set_str(df, "name", "Ada")
    error d = mongo.delete(m, "people", df)
    if d != nil { print("delete error:", d) }

    Json all = mongo.find(m, "people", json.object())
    print("remaining:", json.count(all))
    int i = 0
    for i < json.count(all) {
        Json u = json.elem(all, i)
        print(json.as_str(json.field(u, "name")), json.as_int(json.field(u, "age")))
        i = i + 1
    }
    mongo.close(m)
}
EOF
echo "=== mongo update + delete (expect: remaining 1 -> Marco 31) ==="
"$BIN" llvm /tmp/mud.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built' | tail -6