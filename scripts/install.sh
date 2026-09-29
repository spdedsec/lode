#!/usr/bin/env sh
set -eu

VERSION="${LODE_VERSION:-latest}"
ARCH="$(uname -m)"
OS="$(uname -s)"
case "$OS/$ARCH" in
  Linux/x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
  Linux/aarch64|Linux/arm64) TARGET="aarch64-unknown-linux-gnu" ;;
  *) echo "Unsupported platform: $OS/$ARCH" >&2; exit 1 ;;
esac

REPO="${LODE_REPO:-spdedsec/lode}"
BASE="https://github.com/$REPO/releases"
if [ "$VERSION" = "latest" ]; then BASE="$BASE/latest/download"; else BASE="$BASE/download/v$VERSION"; fi
ASSET="lode-$TARGET.tar.gz"
SUMS="$(mktemp)"; ARCHIVE="$(mktemp)"
trap 'rm -f "$SUMS" "$ARCHIVE"' EXIT
curl -fsSL "$BASE/SHA256SUMS.txt" -o "$SUMS"
curl -fsSL "$BASE/$ASSET" -o "$ARCHIVE"
EXPECTED="$(awk -v f="$ASSET" '$2==f {print $1}' "$SUMS")"
[ -n "$EXPECTED" ] || { echo "No checksum for $ASSET" >&2; exit 1; }
ACTUAL="$(sha256sum "$ARCHIVE" | awk '{print $1}')"
[ "$EXPECTED" = "$ACTUAL" ] || { echo "Checksum verification failed" >&2; exit 1; }

PREFIX="${LODE_INSTALL_DIR:-$HOME/.local/bin}"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP" "$SUMS" "$ARCHIVE"' EXIT
tar -xzf "$ARCHIVE" -C "$TMP"
mkdir -p "$PREFIX"
install -m 0755 "$TMP/lode-$TARGET/lode" "$PREFIX/lode"
echo "LODE installed to $PREFIX/lode"
