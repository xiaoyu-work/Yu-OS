#!/bin/bash
set -euo pipefail

cd "${GITHUB_WORKSPACE:?GITHUB_WORKSPACE is required}"
: "${GITHUB_ENV:?GITHUB_ENV is required}"

if [[ -n ${APP_SOURCE_ARTIFACT:-} ]]; then
  if [[ -n ${APP_CANDIDATE_REVISION:-} ]]; then
    echo "Use either the release source artifact or an App candidate revision" >&2
    exit 1
  fi
  export YUOS_APP_SOURCE_LOCK="$GITHUB_WORKSPACE/build/release/app-sources.json"
elif [[ -n ${APP_CANDIDATE_REVISION:-} ]]; then
  lock="$GITHUB_WORKSPACE/build/app-validation-source.json"
  env -u YUOS_APP_SOURCE_LOCK python3 - "$lock" "$APP_CANDIDATE_REVISION" <<'PY'
import sys
from scripts import app_sources

selection = app_sources.read_lock()
app_sources.write_lock(sys.argv[1], {**selection, "version": 1, "revision": sys.argv[2]})
PY
  export YUOS_APP_SOURCE_LOCK="$lock"
  if [[ ${1:-} == native-* ]]; then
    version="$(python3 -c 'import json; print(json.load(open("packaging/app-platform.json"))["version"])')"
    output="$GITHUB_WORKSPACE/build/app-platform-acceptance"
    python3 scripts/app_platform.py --version "$version" --output "$output"
    printf 'YUOS_ACCEPTANCE_PLATFORM=%s/yuos-app-platform-%s.tar.gz\n' "$output" "$version" >> "$GITHUB_ENV"
  fi
fi

if [[ -n ${YUOS_APP_SOURCE_LOCK:-} ]]; then
  printf 'YUOS_APP_SOURCE_LOCK=%s\n' "$YUOS_APP_SOURCE_LOCK" >> "$GITHUB_ENV"
fi
python3 scripts/app_sources.py
