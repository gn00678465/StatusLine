#!/bin/bash

set -eu

repo_dir=$(cd "$(dirname "$0")/.." && pwd)
script="$repo_dir/claudeStatusLine.sh"
mock_path="$repo_dir/tests/mock-bin:$PATH"
failures=0

strip_ansi() {
    sed $'s/\033\[[0-9;]*m//g'
}

run_statusline_raw() (
    local fixture=$1
    local style=${STATUS_TEST_STYLE:-bar}
    local fable_missing=${STATUS_TEST_FABLE_MISSING:-0}
    local color_scenario=${STATUS_TEST_COLOR_SCENARIO:-0}
    local git_scenario=${STATUS_TEST_GIT_SCENARIO:-clean}
    local columns=${STATUS_TEST_COLUMNS:-1000}
    local model_name=${STATUS_TEST_MODEL_NAME:-}
    local mock_home
    local fixture_input
    local output
    mock_home=$(mktemp -d)
    cleanup_mock_home() {
        [ ! -d "$mock_home" ] || rm -r -- "$mock_home"
    }
    trap cleanup_mock_home EXIT
    mkdir -p "$mock_home/runtime" "$mock_home/mock-project"
    chmod 700 "$mock_home" "$mock_home/runtime" "$mock_home/mock-project"

    git -C "$mock_home/mock-project" init -q
    git -C "$mock_home/mock-project" symbolic-ref HEAD refs/heads/mock-branch
    git -C "$mock_home/mock-project" \
        -c user.name=Mock -c user.email=mock@example.invalid \
        -c commit.gpgSign=false -c core.hooksPath=/dev/null \
        commit -q --allow-empty -m init
    case "$git_scenario" in
        mixed)
            printf 'staged\n' > "$mock_home/mock-project/mixed.txt"
            git -C "$mock_home/mock-project" add mixed.txt
            printf 'unstaged-after-index\n' >> "$mock_home/mock-project/mixed.txt"
            ;;
        untracked)
            printf 'not scanned\n' > "$mock_home/mock-project/untracked.txt"
            ;;
        detached)
            git -C "$mock_home/mock-project" checkout -q --detach
            ;;
        conflict)
            printf 'base\n' > "$mock_home/mock-project/conflict.txt"
            git -C "$mock_home/mock-project" add conflict.txt
            git -C "$mock_home/mock-project" \
                -c user.name=Mock -c user.email=mock@example.invalid \
                -c commit.gpgSign=false -c core.hooksPath=/dev/null \
                commit -q -m base
            git -C "$mock_home/mock-project" checkout -q -b conflicting-branch
            printf 'other\n' > "$mock_home/mock-project/conflict.txt"
            git -C "$mock_home/mock-project" \
                -c user.name=Mock -c user.email=mock@example.invalid \
                -c commit.gpgSign=false -c core.hooksPath=/dev/null \
                commit -qam other
            git -C "$mock_home/mock-project" checkout -q mock-branch
            printf 'main\n' > "$mock_home/mock-project/conflict.txt"
            git -C "$mock_home/mock-project" \
                -c user.name=Mock -c user.email=mock@example.invalid \
                -c commit.gpgSign=false -c core.hooksPath=/dev/null \
                commit -qam main
            git -C "$mock_home/mock-project" \
                -c user.name=Mock -c user.email=mock@example.invalid \
                merge -q conflicting-branch >/dev/null 2>&1 || :
            ;;
    esac
    fixture_input=$(jq --arg cwd "$mock_home/mock-project" --arg model_name "$model_name" '
        .cwd = $cwd
        | if $model_name == "" then . else .model.display_name = $model_name end
    ' \
        "$repo_dir/tests/fixtures/$fixture")

    output=$(env \
        STATUSLINE_USAGE_STYLE="$style" \
        MOCK_FABLE_MISSING="$fable_missing" \
        MOCK_COLOR_SCENARIO="$color_scenario" \
        CLAUDE_CODE_OAUTH_TOKEN="mock-token" \
        CLAUDE_CONFIG_DIR="$mock_home/.claude" \
        XDG_RUNTIME_DIR="$mock_home/runtime" \
        HOME="$mock_home" \
        PATH="$mock_path" \
        COLUMNS="$columns" \
        bash "$script" <<< "$fixture_input")

    printf '%s' "$output"
)

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

