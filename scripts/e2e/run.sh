#!/bin/sh
# End-to-end run of a dev build without Accessibility: the synthetic copy cannot land, so the app
# translates the clipboard. Puts rich HTML on the clipboard, triggers a translation, prints the
# rendered 訳文, screenshots the window, then restores the user's clipboard and quits the app.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
bundle="$here/../../src-tauri/target/debug/bundle/macos/konnyaku.app"
out=${1:-$(mktemp -d)}
mkdir -p "$out"

swiftc -O "$here/pbtool.swift" -o "$out/pbtool"
swiftc -O "$here/winid.swift" -o "$out/winid"

"$out/pbtool" save "$out/clipboard.plist"
trap '"$out/pbtool" restore "$out/clipboard.plist"; pkill -f "konnyaku.app/Contents/MacOS/konnyaku" || true' EXIT
"$out/pbtool" html "$here/rich.html"

rm -f "$out/rendered.html"
open -n --env KONNYAKU_DUMP="$out/rendered.html" "$bundle"
sleep 4
"$bundle/Contents/MacOS/konnyaku" --translate-now
i=0
until [ -f "$out/rendered.html" ] || [ $i -ge 240 ]; do sleep 0.25; i=$((i + 1)); done
[ -f "$out/rendered.html" ] || { echo "no rendered 訳文 within 60s" >&2; exit 1; }

cat "$out/rendered.html"
window=$("$out/winid" | awk '$2 == "konnyaku" { print $1; exit }')
screencapture -x -o -l "$window" "$out/window.png" && echo "screenshot: $out/window.png"
