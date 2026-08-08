#!/usr/bin/env bash
# Build a Debian/Ubuntu .deb package from the release binary.
# Usage: scripts/build-deb.sh [arch]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ARCH="${1:-amd64}"
BIN="$ROOT/target/release/mctx"
VERSION="1.1.0"

if [ ! -f "$BIN" ]; then
  echo "release binary not found at $BIN — run 'cargo build --release' first" >&2
  exit 1
fi

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$STAGE/DEBIAN" "$STAGE/usr/bin" \
         "$STAGE/usr/share/man/man1" \
         "$STAGE/usr/share/doc/mctx"

install -m 0755 "$BIN" "$STAGE/usr/bin/mctx"
gzip -9 -c "$ROOT/man/mctx.1" > "$STAGE/usr/share/man/man1/mctx.1.gz"

cat > "$STAGE/usr/share/doc/mctx/copyright" <<'EOF'
mctx — Memory Context (.mctx) notepad and format library
Copyright 2026, cyberhatc
Licensed under the MIT License or Apache-2.0, at your option.
EOF

cat > "$STAGE/DEBIAN/control" <<EOF
Package: mctx
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: cyberhatc <cyberhatc@users.noreply.github.com>
Homepage: https://github.com/cyberhatc/mctx
Description: Terminal notepad for .mctx AI agent memory files
 A lightweight two-panel terminal editor for the .mctx memory context
 format: section list + body editor, version-bumped saves, and an
 index rebuilt on every write. Also ships the zero-dependency mctx
 library for reading/writing .mctx files programmatically.
EOF

OUT="$ROOT/target/mctx_${VERSION}_${ARCH}.deb"
dpkg-deb --build --root-owner-group "$STAGE" "$OUT" >/dev/null
echo "built: $OUT"
