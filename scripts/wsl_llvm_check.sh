#!/usr/bin/env bash
# Verifica se o LLVM (com llvm-config + headers) está disponível no WSL.
set -u
echo "=== llvm-config no PATH? ==="
for c in llvm-config llvm-config-18 llvm-config-17 llvm-config-16 llvm-config-15 llvm-config-14; do
  if command -v "$c" >/dev/null 2>&1; then
    echo "achei: $c -> $($c --version)"
    echo "  prefix: $($c --prefix)"
    echo "  libdir: $($c --libdir)"
  fi
done
echo "=== clang? ==="
command -v clang >/dev/null 2>&1 && clang --version | head -1 || echo "sem clang"
echo "=== pacotes llvm instalados (dpkg) ==="
dpkg -l 2>/dev/null | grep -iE 'llvm|libllvm' | awk '{print $2, $3}' | head -20 || echo "sem dpkg/llvm"
echo "=== headers? ==="
ls /usr/include/llvm-c/Core.h 2>/dev/null && echo "headers C presentes" || echo "sem headers llvm-c"
echo "=== versão do Windows (caso tenha instalado lá) ==="
ls "/mnt/c/Program Files/LLVM/bin/llvm-config.exe" 2>/dev/null && echo "(LLVM no Windows, NÃO serve pro build no WSL)" || echo "(nada de LLVM no Windows em Program Files)"
