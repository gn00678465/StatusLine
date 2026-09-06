#!/bin/sh
# Prerequisite runs against the base ref, recorded separately from the gate:
#   baseline  — the full suite at <base>; which tests already fail there
#   RED replay — every commit in <base>..HEAD is checked out in a scratch
#                worktree and the suite is run. Commits whose subject starts
#                with `test` are RED commits and must fail; every other commit
#                must pass. This replays the RED -> GREEN ladder from git alone.
#
# Usage: sh tools/gate/replay_history.sh <base> <scope>
# Writes .gate/<scope>/baseline.md and .gate/<scope>/red.md plus per-commit logs.
set -eu
export PATH="$HOME/.cargo/bin:$PATH"
base=$1
scope=$2
repo_root=$(git rev-parse --show-toplevel)
# Absolute: run_suite cd's into the scratch worktree, so a relative log path
# would resolve there instead of under the repo's artifact root.
artifact_dir="$repo_root/.gate/$scope"
mkdir -p "$artifact_dir/replay"
wt="$repo_root/../$(basename "$repo_root")-replay"
export CARGO_TARGET_DIR="$artifact_dir/replay-target"

merge_base=$(git merge-base "$base" HEAD)
git worktree remove --force "$wt" 2>/dev/null || true
git worktree add --detach "$wt" "$merge_base" >/dev/null

run_suite() {
  # prints "ok" or "FAILED" plus the result lines; never aborts the script
  log=$1
  if (cd "$wt" && cargo test --all-targets > "$log" 2>&1); then echo ok; else echo FAILED; fi
}

failing_tests() {
  grep -E '^test .* \.\.\. FAILED$|^    [a-z_:]+$' "$1" | sed 's/^test //; s/ \.\.\. FAILED$//; s/^ *//' | sort -u | tr '\n' ' '
}
result_lines() { grep -E '^test result:|^error(\[E[0-9]+\])?:' "$1" | head -n 6 | tr '\n' ' '; }

status=$(run_suite "$artifact_dir/replay/baseline.txt")
{
  echo "# Baseline at base ($merge_base)"
  echo
  echo "- command: \`cargo test --all-targets\` in a detached worktree at \`$merge_base\`"
  echo "- status: $status"
  echo "- result: $(result_lines "$artifact_dir/replay/baseline.txt")"
  if [ "$status" = "FAILED" ]; then echo "- failing tests: $(failing_tests "$artifact_dir/replay/baseline.txt")"; else echo "- failing tests: none — base was green"; fi
} > "$artifact_dir/baseline.md"
cat "$artifact_dir/baseline.md"

{
  echo "# RED replay: every commit in $merge_base..HEAD"
  echo
  echo "| Commit | Subject | Expected | Observed | Failing tests / first error |"
  echo "|---|---|---|---|---|"
} > "$artifact_dir/red.md"
mismatch=0
for sha in $(git rev-list --reverse "$merge_base..HEAD"); do
  subject=$(git log -1 --format=%s "$sha")
  case "$subject" in test*) expected=fail ;; *) expected=pass ;; esac
  (cd "$wt" && git checkout -q --detach "$sha")
  status=$(run_suite "$artifact_dir/replay/$sha.txt")
  case "$status" in ok) observed=pass ;; *) observed=fail ;; esac
  detail=$(failing_tests "$artifact_dir/replay/$sha.txt")
  [ -n "$detail" ] || detail=$(result_lines "$artifact_dir/replay/$sha.txt")
  mark=""
  [ "$expected" = "$observed" ] || { mark=" **MISMATCH**"; mismatch=1; }
  short=$(git rev-parse --short "$sha")
  printf '| %s | %s | %s | %s%s | %s |\n' "$short" "$(printf '%s' "$subject" | sed 's/|/\\|/g')" "$expected" "$observed" "$mark" "$(printf '%s' "$detail" | sed 's/|/\\|/g')" >> "$artifact_dir/red.md"
done
cat "$artifact_dir/red.md"
git worktree remove --force "$wt"
if [ "$mismatch" -ne 0 ]; then
  echo "FAIL: at least one commit did not behave as its subject claims (RED must fail, GREEN must pass)"
  exit 1
fi
echo "replay ok: every test: commit failed and every other commit passed"
