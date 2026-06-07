#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN=./target/debug/vader
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -20
cargo test --quiet 2>&1 | grep 'test result' | head -1
echo "=== compile vader_mongo.c standalone ==="
clang -c -O2 -Wall runtime/vader_mongo.c -o /tmp/vm.o 2>/tmp/vm.log && echo "mongo OK" || { echo FAILED; grep -E 'error|warning' /tmp/vm.log | head; }
echo "=== run examples/mongo_demo.vd (against Docker Mongo) ==="
"$BIN" llvm examples/mongo_demo.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built' | tail -10