assert_occurrences() {
    local output=$1 needle=$2 expected=$3 label=$4
    local rest=$output count=0
    while [[ "$rest" == *"$needle"* ]]; do
        rest=${rest#*"$needle"}
        count=$((count + 1))
    done
    if [ "$count" -ne "$expected" ]; then
        printf 'FAIL: %s\n  expected occurrences: %s\n  actual occurrences: %s\n  output: %s\n' \
            "$label" "$expected" "$count" "$output" >&2
        failures=$((failures + 1))
    fi
}

bar_output=$(run_statusline status-input.json)
assert_contains "$bar_output" '📁 mock-project › 🌿 mock-branch │ 🤖 Fable 5 · 🧠 med │ ⚡️ 50k/200k' 'semantic emoji layout'
assert_contains "$bar_output" '· 📊 5h:' 'one usage-group emoji before rate limits'
assert_occurrences "$bar_output" '📊' 1 'usage emoji appears exactly once'
assert_contains "$bar_output" '50k/200k (▓▓░░░░░░░░ 25%)' 'default context bar'
assert_contains "$bar_output" '5h: ▓▓░░░░░░░░ 20%' 'default 5h bar'
assert_contains "$bar_output" '7d: ▓▓▓▓▓░░░░░ 50%' 'default weekly bar'
assert_contains "$bar_output" 'Fable 5: ▓▓▓░░░░░░░ 30%' 'Fable weekly bar with built-in rates'
assert_not_contains "$bar_output" '▓ ▓' 'bar fill stays contiguous'
assert_not_contains "$bar_output" '░ ░' 'bar remainder stays contiguous'

git_mixed_output=$(STATUS_TEST_GIT_SCENARIO=mixed run_statusline status-input.json)
assert_contains "$git_mixed_output" '🌿 mock-branch [S1|W1]' 'Git reports staged and post-stage worktree file counts'

git_untracked_output=$(STATUS_TEST_GIT_SCENARIO=untracked run_statusline status-input.json)
assert_not_contains "$git_untracked_output" '[S' 'Git skips expensive untracked enumeration'
assert_not_contains "$git_untracked_output" '[W' 'untracked-only repo stays free of a worktree count'

git_detached_output=$(STATUS_TEST_GIT_SCENARIO=detached run_statusline status-input.json)
assert_contains "$git_detached_output" '🌿 detached' 'detached HEAD has an explicit label'

git_conflict_output=$(STATUS_TEST_GIT_SCENARIO=conflict run_statusline status-input.json)
assert_contains "$git_conflict_output" '🌿 mock-branch [C1]' 'Git reports conflict file count separately'

dot_output=$(STATUS_TEST_STYLE=dots run_statusline status-input.json)
assert_contains "$dot_output" '🤖 Fable 5 · 🧠 med' 'model and effort emoji in dots mode'
assert_contains "$dot_output" '⚡️ 50k/200k' 'context emoji in dots mode'
assert_occurrences "$dot_output" '📊' 1 'usage emoji remains singular in dots mode'
assert_contains "$dot_output" '50k/200k (● ● ○ ○ ○ ○ ○ ○ ○ ○ 25%)' 'spaced 10-dot context meter'
assert_contains "$dot_output" '5h: ● ● ○ ○ ○ ○ ○ ○ ○ ○ 20%' 'spaced 10-dot 5h meter'
assert_contains "$dot_output" '7d: ● ● ● ● ● ○ ○ ○ ○ ○ 50%' 'spaced 10-dot weekly meter'
assert_contains "$dot_output" 'Fable 5: ● ● ● ○ ○ ○ ○ ○ ○ ○ 30%' 'spaced 10-dot Fable weekly meter'

oauth_output=$(STATUS_TEST_STYLE=dots run_statusline status-input-oauth.json)
assert_contains "$oauth_output" '5h: ● ● ○ ○ ○ ○ ○ ○ ○ ○ 20%' 'OAuth 5h fallback'
assert_contains "$oauth_output" '7d: ● ● ● ● ● ○ ○ ○ ○ ○ 50%' 'OAuth weekly fallback'
assert_contains "$oauth_output" 'Fable 5: ● ● ● ○ ○ ○ ○ ○ ○ ○ 30%' 'OAuth Fable weekly entry'

missing_output=$(STATUS_TEST_STYLE=dots STATUS_TEST_FABLE_MISSING=1 run_statusline status-input.json)
assert_not_contains "$missing_output" 'Fable 5:' 'missing scoped Fable entry stays hidden'

invalid_style_output=$(STATUS_TEST_STYLE=invalid run_statusline status-input.json)
assert_contains "$invalid_style_output" '50k/200k (▓▓░░░░░░░░ 25%)' 'invalid style falls back to bar'

color_output=$(STATUS_TEST_STYLE=dots STATUS_TEST_COLOR_SCENARIO=1 run_statusline_raw status-input-colors.json)
assert_contains "$color_output" $'\033[38;2;255;170;80m● ● ● ● ●' '50% dot meter uses orange'
assert_contains "$color_output" $'\033[38;2;255;230;80m● ● ● ● ● ● ●' '70% dot meter uses yellow'
assert_contains "$color_output" $'\033[38;2;255;100;100m● ● ● ● ● ● ● ● ●' '90% dot meter uses red'

wrapped_dot_output=$(STATUS_TEST_STYLE=dots STATUS_TEST_COLUMNS=100 run_statusline status-input.json)
assert_contains "$wrapped_dot_output" $'\n└─ ⚡️ 50k/200k' 'spaced dots participate in dynamic width wrapping'
assert_not_contains "$dot_output" $'\n' 'wide dots output remains a single line'

boundary_output=$(STATUS_TEST_STYLE=dots run_statusline status-input-boundaries.json)
assert_contains "$boundary_output" '0/200k (○ ○ ○ ○ ○ ○ ○ ○ ○ ○ 0%)' '0% renders ten spaced empty dots'
assert_contains "$boundary_output" '5h: ● ● ● ● ● ● ● ● ● ● 100%' '100% renders ten spaced filled dots'
assert_contains "$boundary_output" '7d: ○ ○ ○ ○ ○ ○ ○ ○ ○ ○ 0%' '0% weekly renders ten spaced empty dots'

seven_day_only_output=$(STATUS_TEST_STYLE=dots run_statusline status-input-seven-day-only.json)
assert_contains "$seven_day_only_output" '📊 7d: ● ● ● ● ● ○ ○ ○ ○ ○ 50%' 'usage emoji prefixes first available rate-limit window'
assert_occurrences "$seven_day_only_output" '📊' 1 'seven-day-only output has one usage emoji'

one_column_short_output=$(STATUS_TEST_STYLE=dots STATUS_TEST_COLUMNS=245 run_statusline status-input.json)
assert_contains "$one_column_short_output" $'\n└─ ⚡️ 50k/200k' '246-column candidate wraps at 245 columns'
exact_fit_output=$(STATUS_TEST_STYLE=dots STATUS_TEST_COLUMNS=246 run_statusline status-input.json)
assert_not_contains "$exact_fit_output" $'\n' '246-column candidate stays single-line at exact width'

c_locale_exact_output=$(LC_ALL=C STATUS_TEST_STYLE=dots STATUS_TEST_COLUMNS=246 run_statusline status-input.json)
assert_not_contains "$c_locale_exact_output" $'\n' 'width measurement is locale-independent at exact width'

cjk_short_output=$(LC_ALL=C STATUS_TEST_STYLE=dots STATUS_TEST_COLUMNS=248 STATUS_TEST_MODEL_NAME='Fable 中文' run_statusline status-input.json)
assert_contains "$cjk_short_output" $'\n└─ ⚡️ 50k/200k' 'CJK candidate wraps one column below its display width'
cjk_exact_output=$(LC_ALL=C STATUS_TEST_STYLE=dots STATUS_TEST_COLUMNS=249 STATUS_TEST_MODEL_NAME='Fable 中文' run_statusline status-input.json)
assert_not_contains "$cjk_exact_output" $'\n' 'CJK candidate stays single-line at exact display width'

# Reuse one secured cache directory across redraws: the second invocation must
# use the clean snapshot, while TTL=0 must immediately expose the staged file.
cache_test_home=$(mktemp -d)
cleanup_cache_test() {
    [ ! -d "$cache_test_home" ] || rm -r -- "$cache_test_home"
}
trap cleanup_cache_test EXIT
mkdir -p "$cache_test_home/runtime" "$cache_test_home/mock-project"
chmod 700 "$cache_test_home" "$cache_test_home/runtime" "$cache_test_home/mock-project"
git -C "$cache_test_home/mock-project" init -q
git -C "$cache_test_home/mock-project" symbolic-ref HEAD refs/heads/cache-branch
git -C "$cache_test_home/mock-project" \
    -c user.name=Mock -c user.email=mock@example.invalid \
    -c commit.gpgSign=false -c core.hooksPath=/dev/null \
    commit -q --allow-empty -m init
cache_fixture=$(jq --arg cwd "$cache_test_home/mock-project" '.cwd = $cwd | .session_id = "cache-test"' \
    "$repo_dir/tests/fixtures/status-input.json")
cache_first=$(env STATUSLINE_GIT_CACHE_TTL=60 CLAUDE_CODE_OAUTH_TOKEN=mock-token \
    CLAUDE_CONFIG_DIR="$cache_test_home/.claude" XDG_RUNTIME_DIR="$cache_test_home/runtime" \
    HOME="$cache_test_home" PATH="$mock_path" COLUMNS=1000 \
    bash "$script" <<< "$cache_fixture" | strip_ansi)
printf 'staged\n' > "$cache_test_home/mock-project/staged.txt"
git -C "$cache_test_home/mock-project" add staged.txt
cache_hit=$(env STATUSLINE_GIT_CACHE_TTL=60 CLAUDE_CODE_OAUTH_TOKEN=mock-token \
    CLAUDE_CONFIG_DIR="$cache_test_home/.claude" XDG_RUNTIME_DIR="$cache_test_home/runtime" \
    HOME="$cache_test_home" PATH="$mock_path" COLUMNS=1000 \
    bash "$script" <<< "$cache_fixture" | strip_ansi)
cache_disabled=$(env STATUSLINE_GIT_CACHE_TTL=0 CLAUDE_CODE_OAUTH_TOKEN=mock-token \
    CLAUDE_CONFIG_DIR="$cache_test_home/.claude" XDG_RUNTIME_DIR="$cache_test_home/runtime" \
    HOME="$cache_test_home" PATH="$mock_path" COLUMNS=1000 \
    bash "$script" <<< "$cache_fixture" | strip_ansi)
assert_not_contains "$cache_first" '[S' 'initial clean Git snapshot has no staged count'
assert_not_contains "$cache_hit" '[S' 'rapid redraw reuses the short Git cache'
assert_contains "$cache_disabled" '🌿 cache-branch [S1]' 'Git cache can be disabled for immediate refresh'
cleanup_cache_test
trap - EXIT

if [ "$failures" -gt 0 ]; then
    printf '%d mock status-line assertion(s) failed\n' "$failures" >&2
    exit 1
fi

printf 'All mock status-line assertions passed\n'
