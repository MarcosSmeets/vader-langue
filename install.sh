#!/usr/bin/env bash
# Instala o compilador `vader` a partir do código-fonte.
#   curl/clone o repo, depois:  ./install.sh
# Variáveis:
#   VADER_BINDIR  diretório de instalação (padrão: ~/.local/bin)
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
  echo "erro: 'cargo' (Rust) não encontrado. Instale em https://rustup.rs" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

echo ">> compilando release (cargo build --release)..."
cargo build --release

BINDIR="${VADER_BINDIR:-$HOME/.local/bin}"
mkdir -p "$BINDIR"
install -m 0755 target/release/vader "$BINDIR/vader"

echo ">> instalado: $BINDIR/vader  ($("$BINDIR/vader" version))"

case ":$PATH:" in
  *":$BINDIR:"*) ;;
  *) echo ">> dica: adicione ao PATH:  export PATH=\"$BINDIR:\$PATH\"" ;;
esac

if ! command -v clang >/dev/null 2>&1; then
  echo ">> nota: 'clang' não encontrado — necessário só pro backend nativo ('vader llvm')."
fi
echo ">> pronto. Tente:  vader new api meu-projeto"
