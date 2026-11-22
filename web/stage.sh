#!/usr/bin/env bash
# Stage the static website with its fonts and the shared brand assets for Pages composition.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:?usage: web/stage.sh OUTPUT_DIR}"

mkdir -p "$out/assets"
sha="$(git -C "$root" rev-parse --short HEAD)"
sed "s/@@GIT_SHA@@/$sha/g" "$root/web/index.html" > "$out/index.html"
cp "$root/web/style.css" "$root/web/app.js" "$out/"
cp -R "$root/web/fonts" "$out/"
cp -R "$root/assets/brand" "$out/assets/"
