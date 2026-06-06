#!/usr/bin/env bash
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1

echo "=== vader run examples/basics.vd ==="
cargo run --quiet -- run examples/basics.vd

echo "=== vader build examples/hello.vd ==="
cargo run --quiet -- build examples/hello.vd

echo "=== executando o binario nativo ./hello ==="
./hello

echo "=== tipo do arquivo ==="
file hello
