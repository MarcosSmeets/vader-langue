#!/usr/bin/env bash
# Checks whether LLVM (with llvm-config + headers) is available in WSL.
set -u
echo "=== llvm-config on PATH? ==="
for c in llvm-config llvm-config-18 llvm-config-17 llvm-config-16 llvm-config-15 llvm-config-14; do
  if command -v "$c" >/dev/null 2>&1; then
    echo "found: $c -> $($c --version)"
    echo "  prefix: $($c --prefix)"
    echo "  libdir: $($c --libdir)"
  fi
done
echo "=== clang? ==="
command -v clang >/dev/null 2>&1 && clang --version | head -1 || echo "no clang"
echo "=== installed llvm packages (dpkg) ==="
dpkg -l 2>/dev/null | grep -iE 'llvm|libllvm' | awk '{print $2, $3}' | head -20 || echo "no dpkg/llvm"
echo "=== headers? ==="
ls /usr/include/llvm-c/Core.h 2>/dev/null && echo "C headers present" || echo "no llvm-c headers"
echo "=== Windows version (in case it was installed there) ==="
ls "/mnt/c/Program Files/LLVM/bin/llvm-config.exe" 2>/dev/null && echo "(LLVM on Windows, does NOT work for the WSL build)" || echo "(no LLVM on Windows in Program Files)"
