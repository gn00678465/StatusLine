#!/bin/sh

set -eu

VERSION="2.0.0-rc"
REPOSITORY_URL="https://github.com/gn00678465/StatusLine"

manual_install_hint() {
    printf '%s\n' "Manual install: download the matching asset from $REPOSITORY_URL/releases and place it in ~/.claude/cc-statusline/." >&2
}

fail() {
    printf '%s\n' "cc-statusline installation failed: $1" >&2
    manual_install_hint
    exit 1
}

asset_for_current_platform() {
    os=$(uname -s)
    architecture=$(uname -m)
    case "$os-$architecture" in
        Darwin-arm64) printf '%s\n' 'cc-statusline-darwin-arm64.tar.gz' ;;
        Darwin-x86_64) printf '%s\n' 'cc-statusline-darwin-x64.tar.gz' ;;
        Linux-aarch64 | Linux-arm64) printf '%s\n' 'cc-statusline-linux-arm64-musl.tar.gz' ;;
        Linux-x86_64) printf '%s\n' 'cc-statusline-linux-x64-musl.tar.gz' ;;
        *) fail "unsupported platform: $os-$architecture" ;;
    esac
}

download() {
    url=$1
    destination=$2
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$destination"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$destination" "$url"
    else
        fail "curl or wget is required to download releases"
    fi
}

sha256() {
    file=$1
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$file" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$file" | awk '{print $1}'
    else
        fail "sha256sum or shasum is required to verify releases"
    fi
}

asset=$(asset_for_current_platform)
base_url=${CC_STATUSLINE_BASE_URL:-"$REPOSITORY_URL/releases/download/v$VERSION"}
base_url=${base_url%/}
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/cc-statusline.XXXXXX") || fail "could not create a temporary directory"

cleanup() {
    rm -rf "$work_dir"
}
trap cleanup EXIT HUP INT TERM

archive="$work_dir/$asset"
checksum="$work_dir/$asset.sha256"
download "$base_url/$asset" "$archive" || fail "could not download $asset"
download "$base_url/$asset.sha256" "$checksum" || fail "could not download $asset.sha256"

expected=$(awk -v filename="$asset" '$2 == filename { print $1; exit }' "$checksum")
case "$expected" in
    '' | *[!0123456789abcdefABCDEF]*) fail "invalid checksum sidecar for $asset" ;;
esac
if [ "${#expected}" -ne 64 ]; then
    fail "invalid checksum sidecar for $asset"
fi
actual=$(sha256 "$archive")
if [ "$actual" != "$(printf '%s' "$expected" | tr '[:upper:]' '[:lower:]')" ]; then
    fail "SHA-256 verification failed for $asset"
fi

extract_dir="$work_dir/extract"
mkdir "$extract_dir"
tar -xzf "$archive" -C "$extract_dir" || fail "could not extract $asset"
binary="$extract_dir/cc-statusline"
if [ ! -f "$binary" ]; then
    fail "archive does not contain cc-statusline"
fi

if [ -z "${HOME:-}" ]; then
    fail "could not determine the home directory"
fi
destination_dir="$HOME/.claude/cc-statusline"
destination="$destination_dir/cc-statusline"
mkdir -p "$destination_dir"
cp "$binary" "$destination"
chmod 755 "$destination"
printf '%s\n' "Installed cc-statusline to $destination"
