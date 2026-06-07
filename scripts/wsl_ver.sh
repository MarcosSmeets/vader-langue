#!/usr/bin/env bash
export PATH=$HOME/.cargo/bin:/usr/bin:$PATH
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader
cargo build 2>&1 | grep -E "^error" | head
./target/debug/vader version
