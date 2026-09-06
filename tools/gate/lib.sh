#!/bin/sh
# Fail-closed layer accounting shared by tools/gate.sh and its self-test.
# Every layer is recorded only after its command succeeds; finish_gate refuses
# to print success while any expected layer is missing.

GATE_COMPLETED_LAYERS=""

run_layer() {
  if [ "$#" -lt 2 ]; then
    echo "FAIL: run_layer requires a layer name and command" >&2
    return 2
  fi
  layer=$1
  shift
  case " $GATE_EXPECTED_LAYERS " in
    *" $layer "*) ;;
    *)
      echo "FAIL: unknown layer '$layer'" >&2
      return 2
      ;;
  esac
  case " $GATE_COMPLETED_LAYERS " in
    *" $layer "*)
      echo "FAIL: duplicate layer '$layer'" >&2
      return 2
      ;;
  esac
  printf '=== %s ===\n' "$layer"
  if "$@"; then
    GATE_COMPLETED_LAYERS="$GATE_COMPLETED_LAYERS $layer"
    return 0
  else
    rc=$?
    printf "FAIL: layer '%s' failed (rc=%s)\n" "$layer" "$rc" >&2
    return "$rc"
  fi
}

finish_gate() {
  missing=0
  for layer in $GATE_EXPECTED_LAYERS; do
    case " $GATE_COMPLETED_LAYERS " in
      *" $layer "*) ;;
      *)
        echo "FAIL: missing layer '$layer'" >&2
        missing=1
        ;;
    esac
  done
  if [ "$missing" -ne 0 ]; then
    return 1
  fi
  echo "=== gate: all layers green ==="
}

# Must-find-nothing grep. grep rc 1 (no match) is the only pass; rc 0 means the
# forbidden pattern is present (return 1); rc >= 2 means the scan itself broke
# (return 2). An empty path list is refused with rc 2: a scan that inspected
# nothing must never share an exit code with a scan that found nothing.
must_not_match() {
  pattern=$1
  shift
  if [ "$#" -eq 0 ]; then
    echo "FAIL: no paths given to scan (fail closed): $pattern"
    return 2
  fi
  for path in "$@"; do
    if [ ! -r "$path" ]; then
      echo "FAIL: unreadable scan path (fail closed): $path"
      return 2
    fi
  done
  if grep -nE "$pattern" "$@"; then
    echo "FAIL: forbidden pattern present: $pattern"
    return 1
  elif [ $? -ne 1 ]; then
    echo "FAIL: scan itself broke (fail closed): $pattern"
    return 2
  fi
  return 0
}

# Pinned tool versions. Each `name<TAB>expected` line in tools/gate/versions.txt
# is compared with the first line of `<command> --version`.
check_versions() {
  versions_file=$1
  if [ ! -r "$versions_file" ]; then
    echo "FAIL: versions file missing: $versions_file"
    return 2
  fi
  rc=0
  # CR is stripped into a temp copy (not a pipe: a piped `while` runs in a
  # subshell and would lose rc, failing open) so a checkout under
  # core.autocrlf=true does not turn every command into `<cmd> --version\r`.
  stripped=$(mktemp)
  tr -d '\r' < "$versions_file" > "$stripped"
  while IFS='	' read -r name expected command; do
    case "$name" in ''|'#'*) continue ;; esac
    actual=$($command 2>&1 | head -n 1 | tr -d '\r')
    if [ "$actual" != "$expected" ]; then
      echo "FAIL: version drift for $name: expected '$expected', got '$actual'"
      rc=1
    else
      echo "ok: $name = $actual"
    fi
  done < "$stripped"
  rm -f "$stripped"
  return "$rc"
}
