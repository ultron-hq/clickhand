#!/bin/sh
# OpenHUD for Mac: installs the app into Applications and opens it.
#
#   curl -fsSL https://openhud.vercel.app/mac.sh | sh
#
# Downloads the latest OpenHUD .dmg from GitHub Releases for this Mac (Apple silicon or
# Intel), copies OpenHUD.app into /Applications and opens it; the app's welcome screen
# does the rest (connects your AI apps, asks macOS for the two permissions once).
# Installing this way skips the "downloaded from the internet" warning, because curl
# doesn't mark files as quarantined. Source: https://github.com/ultron-hq/openhud
set -eu

REPO="ultron-hq/openhud"
say() { printf '%s\n' "$*"; }
die() { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = Darwin ] || die "this installer is for macOS. On Linux: curl -fsSL https://openhud.vercel.app/install.sh | sh"

arch=$(uname -m)
# An Apple silicon Mac running this shell under Rosetta still gets the native app.
if [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then
  arch=arm64
fi
case "$arch" in
  arm64) asset="OpenHUD-mac-apple-silicon.dmg" ;;
  x86_64) asset="OpenHUD-mac-intel.dmg" ;;
  *) die "this Mac ($arch) isn't supported" ;;
esac

tmp=$(mktemp -d)
mnt="$tmp/mnt"
cleanup() {
  hdiutil detach "$mnt" -quiet 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup EXIT INT TERM

say "Downloading OpenHUD for $( [ "$arch" = arm64 ] && echo 'Apple silicon' || echo 'Intel' )…"
# The newest release has the app a few minutes after it's published; until then, use the
# newest release that has it.
if ! curl -fsL "https://github.com/$REPO/releases/latest/download/$asset" -o "$tmp/$asset"; then
  url=$(curl -fsSL "https://api.github.com/repos/$REPO/releases?per_page=10" \
    | grep -o "\"browser_download_url\": *\"[^\"]*/$asset\"" | head -1 | sed 's/.*"\(https[^"]*\)"/\1/')
  [ -n "$url" ] || die "couldn't find $asset in the releases"
  curl -fL --progress-bar "$url" -o "$tmp/$asset" || die "couldn't download $asset"
fi

hdiutil attach "$tmp/$asset" -nobrowse -noautoopen -quiet -mountpoint "$mnt" || die "couldn't open the disk image"
app=$(find "$mnt" -maxdepth 1 -name "*.app" | head -1)
[ -n "$app" ] || die "no app in the disk image"
dest="/Applications/$(basename "$app")"

# Quit a running copy so it can be replaced.
osascript -e 'quit app "OpenHUD"' >/dev/null 2>&1 || true

say "Installing into Applications…"
if [ -w /Applications ]; then
  rm -rf "$dest" && ditto "$app" "$dest"
else
  say "(macOS asks for your password to write to Applications)"
  sudo rm -rf "$dest" && sudo ditto "$app" "$dest"
fi
xattr -dr com.apple.quarantine "$dest" 2>/dev/null || true

say "Opening OpenHUD. Follow the welcome window: it connects your AI apps and asks macOS for permission once."
open "$dest"
