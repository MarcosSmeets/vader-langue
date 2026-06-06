#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
cargo build 2>&1 | grep -E '^error' | head -40
echo "--- done ---"
