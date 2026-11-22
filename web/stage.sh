#!/usr/bin/env bash
# Stage the static website with its fonts and the shared brand assets for Pages composition.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:?usage: web/stage.sh OUTPUT_DIR}"

mkdir -p "$out/assets" "$out/install"
sha="$(git -C "$root" rev-parse --short HEAD)"
for page in index.html install/index.html; do
    sed "s/@@GIT_SHA@@/$sha/g" "$root/web/$page" > "$out/$page"
done
cp "$root/web/style.css" "$root/web/app.js" "$out/"
cp "$root/web/install/install.js" "$out/install/"
cp -R "$root/web/fonts" "$out/"
cp -R "$root/assets/brand" "$out/assets/"
