#!/usr/bin/env bash
# Exercise packaging without requiring a Linux desktop on the test host.
set -euo pipefail

root="$(cd "$(dirname "$0")/../../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/system"
printf 'settings executable\n' >"$work/kanaemi-settings"
printf 'xdo library\n' >"$work/system/libxdo.so.3"
printf 'xdo license\n' >"$work/copyright"
export FIXTURE="$work"

cat >"$work/bin/ldd" <<'EOF'
#!/usr/bin/env bash
printf ' libxdo.so.3 => %s/system/libxdo.so.3 (0x1234)\n' "$FIXTURE"
EOF
cat >"$work/bin/patchelf" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$@" >"$FIXTURE/patchelf-args"
EOF
chmod +x "$work/bin/ldd" "$work/bin/patchelf"
export PATH="$work/bin:$PATH"

bash "$root/apps/settings/package/linux.sh" "$work/kanaemi-settings" "$work/out" "$work/copyright"
cmp "$work/kanaemi-settings" "$work/out/kanaemi-settings"
cmp "$work/system/libxdo.so.3" "$work/out/lib/libxdo.so.3"
cmp "$work/copyright" "$work/out/libxdo-copyright"
printf '%s\n' --set-rpath '$ORIGIN/lib' "$work/out/kanaemi-settings" >"$work/expected-args"
cmp "$work/expected-args" "$work/patchelf-args"

# A new SONAME must be bundled under the name the loader asks for too.
mv "$work/system/libxdo.so.3" "$work/system/libxdo.so.4"
perl -pi -e 's/so\.3/so.4/g' "$work/bin/ldd"
bash "$root/apps/settings/package/linux.sh" "$work/kanaemi-settings" "$work/out" "$work/copyright"
cmp "$work/system/libxdo.so.4" "$work/out/lib/libxdo.so.4"
test ! -e "$work/out/lib/libxdo.so.3"

# Never publish an app whose dependency could not be found.
printf '#!/usr/bin/env bash\nprintf " libxdo.so.4 => not found\\n"\n' >"$work/bin/ldd"
if bash "$root/apps/settings/package/linux.sh" "$work/kanaemi-settings" "$work/missing" "$work/copyright" 2>"$work/error"; then
  echo "packaging accepted a missing libxdo" >&2
  exit 1
fi
grep 'libxdo' "$work/error" >/dev/null
