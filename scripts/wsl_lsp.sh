#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
echo "=== cargo build ==="
cargo build 2>&1 | grep -E '^error' | head -40
echo "=== cargo test (json + lsp) ==="
cargo test --quiet 2>&1 | grep -E 'test result|FAILED' | head -5
BIN="$PWD/target/debug/vader"
echo "=== smoke test end-to-end (stdin -> vader lsp -> stdout) ==="
python3 - "$BIN" <<'PY'
import subprocess, sys, json
binp = sys.argv[1]
def msg(obj):
    s = json.dumps(obj)
    return f"Content-Length: {len(s.encode())}\r\n\r\n{s}".encode()
inp  = msg({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})
inp += msg({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///x.vd","text":"fn main() { nope() }"}}})
inp += msg({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///x.vd"},"contentChanges":[{"text":"fn main() { print(1) }"}]}})
inp += msg({"jsonrpc":"2.0","id":2,"method":"shutdown","params":{}})
inp += msg({"jsonrpc":"2.0","method":"exit","params":{}})
p = subprocess.run([binp,"lsp"], input=inp, capture_output=True, timeout=20)
out = p.stdout.decode(errors="replace")
print(out[:1000])
print("--- checks ---")
print("capabilities:", "textDocumentSync" in out)
print("publishDiagnostics:", "publishDiagnostics" in out)
print("reports error on bad code:", '"severity":1' in out)
print("clears diagnostics on fix:", '"diagnostics":[]' in out)
PY
