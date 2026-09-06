#!/usr/bin/env python3
"""Emit the added PRODUCT lines of the change set as `path:line: text`.

Product lines are added lines in files under src/ that sit before the file's
first `#[cfg(test)]` marker (this crate keeps its unit-test module at the end
of each file). Lines in tests/ and inside test modules are excluded, so the
must-not scans that read this output judge product behaviour only.

Usage: added_lines.py --base <ref> [--diff-file <path>] [--source-root <dir>]
Exit 0 with output; 2 when the diff yields no product lines (fail closed) or
an input cannot be read.
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

HUNK = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@")


def added(diff_text: str) -> dict[str, list[int]]:
    result: dict[str, list[int]] = {}
    current = None
    line_no = 0
    for raw in diff_text.splitlines():
        if raw.startswith("+++ "):
            target = raw[4:].strip()
            target = target[2:] if target.startswith("b/") else target
            target = target.replace("\\", "/")
            current = target if target.startswith("src/") and target.endswith(".rs") else None
            if current:
                result.setdefault(current, [])
            continue
        if current is None:
            continue
        m = HUNK.match(raw)
        if m:
            line_no = int(m.group(1))
            continue
        if raw.startswith("+") and not raw.startswith("+++"):
            result[current].append(line_no)
            line_no += 1
        elif raw.startswith(" "):
            line_no += 1
    return result


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", required=True)
    ap.add_argument("--diff-file")
    ap.add_argument("--source-root", default=".")
    args = ap.parse_args()
    try:
        if args.diff_file:
            diff_text = Path(args.diff_file).read_text(encoding="utf-8")
        else:
            merge_base = subprocess.run(
                ["git", "merge-base", args.base, "HEAD"], check=True, capture_output=True, text=True
            ).stdout.strip()
            diff_text = subprocess.run(
                ["git", "diff", "-U0", merge_base, "HEAD", "--", "src"],
                check=True, capture_output=True, text=True, encoding="utf-8",
            ).stdout
        count = 0
        for rel, lines in sorted(added(diff_text).items()):
            source = (Path(args.source_root) / rel).read_text(encoding="utf-8").splitlines()
            marker = next((i + 1 for i, t in enumerate(source) if t.strip() == "#[cfg(test)]"), len(source) + 1)
            for line in lines:
                if line < marker and line - 1 < len(source):
                    print(f"{rel}:{line}: {source[line - 1]}")
                    count += 1
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"FAIL: cannot read inputs (fail closed): {error}", file=sys.stderr)
        return 2
    if count == 0:
        print("FAIL: no added product lines found (fail closed: nothing to scan)", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
