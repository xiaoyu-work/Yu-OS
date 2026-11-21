#!/usr/bin/env bash
# presets/wsl.sh — Yu OS as an installable WSL2 distribution.
#
# Builds a modern WSL distribution containing the headless Yu OS
# runtime (cos / yuosd / browser stack, no COSMIC desktop — WSL has no
# graphical session). Install it on Windows with:
#
#     wsl --install --from-file build\yuos-wsl-<arch>.wsl \
#         --name YuOS --location C:\YuOS --version 2
#
# Note: this produces the Yu OS *distro* you run inside WSL. It is a
# build artifact, distinct from the Ubuntu WSL you build it in.
#
# Uses the wsl target's own default feature set (base, yuos-core, browser,
# systemd, apt-source, qwen3-embedding), so no FEATURES override is needed.
#
# Usage:
#   sudo ./presets/wsl.sh
#
# Output: build/yuos-wsl-<arch>.wsl

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

run_preset wsl
