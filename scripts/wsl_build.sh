#!/usr/bin/env bash
# Dev helper: roda dentro do WSL (Ubuntu), onde o Rust está instalado.
# O projeto vive no disco do Windows, acessível em /mnt/c/...
set -u

export PATH="$HOME/.cargo/bin:$PATH"
PROJECT="/mnt/c/Users/marco/Documents/workspace/side_projects/vader"

echo "=== toolchain ==="
echo "cargo:  $(cargo --version 2>&1)"
echo "rustc:  $(rustc --version 2>&1)"
if command -v cc  >/dev/null 2>&1; then echo "cc:     $(command -v cc)";  else echo "cc:     NONE"; fi
if command -v gcc >/dev/null 2>&1; then echo "gcc:    $(command -v gcc)"; else echo "gcc:    NONE"; fi
if sudo -n true 2>/dev/null; then echo "sudo:   PASSWORDLESS"; else echo "sudo:   NEEDS-PASSWORD"; fi

echo "=== cargo test ==="
cd "$PROJECT" || { echo "projeto não encontrado em $PROJECT"; exit 1; }
cargo test --color never 2>&1 | tail -n 40
