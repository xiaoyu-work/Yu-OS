#!/usr/bin/env bash
# rootfs/features/apt-source/install.sh — configure the Yu OS apt repo on
# the installed system. After this runs, `apt update` will see
# yuos-framework, yuos-agent, yuos-base, yuos-desktop and yuos-agent-desktop
# as upgradeable packages.
#
# Inherited from environment: ROOTFS.
#
# Overridable env vars:
#   YUOS_APT_REPO_URL    — repo base URL (default: official GH Pages)
#   YUOS_APT_REPO_SUITE  — suite name    (default: trixie)
#   YUOS_APT_PUBLIC_KEY_FILE — binary OpenPGP public keyring
#   YUOS_APT_PUBLIC_KEY_URL  — fallback public key URL
#   YUOS_APT_PUBLIC_KEY_FINGERPRINT — required when downloading the key

set -euo pipefail

YUOS_APT_REPO_URL="${YUOS_APT_REPO_URL:-https://xiaoyu-work.github.io/Yu-OS/apt}"
YUOS_APT_REPO_SUITE="${YUOS_APT_REPO_SUITE:-trixie}"
YUOS_APT_PUBLIC_KEY_FILE="${YUOS_APT_PUBLIC_KEY_FILE:-$PROJECT_DIR/packaging/apt-repo/yuos-archive-keyring.gpg}"
YUOS_APT_PUBLIC_KEY_URL="${YUOS_APT_PUBLIC_KEY_URL:-https://xiaoyu-work.github.io/Yu-OS/apt/yuos-archive-keyring.gpg}"
YUOS_APT_PUBLIC_KEY_FINGERPRINT="${YUOS_APT_PUBLIC_KEY_FINGERPRINT:-}"

if [ ! -s "$YUOS_APT_PUBLIC_KEY_FILE" ]; then
    if [ -z "$YUOS_APT_PUBLIC_KEY_FINGERPRINT" ]; then
        echo "error: set YUOS_APT_PUBLIC_KEY_FILE or YUOS_APT_PUBLIC_KEY_FINGERPRINT" >&2
        exit 1
    fi
    case "$YUOS_APT_PUBLIC_KEY_URL" in
        https://*) ;;
        *)
            echo "error: apt public key URL must use HTTPS" >&2
            exit 1
            ;;
    esac
    YUOS_APT_PUBLIC_KEY_FILE="$(mktemp)"
    trap 'rm -f "$YUOS_APT_PUBLIC_KEY_FILE"' EXIT
    echo "  :: fetching apt public key from $YUOS_APT_PUBLIC_KEY_URL"
    curl --proto '=https' --proto-redir '=https' -fsSL \
        "$YUOS_APT_PUBLIC_KEY_URL" -o "$YUOS_APT_PUBLIC_KEY_FILE"
fi
if ! gpg --batch --show-keys "$YUOS_APT_PUBLIC_KEY_FILE" >/dev/null 2>&1; then
    echo "error: invalid Yu OS apt public key: $YUOS_APT_PUBLIC_KEY_FILE" >&2
    exit 1
fi
if [ -n "$YUOS_APT_PUBLIC_KEY_FINGERPRINT" ]; then
    mapfile -t primary_fingerprints < <(
        gpg --batch --with-colons --show-keys "$YUOS_APT_PUBLIC_KEY_FILE" \
            | awk -F: '
                $1 == "pub" { want_fpr = 1; next }
                want_fpr && $1 == "fpr" {
                    print toupper($10)
                    want_fpr = 0
                }
            '
    )
    expected_fingerprint="$(printf '%s' "$YUOS_APT_PUBLIC_KEY_FINGERPRINT" \
        | tr -d '[:space:]' | tr '[:lower:]' '[:upper:]')"
    if [ "${#primary_fingerprints[@]}" -ne 1 ] \
        || [ "${primary_fingerprints[0]}" != "$expected_fingerprint" ]; then
        echo "error: Yu OS apt public key fingerprint mismatch" >&2
        exit 1
    fi
fi

echo "  :: writing /etc/apt/sources.list.d/yuos.list"
mkdir -p "$ROOTFS/etc/apt/sources.list.d" "$ROOTFS/etc/apt/apt.conf.d"
install -Dm0644 "$YUOS_APT_PUBLIC_KEY_FILE" \
    "$ROOTFS/usr/share/keyrings/yuos-archive-keyring.gpg"

# Signature options are freshness options too. `check-valid-until=yes`
# makes APT refuse an index whose signed `Valid-Until` has passed, so a
# stale or frozen mirror cannot keep offering a superseded release
# indefinitely; the `allow-*` options refuse every insecure fallback
# that would otherwise accept it. Downgrade protection itself lives in
# the packages and in /var/lib/yuos/security — these only stop the
# repository layer from handing an old snapshot over in the first
# place.
cat > "$ROOTFS/etc/apt/sources.list.d/yuos.list" <<EOF
# Yu OS — official package repository.
# Source: https://github.com/xiaoyu-work/Yu-OS
deb [signed-by=/usr/share/keyrings/yuos-archive-keyring.gpg check-valid-until=yes allow-insecure=no allow-weak=no allow-downgrade-to-insecure=no by-hash=yes] $YUOS_APT_REPO_URL $YUOS_APT_REPO_SUITE main
EOF

cat > "$ROOTFS/etc/apt/apt.conf.d/50yuos-repository" <<'EOF'
// Yu OS repository freshness.
//
// APT signatures prove who published an index; `Valid-Until` is what
// bounds how long that index may still be served. Keeping the check on
// — and refusing the insecure fallbacks that would bypass it — is what
// stops a stale or hostile mirror from replaying an old, still validly
// signed snapshot at an installed system.
Acquire::Check-Valid-Until "true";
Acquire::AllowInsecureRepositories "false";
Acquire::AllowDowngradeToInsecureRepositories "false";
EOF

echo "  :: apt source ready ($YUOS_APT_REPO_URL $YUOS_APT_REPO_SUITE main)"
