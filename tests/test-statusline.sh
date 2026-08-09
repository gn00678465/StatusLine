#!/bin/bash

set -eu

repo_dir=$(cd "$(dirname "$0")/.." && pwd)
script="$repo_dir/claudeStatusLine.sh"
mock_path="$repo_dir/tests/mock-bin:$PATH"
failures=0

strip_ansi() {
    sed $'s/\033\[[0-9;]*m//g'
}

run_statusline_raw() {
    local fixture=$1 style=${2:-bar} fable_missing=${3:-0} color_scenario=${4:-0}
    local mock_home
    local output
    mock_home=$(mktemp -d)
    mkdir -p "$mock_home/runtime"
    chmod 700 "$mock_home" "$mock_home/runtime"

    output=$(env \
        STATUSLINE_USAGE_STYLE="$style" \
        MOCK_FABLE_MISSING="$fable_missing" \
        MOCK_COLOR_SCENARIO="$color_scenario" \
        CLAUDE_CODE_OAUTH_TOKEN="mock-token" \
        CLAUDE_CONFIG_DIR="$mock_home/.claude" \
        XDG_RUNTIME_DIR="$mock_home/runtime" \
        HOME="$mock_home" \
        PATH="$mock_path" \
        COLUMNS=1000 \
        bash "$script" < "$repo_dir/tests/fixtures/$fixture")

    rm -rf -- "$mock_home"
    printf '%s' "$output"
}

run_statusline() {
    run_statusline_raw "$@" | strip_ansi
}

assert_contains() {
    local output=$1 expected=$2 label=$3
    if [[ "$output" != *"$expected"* ]]; then
        printf 'FAIL: %s\n  expected: %s\n  output: %s\n' "$label" "$expected" "$output" >&2
        failures=$((failures + 1))
    fi
}

assert_not_contains() {
    local output=$1 unexpected=$2 label=$3
    if [[ "$output" == *"$unexpected"* ]]; then
        printf 'FAIL: %s\n  unexpected: %s\n  output: %s\n' "$label" "$unexpected" "$output" >&2
        failures=$((failures + 1))
    fi
}

bar_output=$(run_statusline status-input.json)
assert_contains "$bar_output" '50k/200k (▓▓░░░░░░░░ 25%)' 'default context bar'
assert_contains "$bar_output" '5h: ▓▓░░░░░░░░ 20%' 'default 5h bar'
assert_contains "$bar_output" '7d: ▓▓▓▓▓░░░░░ 50%' 'default weekly bar'
assert_contains "$bar_output" 'F5: ▓▓▓░░░░░░░ 30%' 'Fable weekly bar with built-in rates'

dot_output=$(run_statusline status-input.json dots)
assert_contains "$dot_output" '50k/200k (●●○○○○○○○○ 25%)' '10-dot context meter'
assert_contains "$dot_output" '5h: ●●○○○○○○○○ 20%' '10-dot 5h meter'
assert_contains "$dot_output" '7d: ●●●●●○○○○○ 50%' '10-dot weekly meter'
assert_contains "$dot_output" 'F5: ●●●○○○○○○○ 30%' '10-dot Fable weekly meter'

oauth_output=$(run_statusline status-input-oauth.json dots)
assert_contains "$oauth_output" '5h: ●●○○○○○○○○ 20%' 'OAuth 5h fallback'
assert_contains "$oauth_output" '7d: ●●●●●○○○○○ 50%' 'OAuth weekly fallback'
assert_contains "$oauth_output" 'F5: ●●●○○○○○○○ 30%' 'OAuth Fable weekly entry'

missing_output=$(run_statusline status-input.json dots 1)
assert_not_contains "$missing_output" 'F5:' 'missing scoped Fable entry stays hidden'

invalid_style_output=$(run_statusline status-input.json invalid)
assert_contains "$invalid_style_output" '50k/200k (▓▓░░░░░░░░ 25%)' 'invalid style falls back to bar'

color_output=$(run_statusline_raw status-input-colors.json dots 0 1)
assert_contains "$color_output" $'\033[38;2;255;170;80m●●●●●' '50% dot meter uses orange'
assert_contains "$color_output" $'\033[38;2;255;230;80m●●●●●●●' '70% dot meter uses yellow'
assert_contains "$color_output" $'\033[38;2;255;100;100m●●●●●●●●●' '90% dot meter uses red'

if [ "$failures" -gt 0 ]; then
    printf '%d mock status-line assertion(s) failed\n' "$failures" >&2
    exit 1
fi

printf 'All mock status-line assertions passed\n'
