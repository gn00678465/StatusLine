#!/bin/sh
# Emit the source state (HEAD commit SHA) only when the tree is clean.
# Refuses: dirty tracked files, staged changes, non-ignored untracked files
# outside the enumerated whitelist, and truncated (shallow) history.
#
# Whitelist admission test (checked per prefix, never asserted in prose): a
# prefix is excused only if `git ls-files -- <prefix>` returns nothing AND the
# prefix does not appear in the change set's diff against GATE_BASE. A prefix
# that holds even one tracked file is part of the subject and is refused even
# though listed.
#
# Usage: sh tools/gate/source_state.sh [--base <ref>] [--whitelist "p1 p2"]
set -eu

base=main
whitelist=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --base) base=$2; shift 2 ;;
    --whitelist) whitelist=$2; shift 2 ;;
    *) echo "FAIL: unknown argument $1" >&2; exit 2 ;;
  esac
done

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "FAIL: not a git work tree" >&2
  exit 2
fi
if [ "$(git rev-parse --is-shallow-repository)" != "false" ]; then
  echo "FAIL: shallow repository; history is truncated" >&2
  exit 1
fi
if ! git cat-file -e "${base}^{commit}"; then
  echo "FAIL: base '$base' does not resolve to a local commit" >&2
  exit 1
fi

diff_paths=$(git diff --name-only "$(git merge-base "$base" HEAD)" HEAD)
admitted=""
for prefix in $whitelist; do
  if [ -n "$(git ls-files -- "$prefix")" ]; then
    echo "FAIL: whitelist prefix '$prefix' holds tracked files; refused" >&2
    exit 1
  fi
  if printf '%s\n' "$diff_paths" | grep -q "^$prefix"; then
    echo "FAIL: whitelist prefix '$prefix' appears in the change set; refused" >&2
    exit 1
  fi
  admitted="$admitted $prefix"
done

status=$(git status --porcelain --untracked-files=all)
violations=""
old_ifs=$IFS
IFS='
'
for line in $status; do
  [ -z "$line" ] && continue
  code=$(printf '%s' "$line" | cut -c1-2)
  path=$(printf '%s' "$line" | cut -c4-)
  excused=0
  if [ "$code" = "??" ]; then
    IFS=$old_ifs
    for prefix in $admitted; do
      case "$path" in "$prefix"*) excused=1 ;; esac
    done
    IFS='
'
  fi
  if [ "$excused" -eq 0 ]; then
    violations="$violations
$line"
  fi
done
IFS=$old_ifs

if [ -n "$violations" ]; then
  echo "FAIL: working tree is not clean; refusing to emit a source state:$violations" >&2
  exit 1
fi

sha=$(git rev-parse HEAD)
echo "source_state=$sha"
echo "source_state_exclusions=${admitted:- none}"
