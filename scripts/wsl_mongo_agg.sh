#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1
cat > /tmp/magg.vd <<'EOF'
import "std/mongo"
import "std/json"

fn addEmp(m Mongo, dept string, salary int) {
    Json e = json.object()
    json.set_str(e, "dept", dept)
    json.set_int(e, "salary", salary)
    mongo.insert(m, "emp", e)
}

public fn main() {
    Mongo m = mongo.connect("mongodb://admin:secret@127.0.0.1:27017/vaderdb")
    mongo.delete(m, "emp", json.object())
    addEmp(m, "Eng", 100)
    addEmp(m, "Eng", 120)
    addEmp(m, "Sales", 80)

    // pipeline: [{ $group: { _id: "$dept", total: { $sum: "$salary" } } }]
    Json pipeline = json.array()
    Json stage = json.object()
    Json group = json.object()
    json.set_str(group, "_id", "$dept")
    Json sum = json.object()
    json.set_str(sum, "$sum", "$salary")
    json.set(group, "total", sum)
    json.set(stage, "$group", group)
    json.add(pipeline, stage)

    Json res = mongo.aggregate(m, "emp", pipeline)
    print("groups:", json.count(res))
    int i = 0
    for i < json.count(res) {
        Json g = json.elem(res, i)
        print(json.as_str(json.field(g, "_id")), json.as_int(json.field(g, "total")))
        i = i + 1
    }
    mongo.close(m)
}
EOF
echo "=== mongo aggregation ($group + $sum; expect Eng=220, Sales=80) ==="
"$BIN" llvm /tmp/magg.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built' | tail -6
echo "=== openssl runtime libs (for MySQL caching_sha2 full-auth) ==="
ls /usr/lib/x86_64-linux-gnu/libcrypto.so* /usr/lib/x86_64-linux-gnu/libssl.so* 2>/dev/null || echo "none in default path"
find / -name 'libcrypto.so*' 2>/dev/null | head -3