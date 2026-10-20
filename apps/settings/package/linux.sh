#!/usr/bin/env bash
# Keep the libxdo ABI the app was built against private to the package.
# The packaging host supplies the license from libxdo-dev.
set -euo pipefail

binary="$1"
out="$2"
copyright="${3:-/usr/share/doc/libxdo-dev/copyright}"
library="$(ldd "$binary" | perl -ne 'print "$1\t$2\n" if /^\s*(libxdo\.so\.\S+)\s+=>\s+(\/\S+)/')"
if [ -z "$library" ]; then
  echo "cannot find the libxdo loaded by $binary" >&2
  exit 1
fi
IFS=$'\t' read -r soname source <<<"$library"

mkdir -p "$out/lib"
# A later build can use a different SONAME; do not ship the previous one.
rm -f "$out"/lib/libxdo.so.*
install -m 755 "$binary" "$out/kanaemi-settings"
install -m 644 "$source" "$out/lib/$soname"
install -m 644 "$copyright" "$out/libxdo-copyright"
patchelf --set-rpath '$ORIGIN/lib' "$out/kanaemi-settings"
