#!/usr/bin/env python3
"""Must-NOT checks that need file-content comparison against the base ref.

Enforces, from specs/config-file/SPEC.md "Must NOT":
  1. src/main.rs test module (from the first `#[cfg(test)]` to EOF) is
     byte-identical between base and HEAD (inline snapshot untouched).
  2. src/config.rs existing test fn
     `parses_usage_style_ttl_and_columns_with_clamps_and_fallbacks` unchanged.
  3. tests/cc_statusline.rs existing fns `renders_fallback_for_non_json_input`
     and `renders_fallback_for_empty_input` unchanged.
  4. tests/fixtures/ has no diff.
  5. Cargo.toml `version = "..."` (package) unchanged.
  6. Cargo.lock package names added by the change are exactly {basic-toml}.

`--override <path>=<file>` substitutes HEAD content for one path (used by the
self-test to prove the checker can fail). Exit 0 pass, 1 violation, 2 checker
broke (e.g. a named function could not be located: fail closed).
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ALLOWED_NEW_CRATES = {"basic-toml"}


def git_show(ref: str, path: str) -> str:
    return subprocess.run(
        ["git", "show", f"{ref}:{path}"], check=True, capture_output=True, text=True, encoding="utf-8"
    ).stdout


def extract_fn(source: str, name: str) -> str:
    match = re.search(rf"^\s*fn {re.escape(name)}\s*\(", source, re.M)
    if not match:
        raise LookupError(f"function {name} not found")
    start = match.start()
    depth = 0
    i = source.index("{", start)
    for j in range(i, len(source)):
        if source[j] == "{":
            depth += 1
        elif source[j] == "}":
            depth -= 1
            if depth == 0:
                return source[start:j + 1]
    raise LookupError(f"function {name} has unbalanced braces")


def test_module(source: str) -> str:
    idx = source.find("#[cfg(test)]")
    if idx < 0:
        raise LookupError("no #[cfg(test)] module")
    return source[idx:]


def lock_names(text: str) -> set[str]:
    return set(re.findall(r'^name = "([^"]+)"', text, re.M))


def package_version(text: str) -> str:
    m = re.search(r'^\[package\](?:.*\n)*?version = "([^"]+)"', text, re.M)
    if not m:
        raise LookupError("package version not found in Cargo.toml")
    return m.group(1)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", required=True)
    ap.add_argument("--override", action="append", default=[], help="path=file to use as HEAD content")
    args = ap.parse_args()
    overrides = {}
    for item in args.override:
        path, _, file = item.partition("=")
        overrides[path] = Path(file).read_text(encoding="utf-8")

    def head(path: str) -> str:
        if path in overrides:
            return overrides[path]
        return git_show("HEAD", path)

    base = subprocess.run(
        ["git", "merge-base", args.base, "HEAD"], check=True, capture_output=True, text=True
    ).stdout.strip()

    violations: list[str] = []
    try:
        if test_module(git_show(base, "src/main.rs")) != test_module(head("src/main.rs")):
            violations.append("src/main.rs: test module differs from base (inline snapshot or tests changed)")
        name = "parses_usage_style_ttl_and_columns_with_clamps_and_fallbacks"
        if extract_fn(git_show(base, "src/config.rs"), name) != extract_fn(head("src/config.rs"), name):
            violations.append(f"src/config.rs: existing test {name} changed")
        for name in ("renders_fallback_for_non_json_input", "renders_fallback_for_empty_input"):
            if extract_fn(git_show(base, "tests/cc_statusline.rs"), name) != extract_fn(
                head("tests/cc_statusline.rs"), name
            ):
                violations.append(f"tests/cc_statusline.rs: existing test {name} changed")
        fixture_diff = subprocess.run(
            ["git", "diff", "--name-only", base, "HEAD", "--", "tests/fixtures"],
            check=True, capture_output=True, text=True,
        ).stdout.strip()
        if fixture_diff:
            violations.append(f"tests/fixtures changed: {fixture_diff}")
        if package_version(git_show(base, "Cargo.toml")) != package_version(head("Cargo.toml")):
            violations.append("Cargo.toml package version changed")
        new_crates = lock_names(head("Cargo.lock")) - lock_names(git_show(base, "Cargo.lock"))
        if new_crates - ALLOWED_NEW_CRATES:
            violations.append(f"Cargo.lock adds crates beyond {sorted(ALLOWED_NEW_CRATES)}: {sorted(new_crates)}")
        print(f"ok: Cargo.lock new crates = {sorted(new_crates)}")
    except (LookupError, subprocess.CalledProcessError) as error:
        print(f"FAIL: checker could not run (fail closed): {error}")
        return 2

    if violations:
        for v in violations:
            print(f"FAIL: {v}")
        return 1
    print("ok: existing tests, fixtures, package version, and lockfile constraints hold")
    return 0


if __name__ == "__main__":
    sys.exit(main())
