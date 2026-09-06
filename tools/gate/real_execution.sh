#!/bin/sh
# Real execution layer: run the built binary on a realistic input with an
# isolated home directory, three ways: (1) config file selects dots, (2)
# malformed config file is ignored with a stderr diagnostic, (3) no config file
# keeps the bar default. Network is avoided by seeding the update cache and
# pointing CLAUDE_CONFIG_DIR at an empty directory.
#
# Usage: sh tools/gate/real_execution.sh <artifact_dir>
set -eu

artifact_dir=$1
home="$artifact_dir/home"
rm -rf "$home"
mkdir -p "$home/.config/cc-statusline" "$home/.cache/StatusLine" "$home/.claude"
chmod 700 "$home" "$home/.cache" "$home/.cache/StatusLine" 2>/dev/null || true
printf '{"tag_name":"v0.0.0"}' > "$home/.cache/StatusLine/statusline-version-cache.json"

if [ -x target/debug/cc-statusline.exe ]; then
  bin=target/debug/cc-statusline.exe
elif [ -x target/debug/cc-statusline ]; then
  bin=target/debug/cc-statusline
else
  echo "FAIL: debug binary not found (run cargo test/build first)"
  exit 2
fi
fixture=tests/fixtures/status-input.json
[ -r "$fixture" ] || { echo "FAIL: fixture missing: $fixture"; exit 2; }

run_bin() {
  # Every statusline env var is removed so only the config file and the
  # explicit overrides below decide the outcome.
  env -u STATUSLINE_USAGE_STYLE -u STATUSLINE_GIT_CACHE_TTL -u CLAUDE_CODE_OAUTH_TOKEN \
      -u XDG_RUNTIME_DIR \
      HOME="$home" USERPROFILE="$home" XDG_CONFIG_HOME="$home/.config" \
      CLAUDE_CONFIG_DIR="$home/.claude" COLUMNS=1000 \
      "$bin" < "$fixture" > "$artifact_dir/real-stdout.txt" 2> "$artifact_dir/real-stderr.txt"
}

check() {
  label=$1; want_stdout=$2; forbid_stdout=$3; want_stderr=$4
  if run_bin; then rc=0; else rc=$?; fi
  [ "$rc" -eq 0 ] || { echo "FAIL[$label]: exit code $rc"; return 1; }
  [ -s "$artifact_dir/real-stdout.txt" ] || { echo "FAIL[$label]: empty stdout"; return 1; }
  grep -qF "$want_stdout" "$artifact_dir/real-stdout.txt" || { echo "FAIL[$label]: stdout lacks '$want_stdout'"; return 1; }
  if [ -n "$forbid_stdout" ] && grep -qF "$forbid_stdout" "$artifact_dir/real-stdout.txt"; then
    echo "FAIL[$label]: stdout must not contain '$forbid_stdout'"; return 1
  fi
  if [ -n "$want_stderr" ]; then
    grep -qF "$want_stderr" "$artifact_dir/real-stderr.txt" || { echo "FAIL[$label]: stderr lacks '$want_stderr'"; return 1; }
  else
    [ ! -s "$artifact_dir/real-stderr.txt" ] || { echo "FAIL[$label]: stderr not empty"; cat "$artifact_dir/real-stderr.txt"; return 1; }
  fi
  printf 'ok[%s]: %s\n' "$label" "$(cat "$artifact_dir/real-stdout.txt")"
}

printf 'usage_style = "dots"\n' > "$home/.config/cc-statusline/config.toml"
check dots-from-file "●" "▓" ""
printf 'usage_style = 1\n' > "$home/.config/cc-statusline/config.toml"
check malformed-file "Fable 5" "●" "config.toml"
rm "$home/.config/cc-statusline/config.toml"
check no-file "░" "●" ""
