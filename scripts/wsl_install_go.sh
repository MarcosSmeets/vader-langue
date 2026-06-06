#!/usr/bin/env bash
# Instala o Go no espaço do usuário (sem sudo), em ~/.local/go.
set -eu

VER=$(curl -fsSL "https://go.dev/VERSION?m=text" | head -n1)
echo "latest go: $VER"
URL="https://go.dev/dl/${VER}.linux-amd64.tar.gz"
echo "downloading $URL"
curl -fsSL "$URL" -o /tmp/go.tar.gz

rm -rf "$HOME/.local/go"
mkdir -p "$HOME/.local"
tar -C "$HOME/.local" -xzf /tmp/go.tar.gz   # cria ~/.local/go
rm -f /tmp/go.tar.gz

"$HOME/.local/go/bin/go" version
echo "installed at: $HOME/.local/go/bin/go"
