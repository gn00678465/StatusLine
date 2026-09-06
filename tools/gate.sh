#!/bin/sh
# Verification gate entry point for cc-statusline. Runs every layer in order
# and fails on the first broken one. Rerun the whole evidence with:
#
#   sh tools/gate.sh [--base <ref>] [--scope <name>] [--whitelist "<prefix> ..."]
#
# Run it in a clean checkout of the commit under test (the source-state layer
# refuses a dirty tree). Artifacts land under .gate/<scope>/ and are deleted
# at the start of every run; the cargo target/ directory is kept because it is
# a build cache, not a report.
#
# Layer functions run inside run_layer's `if`, where `set -e` is suspended, so
# every step inside a layer is chained explicitly with `|| return $?`.
set -eu
export PATH="$HOME/.cargo/bin:$PATH"

base=main
scope=config-file
whitelist=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --base) base=$2; shift 2 ;;
    --scope) scope=$2; shift 2 ;;
    --whitelist) whitelist=$2; shift 2 ;;
    *) echo "FAIL: unknown argument $1" >&2; exit 2 ;;
  esac
done

artifact_dir=".gate/$scope"
rm -rf "$artifact_dir"
mkdir -p "$artifact_dir"

. tools/gate/lib.sh
GATE_EXPECTED_LAYERS="versions selftest source-state-before tests types lint-format suite-health changed-line-coverage mutation mutation-kill-sample real-execution must-not-scans supply-chain source-state-after"

merge_base=$(git merge-base "$base" HEAD)
git diff "$merge_base" HEAD > "$artifact_dir/change.diff"
# Added lines of the change set (subject files only). grep rc 1 = no added
# lines; the file is then empty and must_not_match refuses it with rc 2.
if git diff -U0 "$merge_base" HEAD -- src tests Cargo.toml | grep -E '^\+[^+]' | sed 's/^+//' > "$artifact_dir/added-lines.txt"; then :; fi

# capture <logfile> <cmd...>: run cmd with all output to logfile, echo the log,
# and return cmd's own exit status (a pipe to tee would return tee's).
capture() {
  log=$1
  shift
  if "$@" > "$log" 2>&1; then rc=0; else rc=$?; fi
  cat "$log"
  return "$rc"
}

