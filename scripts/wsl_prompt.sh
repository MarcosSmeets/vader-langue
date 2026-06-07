#!/usr/bin/env bash
export PATH=$HOME/.cargo/bin:/usr/bin:$PATH
BIN=/mnt/c/Users/marco/Documents/workspace/side_projects/vader/target/debug/vader
cd /tmp && rm -rf demoP
echo "=== interactive prompt (piping choice 2 = postgres) ==="
echo "2" | "$BIN" new api demoP --arch tdd
echo "--- .env.example ---"; cat demoP/.env.example
echo "--- vader.toml [database] ---"; grep -A1 database demoP/vader.toml
