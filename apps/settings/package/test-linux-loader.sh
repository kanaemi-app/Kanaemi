#!/usr/bin/env bash
# Prove that the ELF loader uses the private library after relocation, even
# when the build host's copy is gone. Run on the native Linux packaging host.
set -euo pipefail

root="$(cd "$(dirname "$0")/../../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/system"
printf 'int fixture(void) { return 0; }\n' >"$work/library.c"
printf 'int fixture(void); int main(void) { return fixture(); }\n' >"$work/main.c"
printf 'fixture license\n' >"$work/copyright"
cc -shared -fPIC -Wl,-soname,libxdo.so.999 "$work/library.c" -o "$work/system/libxdo.so.999"
cc "$work/main.c" -L"$work/system" -l:libxdo.so.999 -Wl,-rpath,"$work/system" -o "$work/kanaemi-settings"

# Red: an executable with only the build host's library path cannot run.
mv "$work/system" "$work/hidden"
if "$work/kanaemi-settings" 2>"$work/error"; then
  echo "the fixture ran without its library" >&2
  exit 1
fi
grep 'libxdo.so.999' "$work/error" >/dev/null
mv "$work/hidden" "$work/system"

bash "$root/apps/settings/package/linux.sh" "$work/kanaemi-settings" "$work/package" "$work/copyright"
ldd "$work/package/kanaemi-settings" | grep -F "$work/package/lib/libxdo.so.999" >/dev/null
mv "$work/system" "$work/hidden"
mv "$work/package" "$work/installed"
"$work/installed/kanaemi-settings"
ldd "$work/installed/kanaemi-settings" | grep -F "$work/installed/lib/libxdo.so.999" >/dev/null