layer_versions() { check_versions tools/gate/versions.txt; }
layer_selftest() { sh tools/gate/selftest.sh "$artifact_dir" "$base"; }
layer_source_state_before() {
  sh tools/gate/source_state.sh --base "$base" --whitelist "$whitelist" > "$artifact_dir/source-state-before.txt" || return $?
  cat "$artifact_dir/source-state-before.txt"
}
layer_tests() {
  capture "$artifact_dir/tests.txt" cargo test --all-targets || return $?
  grep -qE '^test result: ok' "$artifact_dir/tests.txt" || { echo "FAIL: no passing test result line"; return 1; }
  if grep -qE '^test result: FAILED' "$artifact_dir/tests.txt"; then echo "FAIL: failing test result line"; return 1; fi
}
layer_types() { capture "$artifact_dir/types.txt" cargo check --all-targets; }
layer_lint_format() {
  capture "$artifact_dir/fmt.txt" cargo fmt --check || return $?
  capture "$artifact_dir/clippy.txt" cargo clippy --all-targets -- -D warnings
}
layer_suite_health() {
  # SUBSTITUTED: `cargo test -- --shuffle` needs nightly. Three repeats (two
  # parallel, one single-threaded) cannot detect whole-suite order dependence.
  capture "$artifact_dir/suite-health-1.txt" cargo test --all-targets > /dev/null || { cat "$artifact_dir/suite-health-1.txt"; return 1; }
  capture "$artifact_dir/suite-health-2.txt" cargo test --all-targets > /dev/null || { cat "$artifact_dir/suite-health-2.txt"; return 1; }
  capture "$artifact_dir/suite-health-3.txt" cargo test --all-targets -- --test-threads=1 > /dev/null || { cat "$artifact_dir/suite-health-3.txt"; return 1; }
  grep -hE '^test result' "$artifact_dir"/suite-health-*.txt
}
layer_changed_line_coverage() {
  capture "$artifact_dir/llvm-cov-run.txt" cargo llvm-cov --all-targets --no-report > /dev/null || { cat "$artifact_dir/llvm-cov-run.txt"; return 1; }
  cargo llvm-cov report --lcov --output-path "$artifact_dir/coverage.lcov" || return $?
  cargo llvm-cov report --json --output-path "$artifact_dir/coverage.json" || return $?
  python3 tools/gate/changed_lines.py --base "$base" --lcov "$artifact_dir/coverage.lcov" --json "$artifact_dir/coverage.json" --report "$artifact_dir/changed-lines.md"
}
layer_mutation() {
  # Mutants restricted to the change set. --jobs 1: one build directory at a
  # time, so no mutant can test a binary another mutant built.
  if capture "$artifact_dir/mutation.txt" cargo mutants --in-diff "$artifact_dir/change.diff" --jobs 1 -o "$artifact_dir/mutants" > /dev/null; then rc=0; else rc=$?; fi
  tail -n 15 "$artifact_dir/mutation.txt"
  python3 tools/gate/mutants_summary.py "$artifact_dir/mutants/mutants.out" "$rc"
}
layer_mutation_kill_sample() {
  # Re-run every mutant of up to two functions that had caught mutants, in a
  # separate output dir, and require missed == 0 and caught >= 1 again. Small
  # sample; its power is stated in EVIDENCE rather than implied.
  python3 tools/gate/mutants_summary.py --sample-functions 2 "$artifact_dir/mutants/mutants.out" 0 > "$artifact_dir/kill-sample-functions.txt" || return $?
  n=0
  while IFS= read -r fn; do
    [ -z "$fn" ] && continue
    n=$((n + 1))
    printf 'kill sample %s: function %s\n' "$n" "$fn"
    if capture "$artifact_dir/kill-sample-$n.txt" cargo mutants --in-diff "$artifact_dir/change.diff" --jobs 1 -F "$fn" -o "$artifact_dir/kill-sample-$n" > /dev/null; then rc=0; else rc=$?; fi
    grep -E 'caught|missed|unviable|timeout' "$artifact_dir/kill-sample-$n.txt" | tail -n 5
    python3 tools/gate/mutants_summary.py "$artifact_dir/kill-sample-$n/mutants.out" "$rc" || return $?
  done < "$artifact_dir/kill-sample-functions.txt"
  [ "$n" -ge 1 ] || { echo "FAIL: no functions sampled (fail closed)"; return 2; }
}
layer_real_execution() { capture "$artifact_dir/real-execution.txt" sh tools/gate/real_execution.sh "$artifact_dir"; }
layer_must_not_scans() {
  added="$artifact_dir/added-lines.txt"
  # stdout purity: no new print!/println! anywhere in the change
  must_not_match '\bprintln!\(|\bprint!\(' "$added" || return $?
  # panic paths in new code (also clippy-denied; kept as a spec row)
  must_not_match '\.unwrap\(\)|\.expect\(|panic!\(|unimplemented!\(|todo!\(' "$added" || return $?
  # config file is read-only: no filesystem writes introduced
  must_not_match 'fs::(write|create_dir|create_dir_all|remove_file|remove_dir|rename)|File::create|OpenOptions::new' "$added" || return $?
  # no new subprocess / network in the change
  must_not_match 'Command::new|ureq::' "$added" || return $?
  # existing tests, fixtures, package version, lockfile additions
  python3 tools/gate/unchanged_tests.py --base "$base"
}
layer_supply_chain() {
  # Dependency diff (crate names) and license of every new crate. Vulnerability
  # audit is UNAVAILABLE: cargo-audit is not installed and not authorized.
  git show "$merge_base:Cargo.lock" | grep -E '^name = ' | sort -u > "$artifact_dir/lock-base.txt" || return $?
  grep -E '^name = ' Cargo.lock | sort -u > "$artifact_dir/lock-head.txt" || return $?
  comm -13 "$artifact_dir/lock-base.txt" "$artifact_dir/lock-head.txt" > "$artifact_dir/lock-added.txt" || return $?
  comm -23 "$artifact_dir/lock-base.txt" "$artifact_dir/lock-head.txt" > "$artifact_dir/lock-removed.txt" || return $?
  sed 's/^/added crate: /' "$artifact_dir/lock-added.txt"
  sed 's/^/removed crate: /' "$artifact_dir/lock-removed.txt"
  cargo metadata --format-version 1 --locked > "$artifact_dir/metadata.json" || return $?
  python3 tools/gate/dep_licenses.py "$artifact_dir/metadata.json" "$artifact_dir/lock-added.txt" || return $?
  # secrets in the diff
  must_not_match 'AKIA[0-9A-Z]{16}|-----BEGIN [A-Z ]*PRIVATE KEY|sk-ant-[A-Za-z0-9_-]{8}|ghp_[A-Za-z0-9]{20}' "$artifact_dir/change.diff" || return $?
  # declared-capability diff (informational): `use` items added by the change
  if grep -E '^\s*(pub(\([a-z]+\))? )?use ' "$artifact_dir/added-lines.txt" | sort -u > "$artifact_dir/capability-diff.txt"; then :; fi
  sed 's/^/capability (new use): /' "$artifact_dir/capability-diff.txt"
}
layer_source_state_after() {
  sh tools/gate/source_state.sh --base "$base" --whitelist "$whitelist" > "$artifact_dir/source-state-after.txt" || return $?
  cat "$artifact_dir/source-state-after.txt"
  cmp "$artifact_dir/source-state-before.txt" "$artifact_dir/source-state-after.txt"
}

run_layer versions layer_versions
run_layer selftest layer_selftest
run_layer source-state-before layer_source_state_before
run_layer tests layer_tests
run_layer types layer_types
run_layer lint-format layer_lint_format
run_layer suite-health layer_suite_health
run_layer changed-line-coverage layer_changed_line_coverage
run_layer mutation layer_mutation
run_layer mutation-kill-sample layer_mutation_kill_sample
run_layer real-execution layer_real_execution
run_layer must-not-scans layer_must_not_scans
run_layer supply-chain layer_supply_chain
run_layer source-state-after layer_source_state_after
finish_gate
