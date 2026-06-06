#!/usr/bin/env bash
# Roda a CLI do Vader (via cargo) com cargo e go no PATH, dentro do WSL.
# Uso: wsl_vader.sh <subcomando> <args...>   (ex.: run examples/hello.vd)
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
PROJECT="/mnt/c/Users/marco/Documents/workspace/side_projects/vader"
cd "$PROJECT" || exit 1
cargo run --quiet -- "$@"
