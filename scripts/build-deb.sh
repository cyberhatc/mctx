#!/usr/bin/env bash
# Build a Debian/Ubuntu .deb package from the release binaries.
# Usage: scripts/build-deb.sh [arch]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ARCH="${1:-amd64}"
BIN="$ROOT/target/release/mctx"
GUI="$ROOT/target/release/mctx-gui"
VERSION="2.1.4"

if [ ! -f "$BIN" ]; then
  echo "release binary not found at $BIN — run 'cargo build --release' first" >&2
  exit 1
fi
if [ ! -f "$GUI" ]; then
  echo "GUI binary not found at $GUI — run 'cargo build --release -p mctx-gui' first" >&2
  exit 1
fi

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$STAGE/DEBIAN" "$STAGE/usr/bin" \
         "$STAGE/usr/share/man/man1" \
         "$STAGE/usr/share/doc/mctx" \
         "$STAGE/usr/share/applications" \
         "$STAGE/usr/share/mime/packages" \
         "$STAGE/usr/share/icons/hicolor/scalable/apps"

install -m 0755 "$BIN" "$STAGE/usr/bin/mctx"
install -m 0755 "$GUI" "$STAGE/usr/bin/mctx-gui"
gzip -9 -c "$ROOT/man/mctx.1" > "$STAGE/usr/share/man/man1/mctx.1.gz"

cat > "$STAGE/usr/share/doc/mctx/copyright" <<'EOF'
mctx — Memory Context (.mctx) notepad and format library
Copyright 2026, cyberhatc
Licensed under the MIT License or Apache-2.0, at your option.
EOF

cat > "$STAGE/usr/share/applications/mctx.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=mctx
Comment=Memory Context (.mctx) notepad — human + AI views
Exec=mctx-gui %F
Icon=mctx
Terminal=false
Categories=Utility;TextEditor;
Keywords=memory;context;agent;note;
MimeType=application/x-mctx;text/plain;
EOF

cat > "$STAGE/usr/share/mime/packages/mctx.xml" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-mctx">
    <comment>Memory Context file</comment>
    <glob pattern="*.mctx"/>
    <glob pattern="*.mctx.txt"/>
  </mime-type>
</mime-info>
EOF

cat > "$STAGE/usr/share/icons/hicolor/scalable/apps/mctx.svg" <<'EOF'
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <rect x="8" y="8" width="112" height="112" rx="16" fill="#1f6feb"/>
  <path d="M24 32h80M24 64h80M24 96h48" stroke="#ffffff" stroke-width="10" stroke-linecap="round"/>
</svg>
EOF

cat > "$STAGE/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if [ -x /usr/bin/update-mime-database ]; then
  update-mime-database /usr/share/mime >/dev/null 2>&1 || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
fi
if [ -x /usr/bin/gtk-update-icon-cache ]; then
  gtk-update-icon-cache /usr/share/icons/hicolor >/dev/null 2>&1 || true
fi
exit 0
EOF
chmod 0755 "$STAGE/DEBIAN/postinst"

cat > "$STAGE/DEBIAN/control" <<EOF
Package: mctx
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: cyberhatc <cyberhatc@users.noreply.github.com>
Homepage: https://github.com/cyberhatc/mctx
Description: Notepad for .mctx AI agent memory files
  .mctx is a token-optimized, seek-indexed memory format readable by both
  humans and AI agents. Ships a GUI notepad (mctx-gui, with a human Markdown
  view and an AI raw/JSON view), a two-panel terminal editor (mctx), and the
  zero-dependency mctx library. Registers .mctx files with the desktop so they
  open in the mctx app like any notepad.
EOF

OUT="$ROOT/target/mctx_${VERSION}_${ARCH}.deb"
# -Zgzip: gzip control/data archives, understood by every apt/dpkg version.
# zstd (dpkg's newer default) makes older Ubuntu/Debian systems fail with
# "Error: Unsupported file ..." at install time.
dpkg-deb --build --root-owner-group -Zgzip "$STAGE" "$OUT" >/dev/null
echo "built: $OUT"
