#!/usr/bin/env bash
# Yu OS — docker target.
#
# Produces the headless Yu OS Docker image: the full non-desktop OS runtime
# (`base,yuos-core,browser,systemd,apt-source,qwen3-embedding`) with Claw's own
# cos/yuosd agent, apps, skills, browser automation, upgrade source, and the
# local embedding stack on architectures where upstream ships a Linux
# ort-genai runtime. It intentionally does not include desktop UI,
# installer/boot/VM-only features, or third-party agent providers such as
# copilot-cli.
#
# The entrypoint creates the account named by CLAW_USER before systemd starts.
#
# Environment variables:
#   TAG        Docker image tag (default: yuos)
#   FEATURES   Rootfs feature set (default: headless Yu OS runtime)
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
ROOTFS="$PROJECT_DIR/build/yuos-rootfs"

source "$PROJECT_DIR/scripts/lib/arch.sh"
source "$PROJECT_DIR/scripts/lib/image-identity.sh"
source "$PROJECT_DIR/scripts/lib/image-profiles.sh"

FEATURES="${FEATURES:-$IMAGE_FEATURES_HEADLESS_RUNTIME}"

DOCKERFILE="$SCRIPT_DIR/Dockerfile"
TAG="${TAG:-yuos}"

if [ ! -f "$DOCKERFILE" ]; then
    echo "error: dockerfile not found: $DOCKERFILE" >&2
    exit 1
fi

if [ "$(id -u)" -ne 0 ]; then
    echo "error: must run as root (debootstrap + chroot need it)" >&2
    echo "       run: sudo ./targets/docker/build.sh" >&2
    exit 1
fi

"$PROJECT_DIR/rootfs/build.sh" --reuse-if-matching --features "$FEATURES"
assert_no_human_login_users "$ROOTFS" "Docker image"

cd "$PROJECT_DIR"
docker build \
    --build-arg "ROOTFS_DIR=build/$(basename "$ROOTFS")" \
    -f "$DOCKERFILE" -t "$TAG" "$@" .
