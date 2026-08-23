#!/bin/sh

set -eu

repo_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
installer="$repo_dir/install.sh"
work_dir=$(mktemp -d)

cleanup() {
    rm -rf "$work_dir"
}
trap cleanup EXIT HUP INT TERM

asset_for_current_platform() {
    os=$(uname -s)
    architecture=$(uname -m)
    case "$os-$architecture" in
        Darwin-arm64) printf '%s\n' 'cc-statusline-darwin-arm64.tar.gz' ;;
        Darwin-x86_64) printf '%s\n' 'cc-statusline-darwin-x64.tar.gz' ;;
        Linux-aarch64 | Linux-arm64) printf '%s\n' 'cc-statusline-linux-arm64-musl.tar.gz' ;;
        Linux-x86_64) printf '%s\n' 'cc-statusline-linux-x64-musl.tar.gz' ;;
        *) printf 'unsupported test platform: %s-%s\n' "$os" "$architecture" >&2; exit 1 ;;
    esac
}

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

asset=$(asset_for_current_platform)
release_dir="$work_dir/release"
source_dir="$work_dir/source"
mkdir -p "$release_dir" "$source_dir"
printf '%s\n' 'statusline test binary' > "$source_dir/cc-statusline"
chmod 755 "$source_dir/cc-statusline"
tar -czf "$release_dir/$asset" -C "$source_dir" cc-statusline
printf '%s  %s\n' "$(sha256 "$release_dir/$asset")" "$asset" > "$release_dir/$asset.sha256"

verified_home="$work_dir/verified-home"
HOME="$verified_home" CC_STATUSLINE_BASE_URL="file://$release_dir" sh "$installer" >/dev/null
cmp "$source_dir/cc-statusline" "$verified_home/.claude/cc-statusline/cc-statusline"
test -x "$verified_home/.claude/cc-statusline/cc-statusline"

tampered_dir="$work_dir/tampered-release"
mkdir -p "$tampered_dir"
cp "$release_dir/$asset" "$tampered_dir/$asset"
printf '%s' 'tampered' >> "$tampered_dir/$asset"
cp "$release_dir/$asset.sha256" "$tampered_dir/$asset.sha256"
tampered_home="$work_dir/tampered-home"
if HOME="$tampered_home" CC_STATUSLINE_BASE_URL="file://$tampered_dir" sh "$installer" >/dev/null 2>&1; then
    printf 'tampered archive was unexpectedly installed\n' >&2
    exit 1
fi
if [ -e "$tampered_home/.claude/cc-statusline/cc-statusline" ]; then
    printf 'tampered archive reached the installation directory\n' >&2
    exit 1
fi

printf '%s\n' 'install.sh installation and checksum tests passed'
