#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
cargo test --test conformance 2>&1 | grep -E 'running [0-9]|test result|conformance|FAILED|regress' | head -20
echo "=== full suite ==="
cargo test --quiet 2>&1 | grep -E 'test result' | head -5