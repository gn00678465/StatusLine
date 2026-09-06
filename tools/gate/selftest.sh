#!/bin/sh
# Negative controls for the gate's home-grown checks. Runs before the real
# layers so a broken harness fails before it can print anything green.
# Every control both proves the check can fail AND proves it can pass.
#
# Usage: sh tools/gate/selftest.sh <artifact_dir> <base>
set -eu
artifact_dir=$1
base=$2
tmp="$artifact_dir/selftest"
rm -rf "$tmp"
mkdir -p "$tmp"
# The source-state controls plant throwaway files in the tree; remove them on
# any exit so an aborted self-test cannot leave the tree dirty.
trap 'rm -rf zz_selftest_wl src/zz_selftest_stray.rs' EXIT

expect_rc() {
  want=$1; label=$2; shift 2
  if "$@" >"$tmp/out.txt" 2>&1; then got=0; else got=$?; fi
  if [ "$got" -ne "$want" ]; then
    echo "SELFTEST FAIL: $label: expected rc $want, got $got"
    cat "$tmp/out.txt"
    return 1
  fi
  echo "selftest ok: $label (rc $got)"
}

# --- orchestration: run_layer / finish_gate -------------------------------
. tools/gate/lib.sh
orch() {
  ( GATE_EXPECTED_LAYERS="a b"; GATE_COMPLETED_LAYERS=""
    run_layer a true && finish_gate )
}
orch_missing() {
  ( GATE_EXPECTED_LAYERS="a b"; GATE_COMPLETED_LAYERS=""
    run_layer a true && run_layer b true && finish_gate )
}
orch_fail_rc() { ( GATE_EXPECTED_LAYERS="a"; GATE_COMPLETED_LAYERS=""; run_layer a sh -c 'exit 7' ); }
orch_unknown() { ( GATE_EXPECTED_LAYERS="a"; GATE_COMPLETED_LAYERS=""; run_layer zzz true ); }
orch_dup() { ( GATE_EXPECTED_LAYERS="a"; GATE_COMPLETED_LAYERS=""; run_layer a true && run_layer a true ); }
expect_rc 1 "orchestration: missing layer is named, no green" orch
expect_rc 0 "orchestration: complete manifest reaches green (positive control)" orch_missing
expect_rc 7 "orchestration: failing command preserves rc" orch_fail_rc
expect_rc 2 "orchestration: unknown layer is rc 2" orch_unknown
expect_rc 2 "orchestration: duplicate layer is rc 2" orch_dup

# --- must_not_match ---------------------------------------------------------
printf 'clean line\n' > "$tmp/clean.txt"
printf 'has println!("x")\n' > "$tmp/dirty.txt"
expect_rc 0 "must_not_match: clean file passes" must_not_match 'println!' "$tmp/clean.txt"
expect_rc 1 "must_not_match: forbidden pattern fails" must_not_match 'println!' "$tmp/dirty.txt"
expect_rc 2 "must_not_match: nonexistent path is rc 2" must_not_match 'println!' "$tmp/nope.txt"
expect_rc 2 "must_not_match: empty path list is rc 2" must_not_match 'println!'

# --- check_versions ---------------------------------------------------------
printf 'python3\tPython 0.0.0\tpython3 --version\n' > "$tmp/bad-versions.txt"
expect_rc 1 "check_versions: drift fails" check_versions "$tmp/bad-versions.txt"
expect_rc 2 "check_versions: missing file is rc 2" check_versions "$tmp/absent.txt"

