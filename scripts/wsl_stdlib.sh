#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
echo "=== build + test ==="
cargo build 2>&1 | grep -E '^error' | head -30
cargo test --quiet 2>&1 | grep 'test result' | head -1
echo "=== standalone compile of the new runtimes ==="
for f in vader_str vader_math vader_time vader_fs vader_fmt; do
  clang -c -O2 -Wall runtime/$f.c -o /tmp/$f.o 2>/tmp/$f.log && echo "  $f OK" || { echo "  $f FAILED"; grep -E 'error|warning' /tmp/$f.log | head -3; }
done
echo "=== run examples/stdlib_demo.vd ==="
"$BIN" llvm examples/stdlib_demo.vd 2>&1 | grep -vE 'emitted|compiling|linking|compiled|built|cached' | tail -30