#!/usr/bin/env bash
# One-line global installer for the mctx notepad.
#
#   curl -sSL https://raw.githubusercontent.com/cyberhatc/mctx/main/scripts/install.sh | bash
#
# Downloads the latest prebuilt binaries for your platform from the GitHub
# release, installs them into ~/.local/bin, registers the .mctx MIME type,
# desktop entry and icon (so .mctx files open in the app from the file
# manager), and prints next steps. On Debian/Ubuntu it also offers the .deb;
# on Android you can use Termux.
set -euo pipefail

REPO="cyberhatc/mctx"
VER="v2.1.5"
API="https://api.github.com/repos/${REPO}/releases/tags/${VER}"

info()  { printf '\033[1;36m[mctx]\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m[mctx]\033[0m %s\n' "$*"; }
die()   { printf '\033[1;31m[mctx]\033[0m %s\n' "$*" >&2; exit 1; }

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)   ASSET="mctx-linux-x86_64";       GUI_ASSET="mctx-gui-linux-x86_64";       ;;
  Linux-aarch64)  die "no aarch64 linux build yet — build with: cargo build --release" ;;
  Darwin-x86_64)  ASSET="mctx-macos-x86_64";       GUI_ASSET="mctx-gui-macos-x86_64";       ;;
  Darwin-arm64)   ASSET="mctx-macos-arm64";        GUI_ASSET="mctx-gui-macos-arm64";        ;;
  FreeBSD-amd64)  ASSET="mctx-freebsd-amd64";      GUI_ASSET="" ;;
  *) die "unsupported platform: $(uname -s) $(uname -m)" ;;
esac

INSTALL_DIR="${MCTX_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$INSTALL_DIR"

# --- download & install the CLI -------------------------------------------
DEST="$INSTALL_DIR/mctx"
info "downloading $ASSET from $REPO $VER"
URL=$(curl -fsSL "$API" | sed -n "s/.*\"browser_download_url\": \"\([^\"]*$ASSET[^\"]*\)\".*/\1/p" | head -1)
[ -n "${URL:-}" ] || die "could not find download URL for $ASSET"

TMP="$(mktemp)"
curl -fsSL "$URL" -o "$TMP"
chmod +x "$TMP"
mv -f "$TMP" "$DEST"
info "installed mctx to $DEST"

# --- download & install the GUI (Linux/macOS) ------------------------------
GUI_DEST="$INSTALL_DIR/mctx-gui"
if [ -n "${GUI_ASSET:-}" ]; then
  info "downloading $GUI_ASSET"
  GUI_URL=$(curl -fsSL "$API" | sed -n "s/.*\"browser_download_url\": \"\([^\"]*$GUI_ASSET[^\"]*\)\".*/\1/p" | head -1)
  if [ -n "${GUI_URL:-}" ]; then
    curl -fsSL "$GUI_URL" -o "$TMP"
    chmod +x "$TMP"
    mv -f "$TMP" "$GUI_DEST"
    info "installed mctx-gui to $GUI_DEST"
  else
    warn "could not find $GUI_ASSET in the release — skipping the GUI"
    GUI_DEST=""
  fi
else
  GUI_DEST=""
fi

# --- desktop integration (MIME + app entry + icon) -------------------------
register_desktop() {
  local bin="$1"
  local data="${XDG_DATA_HOME:-$HOME/.local/share}"
  mkdir -p "$data/mime/packages" "$data/applications" \
           "$data/icons/hicolor/scalable/apps"

  cat > "$data/mime/packages/mctx.xml" <<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-mctx">
    <comment>Memory Context file</comment>
    <glob pattern="*.mctx"/>
    <glob pattern="*.mctx.txt"/>
  </mime-type>
</mime-info>
XML

  cat > "$data/applications/mctx.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=mctx
Comment=Memory Context (.mctx) notepad — human + AI views
Exec=$bin %F
Icon=mctx
Terminal=false
StartupNotify=true
Categories=Utility;TextEditor;
Keywords=memory;context;agent;note;
MimeType=application/x-mctx;text/plain;
EOF

  cat > "$data/icons/hicolor/scalable/apps/mctx.svg" <<'SVG'
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <rect x="8" y="8" width="112" height="112" rx="16" fill="#1f6feb"/>
  <path d="M24 32h80M24 64h80M24 96h48" stroke="#ffffff" stroke-width="10" stroke-linecap="round"/>
</svg>
SVG

  if [ -x /usr/bin/update-mime-database ] || command -v update-mime-database >/dev/null 2>&1; then
    update-mime-database "$data/mime" >/dev/null 2>&1 || true
  fi
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$data/applications" >/dev/null 2>&1 || true
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f "$data/icons/hicolor" >/dev/null 2>&1 || true
  fi
  if command -v xdg-mime >/dev/null 2>&1; then
    xdg-mime default mctx.desktop application/x-mctx >/dev/null 2>&1 || true
  fi
  info "registered .mctx files — they now open with mctx from the file manager"
}

if [ -n "${GUI_DEST:-}" ]; then
  register_desktop "$GUI_DEST"
fi

# --- PATH check ------------------------------------------------------------
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
      DEB="mctx_2.1.5_amd64.deb"
      DEBURL=$(curl -fsSL "$API" | sed -n "s/.*\"browser_download_url\": \"\([^\"]*$DEB[^\"]*\)\".*/\1/p" | head -1)
      if [ -n "${DEBURL:-}" ]; then
        curl -fsSL "$DEBURL" -o "/tmp/$DEB"
        sudo apt-get install -y "/tmp/$DEB" || warn "apt install failed — try: sudo dpkg -i /tmp/$DEB"
      fi
      ;;
  esac
fi
