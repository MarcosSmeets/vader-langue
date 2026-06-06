#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader
echo "=== index.json publicado ==="
cat /tmp/reg/index.json 2>/dev/null; echo
echo "=== testes ==="
cargo test --quiet 2>&1 | grep -E "test result" | head -1
