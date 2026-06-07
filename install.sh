#!/bin/sh
# Vader installer.
#
#   curl -fsSL https://raw.githubusercontent.com/MarcosSmeets/vader-langue/main/install.sh | sh
#
# Downloads a prebuilt `vader` binary for your OS/arch from GitHub Releases and puts
# it on your PATH. Options (environment variables):
#   VADER_VERSION          release tag to install (default: latest), e.g. v0.6.0
#   VADER_BINDIR           install directory (default: $HOME/.local/bin)
#   VADER_NO_MODIFY_PATH=1 don't touch shell profiles, just print the PATH line
#
# Run `./install.sh --source` inside a checkout to build from source instead.
set -eu

REPO="MarcosSmeets/vader-langue"
BINDIR="${VADER_BINDIR:-$HOME/.local/bin}"
VERSION="${VADER_VERSION:-latest}"

say() { printf 'vader-install: %s\n' "$*"; }
err() { printf 'vader-install: error: %s\n' "$*" >&2; exit 1; }

TMP=""
cleanup() { [ -n "$TMP" ] && rm -f "$TMP" 2>/dev/null || true; }
trap cleanup EXIT

if [ "${1:-}" = "--source" ] || [ "${1:-}" = "-s" ] || [ "${VADER_FROM_SOURCE:-}" = "1" ]; then
  # --- build from source ---
  command -v cargo >/dev/null 2>&1 || err "'cargo' (Rust) not found — install it at https://rustup.rs"
  ROOT="$(cd "$(dirname "$0")" && pwd)"
  say "building from source (cargo build --release)…"
  ( cd "$ROOT" && cargo build --release )
  SRC="$ROOT/target/release/vader"
else
  # --- download a prebuilt binary ---
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Linux) plat="linux" ;;
    Darwin) plat="macos" ;;
    *) err "unsupported OS '$os'. On Windows use install.ps1 (PowerShell) or winget." ;;
  esac
  case "$arch" in
    x86_64 | amd64) cpu="x86_64" ;;
    arm64 | aarch64) cpu="arm64" ;;
    *) err "unsupported architecture '$arch'." ;;
  esac
  if [ "$plat" = "linux" ] && [ "$cpu" = "arm64" ]; then
    err "no prebuilt Linux arm64 binary yet — run with --source inside a checkout, or build with cargo."
  fi

  asset="vader-${plat}-${cpu}"
  if [ "$VERSION" = "latest" ]; then
    url="https://github.com/$REPO/releases/latest/download/$asset"
  else
    url="https://github.com/$REPO/releases/download/$VERSION/$asset"
  fi

  TMP="$(mktemp)"
  say "downloading $asset ($VERSION)…"
  if command -v curl >/dev/null 2>&1; then
    curl -fSL --proto '=https' --tlsv1.2 "$url" -o "$TMP" \
      || err "download failed: $url  (is there a published release with that asset?)"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$TMP" "$url" || err "download failed: $url"
  else
    err "need 'curl' or 'wget' to download."
  fi
  SRC="$TMP"
fi

# --- install ---
mkdir -p "$BINDIR"
if ! install -m 0755 "$SRC" "$BINDIR/vader" 2>/dev/null; then
  cp "$SRC" "$BINDIR/vader"
  chmod 0755 "$BINDIR/vader"
fi
say "installed: $BINDIR/vader ($("$BINDIR/vader" version 2>/dev/null || echo '?'))"

# --- PATH setup ---
case ":$PATH:" in
  *":$BINDIR:"*)
    : # already on PATH
    ;;
  *)
    line="export PATH=\"$BINDIR:\$PATH\""
    if [ "${VADER_NO_MODIFY_PATH:-}" = "1" ]; then
      say "add this to your shell profile:  $line"
    else
      case "$(basename "${SHELL:-sh}")" in
        zsh) prof="$HOME/.zshrc" ;;
        bash) [ "$(uname -s)" = "Darwin" ] && prof="$HOME/.bash_profile" || prof="$HOME/.bashrc" ;;
        fish) prof="$HOME/.config/fish/config.fish"; line="fish_add_path $BINDIR" ;;
        *) prof="$HOME/.profile" ;;
      esac
      mkdir -p "$(dirname "$prof")"
      touch "$prof"
      if grep -Fq "$BINDIR" "$prof" 2>/dev/null; then
        say "$BINDIR already referenced in $prof"
      else
        printf '\n# added by vader-install\n%s\n' "$line" >> "$prof"
        say "added $BINDIR to PATH in $prof"
      fi
      say "restart your shell, or run:  source $prof"
    fi
    ;;
esac

command -v clang >/dev/null 2>&1 || say "note: install 'clang' for the native backend (vader llvm)."
say "done. Try:  vader new api my-project"
