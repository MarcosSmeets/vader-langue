#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
echo "=== install.sh (build release + installs to /tmp/vbin) ==="
VADER_BINDIR=/tmp/vbin bash install.sh 2>&1 | tail -8
echo "=== vader version (installed binary) ==="
/tmp/vbin/vader version
echo "=== vader llvm smoke (installed binary, without the repo on cargo's PATH) ==="
/tmp/vbin/vader llvm examples/maps.vd 2>&1 | grep -vE 'emitted|compiled|linkando' | tail -5
echo "=== test suite ==="
cargo test --quiet 2>&1 | grep -E 'test result' | head -1