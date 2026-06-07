#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
BIN="$PWD/target/debug/vader"
cargo build 2>&1 | grep -E '^error' | head
for vd in tests/conformance/*.vd; do
  base="${vd%.vd}"
  out=$("$BIN" llvm "$vd" 2>&1)
  prog=$(echo "$out" | sed -n '/^--- running ---$/,$p' | tail -n +2)
  printf '%s\n' "$prog" > "$base.expected"
  echo "===== $(basename "$base") ====="
  printf '%s\n' "$prog"
done