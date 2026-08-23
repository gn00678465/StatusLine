#!/usr/bin/env bash

set -euo pipefail

repo_dir=$(cd "$(dirname "$0")/.." && pwd)
shell_script="$repo_dir/claudeStatusLine.sh"
whitelist_file="$repo_dir/.scratch/rust-rewrite/parity-whitelist.md"
rust_binary=${CC_STATUSLINE_BIN:-"$repo_dir/target/debug/cc-statusline"}
fixture_names=(
    status-input.json
    status-input-boundaries.json
    status-input-colors.json
    status-input-oauth.json
    status-input-seven-day-only.json
    status-input-xhigh.json
    status-input-effort-missing.json
    status-input-one-million.json
    status-input-multiple-weekly.json
    status-input-wrapped.json
)

if [ ! -f "$whitelist_file" ]; then
    printf 'Missing parity whitelist: %s\n' "$whitelist_file" >&2
    exit 1
fi

if [ ! -x "$rust_binary" ]; then
    cargo build --quiet --manifest-path "$repo_dir/Cargo.toml"
fi

work_dir=$(mktemp -d)
cleanup() {
    [ ! -d "$work_dir" ] || rm -rf "$work_dir"
}
trap cleanup EXIT

fake_home="$work_dir/home"
runtime_dir="$fake_home/runtime"
config_dir="$fake_home/claude"
project_dir="$fake_home/project"
cache_dir="$runtime_dir/StatusLine"
mkdir -p "$runtime_dir" "$config_dir" "$project_dir" "$cache_dir"
chmod 700 "$fake_home" "$runtime_dir" "$config_dir" "$project_dir" "$cache_dir"

git -C "$project_dir" init -q
git -C "$project_dir" symbolic-ref HEAD refs/heads/integration
git -C "$project_dir" \
    -c user.name=Parity -c user.email=parity@example.invalid \
    -c commit.gpgSign=false -c core.hooksPath=/dev/null \
    commit -q --allow-empty -m initial

config_key=$(printf '%s' "$config_dir" | LC_ALL=C tr -c 'A-Za-z0-9' '_')
oauth_cache="$cache_dir/usage-cache-${config_key:0:64}.json"
oauth_response='{"five_hour":{"utilization":20,"resets_at":"2030-03-17T17:46:40Z"},"seven_day":{"utilization":50,"resets_at":"2030-03-24T17:46:40Z"},"extra_usage":{"is_enabled":false},"limits":[{"kind":"weekly_scoped","scope":{"model":{"display_name":"Other"}},"percent":99,"resets_at":"2030-03-24T17:46:40Z"},{"kind":"weekly_scoped","scope":{"model":{"display_name":"Fable"}},"percent":30,"resets_at":"2030-03-24T17:46:40Z"}]}'
printf '%s' "$oauth_response" > "$oauth_cache"
printf '%s' '{"tag_name":"v0.1.0"}' > "$cache_dir/statusline-version-cache.json"

unset CLAUDE_CODE_OAUTH_TOKEN
unset CLAUDE_CODE_EFFORT_LEVEL

strip_ansi() {
    sed $'s/\033\[[0-9;]*m//g'
}

normalize_output() {
    strip_ansi \
        | sed -E \
            -e 's/@([0-9]{2}:[0-9]{2}|[A-Z][a-z]{2} [0-9]{1,2}, [0-9]{2}:[0-9]{2})/@<reset>/g' \
            -e 's/Cache ([0-9]+%) [0-9]+:[0-9]{2}/Cache \1 <ttl>/g' \
            -e 's/ · 🧠 [^ │]+//g' \
            -e 's/ · Other: [^ ]+ [0-9]+% @<reset>//g' \
            -e 's/ · Fable: / · Fable 5: /g'
}

for fixture_name in "${fixture_names[@]}"; do
    fixture_path="$repo_dir/tests/fixtures/$fixture_name"
    input=$(jq --arg cwd "$project_dir" '.cwd = $cwd' "$fixture_path")
    shell_output=$(printf '%s' "$input" | env \
        HOME="$fake_home" \
        XDG_RUNTIME_DIR="$runtime_dir" \
        CLAUDE_CONFIG_DIR="$config_dir" \
        PATH="$repo_dir/tests/mock-bin:$PATH" \
        COLUMNS=10000 \
        STATUSLINE_GIT_CACHE_TTL=0 \
        bash "$shell_script")
    rust_output=$(printf '%s' "$input" | env \
        HOME="$fake_home" \
        XDG_RUNTIME_DIR="$runtime_dir" \
        CLAUDE_CONFIG_DIR="$config_dir" \
        PATH="$repo_dir/tests/mock-bin:$PATH" \
        COLUMNS=10000 \
        STATUSLINE_GIT_CACHE_TTL=0 \
        "$rust_binary")
    shell_normalized=$(printf '%s' "$shell_output" | normalize_output)
    rust_normalized=$(printf '%s' "$rust_output" | normalize_output)

    if ! diff -u \
        <(printf '%s\n' "$shell_normalized") \
        <(printf '%s\n' "$rust_normalized"); then
        printf 'Parity mismatch for %s; only parity-whitelist.md differences are normalized.\n' \
            "$fixture_name" >&2
        exit 1
    fi
done

printf 'Parity verification passed for %d fixtures\n' "${#fixture_names[@]}"