# --- changed_lines.py --------------------------------------------------------
# Synthetic diff against a real file so line text can be read; a fake LCOV with
# a zero-hit changed line must fail; a missing DA on an in-span line must fail
# as unmapped; a fully covered set must pass; an empty diff must be rc 2.
src=src/config.rs
first_exec=$(grep -nE '^\s*(pub\(crate\) )?fn ' "$src" | head -n 1 | cut -d: -f1)
[ -n "$first_exec" ] || { echo "SELFTEST FAIL: no fn line in $src"; exit 1; }
body=$((first_exec + 1))
printf 'diff --git a/%s b/%s\n--- a/%s\n+++ b/%s\n@@ -0,0 +%s,2 @@\n+x\n+y\n' "$src" "$src" "$src" "$src" "$first_exec" > "$tmp/diff.txt"
printf 'SF:%s\nDA:%s,1\nDA:%s,1\nend_of_record\n' "$src" "$first_exec" "$body" > "$tmp/cov-good.lcov"
printf 'SF:%s\nDA:%s,1\nDA:%s,0\nend_of_record\n' "$src" "$first_exec" "$body" > "$tmp/cov-zero.lcov"
printf 'SF:%s\nDA:%s,1\nend_of_record\n' "$src" "$first_exec" > "$tmp/cov-unmapped.lcov"
printf '{"data":[{"functions":[{"filenames":["%s"],"regions":[[%s,1,%s,1,1,0,0,0]]}]}]}\n' "$src" "$first_exec" "$((body + 5))" > "$tmp/cov.json"
: > "$tmp/empty-diff.txt"
cl="python3 tools/gate/changed_lines.py --base $base --json $tmp/cov.json --diff-file"
expect_rc 0 "changed_lines: covered lines pass (positive control)" $cl "$tmp/diff.txt" --lcov "$tmp/cov-good.lcov"
expect_rc 1 "changed_lines: zero-hit changed line fails" $cl "$tmp/diff.txt" --lcov "$tmp/cov-zero.lcov"
expect_rc 1 "changed_lines: in-span line without DA fails as unmapped" $cl "$tmp/diff.txt" --lcov "$tmp/cov-unmapped.lcov"
expect_rc 2 "changed_lines: empty diff is rc 2 (inspected nothing)" $cl "$tmp/empty-diff.txt" --lcov "$tmp/cov-good.lcov"
expect_rc 2 "changed_lines: unreadable LCOV is rc 2" $cl "$tmp/diff.txt" --lcov "$tmp/absent.lcov"
# allowlist: an entry matching the zero-hit line turns the miss into an
# accepted line (pass); an entry matching nothing is stale and is rc 2.
body_text=$(sed -n "${body}p" "$src" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')
printf '%s|%s|selftest control\n' "$src" "$body_text" > "$tmp/allow-match.txt"
printf '%s|this line does not exist anywhere|stale\n' "$src" > "$tmp/allow-stale.txt"
expect_rc 0 "changed_lines: allowlisted uncovered line is accepted" $cl "$tmp/diff.txt" --lcov "$tmp/cov-zero.lcov" --allow "$tmp/allow-match.txt"
expect_rc 2 "changed_lines: stale allowlist entry is rc 2" $cl "$tmp/diff.txt" --lcov "$tmp/cov-good.lcov" --allow "$tmp/allow-stale.txt"

# --- added_lines.py (product-line extractor) ---------------------------------
# A line added before the file's #[cfg(test)] marker is product; one added
# after it is not; a diff with only test-module lines is rc 2.
marker=$(grep -n '^#\[cfg(test)\]$' "$src" | head -n 1 | cut -d: -f1)
[ -n "$marker" ] || { echo "SELFTEST FAIL: no #[cfg(test)] marker in $src"; exit 1; }
after=$((marker + 2))
printf 'diff --git a/%s b/%s\n--- a/%s\n+++ b/%s\n@@ -0,0 +%s,1 @@\n+x\n@@ -0,0 +%s,1 @@\n+y\n' "$src" "$src" "$src" "$src" "$first_exec" "$after" > "$tmp/diff-mixed.txt"
printf 'diff --git a/%s b/%s\n--- a/%s\n+++ b/%s\n@@ -0,0 +%s,1 @@\n+y\n' "$src" "$src" "$src" "$src" "$after" > "$tmp/diff-testonly.txt"
if ! python3 tools/gate/added_lines.py --base "$base" --diff-file "$tmp/diff-mixed.txt" > "$tmp/product.txt"; then echo "SELFTEST FAIL: added_lines mixed diff"; exit 1; fi
grep -q "^$src:$first_exec:" "$tmp/product.txt" || { echo "SELFTEST FAIL: added_lines dropped a product line"; exit 1; }
if grep -q "^$src:$after:" "$tmp/product.txt"; then echo "SELFTEST FAIL: added_lines kept a test-module line"; exit 1; fi
echo "selftest ok: added_lines: product line kept, test-module line excluded"
expect_rc 2 "added_lines: test-module-only diff is rc 2" python3 tools/gate/added_lines.py --base "$base" --diff-file "$tmp/diff-testonly.txt"

# --- unchanged_tests.py ------------------------------------------------------
git show HEAD:src/main.rs | sed 's/assert_eq!(app.render_input(""), "Claude");/assert_eq!(app.render_input(""), "Claud");/' > "$tmp/main-mutated.rs"
git show HEAD:src/main.rs > "$tmp/main-orig.rs"
if cmp -s "$tmp/main-mutated.rs" "$tmp/main-orig.rs"; then
  echo "SELFTEST FAIL: mutation of src/main.rs test module did not apply"; exit 1
fi
expect_rc 1 "unchanged_tests: mutated inline test fails" python3 tools/gate/unchanged_tests.py --base "$base" --override "src/main.rs=$tmp/main-mutated.rs"
printf '[package]\nname = "cc-statusline"\nversion = "9.9.9"\n' > "$tmp/cargo-bumped.toml"
expect_rc 1 "unchanged_tests: version bump fails" python3 tools/gate/unchanged_tests.py --base "$base" --override "Cargo.toml=$tmp/cargo-bumped.toml"
printf 'fn nothing() {}\n' > "$tmp/no-tests.rs"
expect_rc 2 "unchanged_tests: missing test module is rc 2" python3 tools/gate/unchanged_tests.py --base "$base" --override "src/main.rs=$tmp/no-tests.rs"

# --- source_state.sh ---------------------------------------------------------
stray=src/zz_selftest_stray.rs
: > "$stray"
expect_rc 1 "source_state: untracked product file is refused (not listed)" sh tools/gate/source_state.sh --base "$base" --whitelist "zz_selftest_wl/"
expect_rc 1 "source_state: untracked product file is refused even when its dir is listed" sh tools/gate/source_state.sh --base "$base" --whitelist "src/"
rm -f "$stray"
expect_rc 1 "source_state: whitelist prefix holding tracked files is refused" sh tools/gate/source_state.sh --base "$base" --whitelist "src/"
mkdir -p zz_selftest_wl
: > zz_selftest_wl/scratch.txt
expect_rc 0 "source_state: only-whitelisted untracked content still emits a state (positive control)" sh tools/gate/source_state.sh --base "$base" --whitelist "zz_selftest_wl/"
rm -rf zz_selftest_wl

echo "selftest: all negative controls behaved"
