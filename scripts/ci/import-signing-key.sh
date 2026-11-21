#!/usr/bin/env bash
set -euo pipefail

: "${RUNNER_TEMP:?RUNNER_TEMP is required}"
: "${GITHUB_ENV:?GITHUB_ENV is required}"
: "${YUOS_APT_SIGNING_FINGERPRINT:?YUOS_APT_SIGNING_FINGERPRINT is required}"
if [ -z "${YUOS_APT_SIGNING_PRIVATE_KEY:-}" ]; then
    echo "error: YUOS_APT_SIGNING_PRIVATE_KEY is required for signed publication" >&2
    exit 1
fi

export GNUPGHOME
GNUPGHOME="$(mktemp -d "$RUNNER_TEMP/yuos-release-gnupg.XXXXXX")"
printf '%s' "$YUOS_APT_SIGNING_PRIVATE_KEY" | gpg --batch --import
unset YUOS_APT_SIGNING_PRIVATE_KEY
key_id="$(gpg --batch --with-colons --list-secret-keys \
    | awk -F: '$1 == "sec" { primary = 1; next } $1 == "fpr" && primary { print $10; primary = 0 }')"
if [[ "$key_id" != "$YUOS_APT_SIGNING_FINGERPRINT" ]]; then
    echo "error: imported signing key does not match the Yu OS archive fingerprint" >&2
    exit 1
fi
keyring="$GNUPGHOME/archive-keyring.gpg"
gpg --batch --export "$key_id" > "$keyring"
test -s "$keyring"
{
    echo "GNUPGHOME=$GNUPGHOME"
    echo "GPG_KEY_ID=$key_id"
    echo "YUOS_RELEASE_SECURITY_KEY_ID=$key_id"
    echo "YUOS_RELEASE_SECURITY_KEYRING=$keyring"
} >> "$GITHUB_ENV"
