#!/bin/sh
# Install Ultron Hands from the live one-liner on fresh Linux distros (Docker) and run
# the end-to-end driver against a virtual X screen in each.
#
#   tests/e2e/linux.sh [out-dir] [image ...]
#
# Each image gets its own folder of screenshots, results.json and log.txt.
# E2E_BIN=/path/to/static/ultron-hands tests a local build instead of the live install.
set -u
here=$(cd "$(dirname "$0")" && pwd)
out=$(realpath -m "${1:-e2e-linux}")
[ $# -gt 0 ] && shift
images=${*:-"ubuntu:22.04 ubuntu:24.04 debian:12 fedora:41 archlinux:latest opensuse/tumbleweed alpine:3.20"}
mkdir -p "$out"

# Runs inside the container.
inner='
set -e
if command -v apt-get >/dev/null; then
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq && apt-get install -y -qq curl ca-certificates python3 xvfb xterm xdotool fonts-dejavu-core >/dev/null
elif command -v dnf >/dev/null; then
  dnf install -y -q --setopt=install_weak_deps=False curl python3 xorg-x11-server-Xvfb xterm xdotool tar gzip >/dev/null
elif command -v pacman >/dev/null; then
  pacman -Sy --noconfirm --needed curl python xorg-server-xvfb xterm xdotool ttf-dejavu >/dev/null
elif command -v zypper >/dev/null; then
  zypper -n -q in curl python3 xorg-x11-server-Xvfb xterm xdotool dejavu-fonts tar gzip >/dev/null
elif command -v apk >/dev/null; then
  apk add -q curl python3 xvfb xterm xdotool font-dejavu
fi
Xvfb :99 -screen 0 1600x900x24 -nolisten tcp >/dev/null 2>&1 &
export DISPLAY=:99 SHELL=/bin/sh
sleep 2
if [ -x /uh ]; then
  bin=/uh
else
  echo "== install"
  curl -fsSL https://getultron.vercel.app/install.sh | sh
  bin="$HOME/.local/bin/ultron-hands"
fi
echo "== e2e"
python3 /e2e/drive.py "$bin" /out
'

mount=""
[ -n "${E2E_BIN:-}" ] && mount="-v $(realpath "$E2E_BIN"):/uh:ro"
# Two at a time: package installs need disk space.
jobs=${E2E_JOBS:-2}
pids=""
n=0
for img in $images; do
  name=$(echo "$img" | tr '/:' '__')
  mkdir -p "$out/$name"
  (
    # shellcheck disable=SC2086
    docker run --rm $mount -v "$here:/e2e:ro" -v "$out/$name:/out" "$img" sh -c "$inner" >"$out/$name/log.txt" 2>&1
    echo "$?" >"$out/$name/exit"
  ) &
  pids="$pids $!"
  n=$((n + 1))
  if [ $((n % jobs)) -eq 0 ]; then wait $pids; pids=""; fi
done
wait $pids

printf '\n%-24s %s\n' IMAGE RESULT
for img in $images; do
  name=$(echo "$img" | tr '/:' '__')
  code=$(cat "$out/$name/exit" 2>/dev/null || echo "?")
  summary=$(grep -E '^[0-9]+/[0-9]+ passed' "$out/$name/log.txt" | tail -1)
  [ -z "$summary" ] && summary="no result: $(grep -v '^\s*$' "$out/$name/log.txt" | tail -1)"
  printf '%-24s exit %s  %s\n' "$img" "$code" "$summary"
done
