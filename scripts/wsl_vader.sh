#!/usr/bin/env bash
# Runs the Vader CLI (via cargo) with cargo and go on PATH, inside WSL.
# Usage: wsl_vader.sh <subcommand> <args...>   (e.g.: run examples/hello.vd)
set -u
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
PROJECT="/mnt/c/Users/marco/Documents/workspace/side_projects/vader"
cd "$PROJECT" || exit 1
cargo run --quiet -- "$@"
