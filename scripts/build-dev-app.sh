#!/bin/sh
# Builds the dev konnyaku.app and signs it with the identity from scripts/setup-dev-signing.sh,
# so an Accessibility grant keeps working across rebuilds.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
identity="konnyaku Local Dev"
app="$root/src-tauri/target/debug/bundle/macos/konnyaku.app"

if ! security find-identity -p codesigning | grep -q "\"$identity\""; then
  echo "No \"$identity\" signing identity. Run scripts/setup-dev-signing.sh first." >&2
  exit 1
fi

pnpm --dir "$root" tauri build --debug --bundles app
codesign --force --deep --sign "$identity" "$app"
codesign --display -r- "$app" 2>&1 | grep designated
echo "$app"
