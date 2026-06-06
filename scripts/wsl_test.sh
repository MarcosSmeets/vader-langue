#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader
cargo test --quiet 2>&1 | grep -E "test result|error\[|panicked|sqlite" | head
