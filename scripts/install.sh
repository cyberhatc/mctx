#!/usr/bin/env bash
# One-line global installer for the mctx notepad.
#
#   curl -sSL https://raw.githubusercontent.com/cyberhatc/mctx/main/scripts/install.sh | bash
#
# Downloads the latest prebuilt binary for your platform from the GitHub
# release, installs it into ~/.local/bin, and prints next steps. On
# Debian/Ubuntu it also offers the .deb; on Android you can use Termux.
set -euo pipefail

REPO="cyberhatc/mctx"
VER="v2.1.4"
API="https://api.github.com/repos/${REPO}/releases/tags/${VER}"

info()  { printf '\033[1;36m[mctx]\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m[mctx]\033[0m %s\n' "$*"; }
die()   { printf '\033[1;31m[mctx]\033[0m %s\n' "$*" >&2; exit 1; }

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)   ASSET="mctx-linux-x86_64";  ;;
  Linux-aarch64)  die "no aarch64 linux build yet — build with: cargo build --release" ;;
  Darwin-x86_64)  ASSET="mctx-macos-x86_64";  ;;
  Darwin-arm64)   ASSET="mctx-macos-arm64";   ;;
  FreeBSD-amd64)  ASSET="mctx-freebsd-amd64"; ;;
  *) die "unsupported platform: $(uname -s) $(uname -m)" ;;
esac

INSTALL_DIR="${MCTX_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$INSTALL_DIR"
DEST="$INSTALL_DIR/mctx"

info "downloading $ASSET from $REPO $VER"
URL=$(curl -fsSL "$API" | sed -n "s/.*\"browser_download_url\": \"\([^\"]*$ASSET[^\"]*\)\".*/\1/p" | head -1)
[ -n "${URL:-}" ] || die "could not find download URL for $ASSET"

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT
curl -fsSL "$URL" -o "$TMP"
chmod +x "$TMP"
mv -f "$TMP" "$DEST"

info "installed to $DEST"
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
  warn "$INSTALL_DIR is not on your PATH. Add it, e.g. in ~/.bashrc:"
  warn "    echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.bashrc"
fi

info "usage: mctx [memory.mctx]"
mctx="$(command -v mctx || true)"
[ -n "$mctx" ] && info "mctx is already on PATH at $mctx — run 'mctx memory.mctx'"

if command -v apt-get >/dev/null 2>&1 && [ -t 0 ]; then
  printf '\n[mctx] Install the Debian package too? [y/N] '
  read -r yes
  case "$yes" in
    y|Y|yes|YES)
      DEB="mctx_2.1.4_amd64.deb"
      DEBURL=$(curl -fsSL "$API" | sed -n "s/.*\"browser_download_url\": \"\([^\"]*$DEB[^\"]*\)\".*/\1/p" | head -1)
      if [ -n "${DEBURL:-}" ]; then
        curl -fsSL "$DEBURL" -o "/tmp/$DEB"
        sudo apt-get install -y "/tmp/$DEB" || warn "apt install failed — try: sudo dpkg -i /tmp/$DEB"
      fi
      ;;
  esac
fi
