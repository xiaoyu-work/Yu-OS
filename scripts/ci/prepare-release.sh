#!/usr/bin/env bash
set -euo pipefail

cd "${GITHUB_WORKSPACE:?GITHUB_WORKSPACE is required}"
: "${GITHUB_OUTPUT:?GITHUB_OUTPUT is required}"
target="${RELEASE_TARGET:?RELEASE_TARGET is required}"
agent=false base=false desktop=false images=false installer=false pages=false
case "$target" in
    agent) agent=true; pages=true ;;
    base) base=true; pages=true ;;
    core) agent=true; base=true; pages=true ;;
    desktop) desktop=true; pages=true ;;
    images) agent=true; base=true; images=true ;;
    installer) agent=true; base=true; desktop=true; installer=true ;;
    website) pages=true ;;
    all) agent=true; base=true; desktop=true; images=true; pages=true ;;
    *) echo "error: unknown release target '$target'" >&2; exit 2 ;;
esac
if [ "${RELEASE_SECURITY_BOOTSTRAP:-0}" = "1" ] \
    && [[ "$target" = images || "$target" = installer || "$target" = website ]]; then
    echo "error: release-security bootstrap requires a package publication target" >&2
    exit 2
fi

run="$(gh api "repos/${GITHUB_REPOSITORY:?}/actions/runs/${GITHUB_RUN_ID:?}" \
    --jq '[.created_at[:10], .workflow_id, .run_number] | @tsv')"
IFS=$'\t' read -r build_day workflow run_number <<< "$run"
if ! [[ "$build_day" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ &&
        "$workflow" =~ ^[1-9][0-9]*$ && "$run_number" =~ ^[1-9][0-9]*$ ]]; then
    echo "error: invalid GitHub Actions release run metadata" >&2
    exit 1
fi

# Anchor the sequence before this UTC day, so deleted runs today do not
# reuse a number and a rerun keeps the original creation date.
previous="$(gh api --method GET "repos/$GITHUB_REPOSITORY/actions/workflows/$workflow/runs" \
    -f "created=<$build_day" -F per_page=1 --jq '.workflow_runs[0].run_number // 0')"
if ! [[ "$previous" =~ ^(0|[1-9][0-9]*)$ ]] || (( previous >= run_number )); then
    echo "error: invalid GitHub Actions release sequence for $build_day" >&2
    exit 1
fi
stamp="$(printf '%s.%02d' "${build_day//-/}" "$((run_number - previous))")"
version="$(bash scripts/lib/package-version.sh "$stamp")"
mkdir -p build/release
app_revision=
if [ "$desktop" = true ]; then
    app_revision="$(python3 - "$target" <<'PY'
import sys
from scripts import app_sources

selection = app_sources.read_lock()
if selection["version"] != 2:
    raise ValueError("Desktop releases must resolve the authoritative App main")
revision = app_sources.resolve_main(selection)
if sys.argv[1] == "all":
    # The reusable test workflow uses the very same commit. Production
    # ordinary App packages still come exclusively from the signed APT inputs.
    app_sources.write_lock(
        "build/release/app-sources.json",
        {**selection, "version": 1, "revision": revision},
    )
print(revision)
PY
)"
fi
if [ "$agent" = true ] || [ "$desktop" = true ]; then
    python3 scripts/app_packages.py check-version --version "$version"
fi
export YUOS_APP_PACKAGE_ROOT="$GITHUB_WORKSPACE/build/release/app-packages"
for variant in agent desktop; do
    if [ "$variant" = agent ] && [ "$agent" != true ]; then continue; fi
    if [ "$variant" = desktop ] && [ "$desktop" != true ]; then continue; fi
    for architecture in amd64 arm64; do
        python3 scripts/app_packages.py prepare --variant "$variant" --architecture "$architecture"
    done
done
{
    printf 'version=%s\n' "$version"
    printf 'app_revision=%s\n' "$app_revision"
    printf 'agent=%s\nbase=%s\ndesktop=%s\nimages=%s\ninstaller=%s\npages=%s\n' \
        "$agent" "$base" "$desktop" "$images" "$installer" "$pages"
} >> "$GITHUB_OUTPUT"
