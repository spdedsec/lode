#!/usr/bin/env sh
set -eu
rm -f "${LODE_INSTALL_DIR:-$HOME/.local/bin}/lode"
rm -rf "$HOME/.lode"
echo "LODE removed"
