#!/usr/bin/env bash
# Micro-benchmarks: Vader vs C / C++ / Rust / Go. All compiled at -O2 / release.
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:/usr/bin:$PATH"
ROOT=/mnt/c/Users/marco/Documents/workspace/side_projects/vader
cd "$ROOT" || exit 1
VADER="$ROOT/target/debug/vader"
B="$ROOT/benchmarks"
OUT=/tmp/bench
mkdir -p "$OUT"

echo "building compilers/binaries..."
cargo build 2>&1 | grep -E '^error' | head
"$VADER" llvm --out "$OUT/fib_vader"    "$B/fib.vd"    >/dev/null 2>&1
"$VADER" llvm --out "$OUT/primes_vader" "$B/primes.vd" >/dev/null 2>&1
clang   -O2 "$B/fib.c"      -o "$OUT/fib_c"
clang   -O2 "$B/primes.c"   -o "$OUT/primes_c"
clang++ -O2 "$B/fib.cpp"    -o "$OUT/fib_cpp"
clang++ -O2 "$B/primes.cpp" -o "$OUT/primes_cpp"
rustc   -O  "$B/fib.rs"     -o "$OUT/fib_rust"    2>/dev/null
rustc   -O  "$B/primes.rs"  -o "$OUT/primes_rust" 2>/dev/null
go build    -o "$OUT/fib_go"     "$B/fib.go"
go build    -o "$OUT/primes_go"  "$B/primes.go"

bench() {  # $1 = binary; prints best-of-3 seconds
  local best=99999999999
  for _ in 1 2 3; do
    local s=$(date +%s%N); "$1" >/dev/null 2>&1; local e=$(date +%s%N)
    local d=$((e - s)); [ "$d" -lt "$best" ] && best=$d
  done
  awk "BEGIN{printf \"%.3f\", $best/1000000000}"
}

for name in "fib(40)=fib" "primes<2M=primes"; do
  label="${name%%=*}"; key="${name##*=}"
  echo "=== $label  (best of 3, seconds) ==="
  for lang in vader c cpp rust go; do
    bin="$OUT/${key}_${lang}"
    [ -x "$bin" ] || { printf "  %-6s (build missing)\n" "$lang"; continue; }
    printf "  %-6s %s s   out=%s\n" "$lang" "$(bench "$bin")" "$("$bin")"
  done
done
