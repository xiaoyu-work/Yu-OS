#!/usr/bin/env bash
# Stage the static website with the shared brand assets for Pages composition.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:?usage: web/stage.sh OUTPUT_DIR}"

mkdir -p "$out/assets"
cp "$root/web/index.html" "$root/web/style.css" "$root/web/app.js" "$out/"
cp -R "$root/assets/brand" "$out/assets/"
