#!/usr/bin/env bash
# rootfs/features/yuos-mail-ai/install.sh — wire the Yu OS Mail AI
# Thunderbird MailExtension and its Python Native Messaging host into
# the rootfs.
#
# Lay-down:
#   /usr/lib/yuos/apps/mail-ai/               — shared Python package (from agent package)
#   /usr/lib/yuos/yuos-mail-ai-host           — trusted native launcher (from agent package)
#   /etc/thunderbird/native-messaging-hosts/os.yuos.mail_ai.json
#                                            — NM host manifest
#   /usr/lib/thunderbird/distribution/extensions/yu-mail-ai@yuos.io.xpi
#                                            — package-owned UI protocol adapter
#   /etc/thunderbird/policies/policies.json  — pin/lock + privacy defaults
#
# Inherited: $ROOTFS, $PROJECT_DIR, $SCRIPT_DIR (features/), $YUOS_VERSION.

set -euo pipefail

FEATURE_DIR="$SCRIPT_DIR/features/yuos-mail-ai"
APP_DEST="$ROOTFS/usr/lib/yuos/apps/email"

EXT_ID="yu-mail-ai@yuos.io"
XPI_DEST="$ROOTFS/usr/lib/thunderbird/distribution/extensions/${EXT_ID}.xpi"

# ---------------------------------------------------------------------------
# 0. Sanity — require the installed package, not product source trees.
# ---------------------------------------------------------------------------
for d in "$APP_DEST" \
    "$ROOTFS/usr/lib/yuos/python/yuos_sdk"; do
    if [ ! -d "$d" ]; then
        echo "  error: required Mail package directory missing: $d" >&2
        exit 1
    fi
done
for file in app.json main.py server.py native_host.py README.md; do
    if [ ! -f "$APP_DEST/$file" ]; then
        echo "  error: yuos-framework Mail package is incomplete: $APP_DEST/$file" >&2
        exit 1
    fi
done
if [ ! -f "$XPI_DEST" ]; then
    echo "  error: yuos-framework Mail extension is missing: $XPI_DEST" >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# 1. Apply static overlay (NM manifest and policies).
# ---------------------------------------------------------------------------
if [ -d "$FEATURE_DIR/overlay" ] && [ -n "$(ls -A "$FEATURE_DIR/overlay" 2>/dev/null)" ]; then
    echo "  :: applying yuos-mail-ai overlay"
    cp -a --no-preserve=ownership "$FEATURE_DIR/overlay/." "$ROOTFS/"
fi
if [ ! -x "$ROOTFS/usr/lib/yuos/yuos-mail-ai-host" ]; then
    echo "  error: trusted yuos-mail-ai-host binary is missing from yuos-framework" >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# 2. Use the canonical App and SDK already installed by yuos-framework.
#    Do not overwrite authenticated package contents or maintain a separate
#    Python copy for the UI that can drift from the Framework's implementation.
# ---------------------------------------------------------------------------
echo "  :: using package-owned Mail host and extension"

# ---------------------------------------------------------------------------
# 3. Install the package-owned Mail documentation.
# ---------------------------------------------------------------------------
README_DEST="$ROOTFS/usr/share/doc/yuos-mail-ai"
install -d -m 0755 "$README_DEST"
cp "$APP_DEST/README.md" "$README_DEST/README.md"

echo "  :: yuos-mail-ai feature applied"
