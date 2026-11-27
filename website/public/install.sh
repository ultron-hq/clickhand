#!/bin/sh
# Clickhand installer for macOS and Linux.
#
#   curl -fsSL https://clickhand.vercel.app/install.sh | sh
#
# Downloads the prebuilt binary for this computer from GitHub Releases, checks its
# SHA-256, puts it in ~/.local/bin and starts the guided setup, which connects your
# AI agents (Claude Code, Codex, Cursor, VS Code, ...). Source:
# https://github.com/ultron-hq/ultron
#
# Options (environment variables):
#   CLICKHAND_INSTALL_DIR=/path   where to put the binary (default ~/.local/bin)
#   CLICKHAND_VERSION=v0.1.0      a specific release (default: the latest)
#   CLICKHAND_NO_SETUP=1          install only, skip the setup
#   CLICKHAND_NO_MODIFY_PATH=1    don't add the install folder to your shell's PATH
set -eu

REPO="ultron-hq/ultron"
BIN_DIR="${CLICKHAND_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${CLICKHAND_VERSION:-latest}"

say() { printf '%s\n' "$*"; }
die() { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }

fetch() {
  if command -v curl >/dev/null 2>&1; then curl -fsSL "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then wget -qO "$2" "$1"
  else die "curl or wget is needed"; fi
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

os=$(uname -s)
arch=$(uname -m)
case "$os" in
  Linux) os=unknown-linux-musl ;;
  Darwin) os=apple-darwin ;;
  MINGW* | MSYS* | CYGWIN*) die "on Windows, run this in PowerShell instead: irm https://clickhand.vercel.app/install.ps1 | iex" ;;
  *) die "this OS ($os) isn't supported yet" ;;
esac
case "$arch" in
  x86_64 | amd64) arch=x86_64 ;;
  arm64 | aarch64) arch=aarch64 ;;
  *) die "this CPU ($arch) isn't supported yet" ;;
esac
# An Apple Silicon Mac running an Intel shell under Rosetta still gets the native build.
if [ "$os" = apple-darwin ] && [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then
  arch=aarch64
fi
target="$arch-$os"
release_url() {
  if [ "$VERSION" = latest ]; then echo "https://github.com/$REPO/releases/latest/download/clickhand-$1.tar.gz"
  else echo "https://github.com/$REPO/releases/download/$VERSION/clickhand-$1.tar.gz"; fi
}
asset="clickhand-$target.tar.gz"
url=$(release_url "$target")

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

say "Installing Clickhand ($target)"
# Linux builds are static (musl) and run on any distro; releases before v0.1.2 only had glibc ones.
if ! fetch "$url" "$tmp/$asset" 2>/dev/null && [ "$os" = unknown-linux-musl ]; then
  target="$arch-unknown-linux-gnu"
  asset="clickhand-$target.tar.gz"
  url=$(release_url "$target")
fi
if [ -s "$tmp/$asset" ] || fetch "$url" "$tmp/$asset" 2>/dev/null; then
  fetch "$url.sha256" "$tmp/$asset.sha256" || die "couldn't download the checksum"
  expected=$(cut -c1-64 "$tmp/$asset.sha256")
  actual=$(sha256 "$tmp/$asset")
  [ "$expected" = "$actual" ] || die "checksum mismatch, the download may be damaged; try again"
  tar -xzf "$tmp/$asset" -C "$tmp"
  mkdir -p "$BIN_DIR"
  mv -f "$tmp/clickhand" "$BIN_DIR/clickhand"
  chmod +x "$BIN_DIR/clickhand"
  bin="$BIN_DIR/clickhand"
elif command -v cargo >/dev/null 2>&1; then
  say "No prebuilt binary for $target, so building it with cargo (takes a minute or two)..."
  cargo install --locked --git "https://github.com/$REPO" clickhand
  bin="${CARGO_HOME:-$HOME/.cargo}/bin/clickhand"
else
  die "no prebuilt binary for $target yet. Install Rust (https://rustup.rs) and run this again to build it."
fi

# Agents set up before the rename start `ultron-hands`; keep that name working.
[ -e "$(dirname "$bin")/ultron-hands" ] || ln -s "$bin" "$(dirname "$bin")/ultron-hands" 2>/dev/null || true

version=$("$bin" --version 2>/dev/null) || die "the downloaded clickhand doesn't run on this system ($target). Please open an issue: https://github.com/$REPO/issues"
say "Installed $version to $bin"

# Make `clickhand stop` and friends work in new terminals.
dir=$(dirname "$bin")
case ":$PATH:" in
  *":$dir:"*) ;;
  *)
    if [ -z "${CLICKHAND_NO_MODIFY_PATH:-}" ]; then
      case "${SHELL:-}" in
        */zsh) rc="$HOME/.zshrc"; line="export PATH=\"$dir:\$PATH\"" ;;
        */bash) rc="$HOME/.bashrc"; line="export PATH=\"$dir:\$PATH\"" ;;
        */fish) rc="$HOME/.config/fish/config.fish"; line="fish_add_path \"$dir\"" ;;
        *) rc="$HOME/.profile"; line="export PATH=\"$dir:\$PATH\"" ;;
      esac
      if ! grep -qs "# clickhand" "$rc"; then
        mkdir -p "$(dirname "$rc")"
        printf '\n%s  # clickhand\n' "$line" >>"$rc"
        say "Added $dir to your PATH in $rc (open a new terminal to use it)"
      fi
    fi
    ;;
esac

if [ -n "${CLICKHAND_NO_SETUP:-}" ]; then
  say "Run \`clickhand\` to connect your AI agents."
  exit 0
fi
say ""
# Piped from curl, stdin is this script; read answers from the terminal instead.
if (exec </dev/tty) 2>/dev/null; then
  "$bin" setup </dev/tty
else
  "$bin" setup --yes
fi
