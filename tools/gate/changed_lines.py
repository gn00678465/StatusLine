#!/usr/bin/env python3
"""Changed-line coverage gate (home-grown; fails closed).

Splits the added lines of the change set into three sets and exits nonzero
unless every executable added line is covered and no executable added line is
unmapped:

  set 1  executable and instrumented: the line has a DA record in the LCOV
  set 2  not executable: plainly non-executable by the textual rule below, OR
         outside every function's coverage-region span in the llvm-cov JSON
  set 3  executable but no coverage mapping: inside a function span, not
         plainly non-executable, and no DA record. Never folded into set 2.

Textual rule for "plainly non-executable" (anything else is ambiguous and is
resolved into set 3, never set 2): blank; `//` comment; `#[..]`/`#![..]`
attribute; `use`/`mod` items; a line made only of delimiters and optionally
the `else` keyword (e.g. `}`, `);`, `} else {`).

Exit codes: 0 pass, 1 threshold missed, 2 the check itself could not run
(missing inputs, no changed lines found, unreadable report).
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

NON_EXEC = re.compile(
    r"^\s*$"
    r"|^\s*//"
    r"|^\s*#!?\["
    r"|^\s*(pub(\([a-z]+\))?\s+)?(use|mod)\s"
    r"|^[\s\)\]\}\(\[\{;,]*(else)?[\s\{;,]*$"
)
HUNK = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@")
SUBJECT_PREFIXES = ("src/", "tests/")


def norm(path: str) -> str:
    return path.replace("\\", "/")


def added_lines(diff_text: str) -> dict[str, set[int]]:
    result: dict[str, set[int]] = {}
    current = None
    line_no = 0
    for raw in diff_text.splitlines():
        if raw.startswith("+++ "):
            target = raw[4:].strip()
            current = None
            if target.startswith("b/"):
                target = target[2:]
            target = norm(target)
            if target.endswith(".rs") and target.startswith(SUBJECT_PREFIXES):
                current = target
                result.setdefault(current, set())
            continue
        if current is None:
            continue
        m = HUNK.match(raw)
        if m:
            line_no = int(m.group(1))
            continue
        if raw.startswith("+") and not raw.startswith("+++"):
            result[current].add(line_no)
            line_no += 1
        elif raw.startswith("-") and not raw.startswith("---"):
            continue
        elif raw.startswith(" "):
            line_no += 1
    return result


def lcov_hits(lcov_text: str) -> dict[str, dict[int, int]]:
    hits: dict[str, dict[int, int]] = {}
    current = None
    for raw in lcov_text.splitlines():
        if raw.startswith("SF:"):
            current = norm(raw[3:].strip())
            hits.setdefault(current, {})
        elif raw.startswith("DA:") and current is not None:
            line_s, count_s = raw[3:].split(",")[:2]
            line = int(line_s)
            count = int(count_s)
            hits[current][line] = max(hits[current].get(line, 0), count)
        elif raw == "end_of_record":
            current = None
    return hits


def function_spans(cov_json: dict) -> dict[str, list[tuple[int, int]]]:
    spans: dict[str, list[tuple[int, int]]] = {}
    for export in cov_json.get("data", []):
        for fn in export.get("functions", []):
            filenames = [norm(f) for f in fn.get("filenames", [])]
            for region in fn.get("regions", []):
                line_start, _cs, line_end, _ce, _count, file_id = region[:6]
                if file_id >= len(filenames):
                    continue
                spans.setdefault(filenames[file_id], []).append((line_start, line_end))
    return spans


def match_key(rel: str, keys) -> str | None:
    rel_n = norm(rel)
    for key in keys:
        if key == rel_n or key.endswith("/" + rel_n):
            return key
    return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", required=True, help="base ref (merge-base with HEAD is used)")
    ap.add_argument("--lcov", required=True)
    ap.add_argument("--json", required=True)
    ap.add_argument("--diff-file", help="use this diff text instead of running git (self-test)")
    ap.add_argument("--report", help="write a markdown report here")
    ap.add_argument(
        "--allow",
        help="allowlist of accepted uncovered lines: `path|exact trimmed line text|reason` per line; "
        "every entry must match exactly one uncovered line or the check fails closed",
    )
    args = ap.parse_args()

    allow: list[tuple[str, str, str]] = []
    if args.allow:
        try:
            for raw in Path(args.allow).read_text(encoding="utf-8").splitlines():
                raw = raw.rstrip("\r")
                if not raw.strip() or raw.startswith("#"):
                    continue
                path_, text_, reason_ = raw.split("|", 2)
                allow.append((norm(path_.strip()), text_.strip(), reason_.strip()))
        except (OSError, ValueError) as error:
            print(f"FAIL: cannot read allowlist (fail closed): {error}")
            return 2

    if args.diff_file:
        diff_text = Path(args.diff_file).read_text(encoding="utf-8")
    else:
        merge_base = subprocess.run(
            ["git", "merge-base", args.base, "HEAD"], check=True, capture_output=True, text=True
        ).stdout.strip()
        diff_text = subprocess.run(
            ["git", "diff", "-U0", merge_base, "HEAD", "--", "src", "tests"],
            check=True, capture_output=True, text=True, encoding="utf-8",
        ).stdout

    changed = added_lines(diff_text)
    total_changed = sum(len(v) for v in changed.values())
    if total_changed == 0:
        print("FAIL: no added lines found under src/ or tests/ (fail closed: nothing to inspect)")
        return 2

    try:
        hits = lcov_hits(Path(args.lcov).read_text(encoding="utf-8"))
        spans = function_spans(json.loads(Path(args.json).read_text(encoding="utf-8")))
    except (OSError, ValueError) as error:
        print(f"FAIL: cannot read coverage inputs (fail closed): {error}")
        return 2
    if not hits:
        print("FAIL: LCOV contains no SF records (fail closed)")
        return 2

    rows = []
    covered = executable = non_exec = unmapped = uncovered = 0
    misses: list[str] = []
    unmapped_lines: list[str] = []
    for rel, lines in sorted(changed.items()):
        source = Path(rel).read_text(encoding="utf-8").splitlines()
        hit_key = match_key(rel, hits.keys())
        span_key = match_key(rel, spans.keys())
        file_hits = hits.get(hit_key, {}) if hit_key else {}
        file_spans = spans.get(span_key, []) if span_key else []
        f_cov = f_exec = f_non = f_unm = 0
        for line in sorted(lines):
            text = source[line - 1] if line - 1 < len(source) else ""
            in_span = any(s <= line <= e for s, e in file_spans)
            if NON_EXEC.match(text) or not in_span:
                non_exec += 1
                f_non += 1
                continue
            if line in file_hits:
                executable += 1
                f_exec += 1
                if file_hits[line] > 0:
                    covered += 1
                    f_cov += 1
                else:
                    uncovered += 1
                    misses.append(f"{rel}:{line}: {text.strip()}")
            else:
                unmapped += 1
                f_unm += 1
                unmapped_lines.append(f"{rel}:{line}: {text.strip()}")
        rows.append((rel, len(lines), f_cov, f_exec, f_non, f_unm))

    lines_out = [
        "| File | Added | Covered/Executable | Not executable | Executable, no mapping |",
        "|---|---|---|---|---|",
    ]
    for rel, n, c, e, ne, u in rows:
        lines_out.append(f"| {rel} | {n} | {c}/{e} | {ne} | {u} |")
    lines_out.append("")
    lines_out.append(
        f"Totals: {covered}/{executable} changed executable lines covered; "
        f"{non_exec} not executable; {unmapped} executable with no coverage mapping."
    )
    if misses:
        lines_out.append("Uncovered:")
        lines_out.extend(f"- {m}" for m in misses)
    if unmapped_lines:
        lines_out.append("Unmapped (set 3):")
        lines_out.extend(f"- {m}" for m in unmapped_lines)
    report = "\n".join(lines_out)
    print(report)
    if args.report:
        Path(args.report).write_text(report + "\n", encoding="utf-8")

    # Assurance boundary: the threshold applies to the subject under test
    # (src/). cargo-llvm-cov's report excludes integration-test sources under
    # tests/ by default, so their lines are reported above for the record but
    # do not decide the layer; the test layer is what exercises them.
    gated_uncovered = [m for m in misses if not m.startswith("tests/")]
    gated_unmapped = [m for m in unmapped_lines if not m.startswith("tests/")]
    if len(gated_uncovered) != len(misses) or len(gated_unmapped) != len(unmapped_lines):
        print("note: tests/ lines above are informational (outside cargo-llvm-cov's report scope); not gated")

    # Accepted uncovered lines: each allow entry must match exactly one
    # currently-uncovered line by file and exact trimmed text. An entry that
    # matches nothing is stale and fails the check (rc 2) so an allowlist can
    # never silently outlive the line it excused. Accepted lines are printed
    # so the evidence report can quote them with their reasons.
    accepted: list[str] = []
    for path_, text_, reason_ in allow:
        key = f"{path_}:"
        hits_ = [m for m in gated_uncovered if m.startswith(key) and m.split(": ", 1)[1] == text_]
        if len(hits_) != 1:
            print(f"FAIL: allowlist entry matches {len(hits_)} uncovered lines (expected exactly 1): {path_} | {text_}")
            return 2
        gated_uncovered.remove(hits_[0])
        accepted.append(f"{hits_[0]} — accepted: {reason_}")
    if accepted:
        print("Accepted uncovered lines (allowlist):")
        for a in accepted:
            print(f"- {a}")
        if args.report:
            with Path(args.report).open("a", encoding="utf-8") as fh:
                fh.write("Accepted uncovered lines (allowlist):\n" + "\n".join(f"- {a}" for a in accepted) + "\n")
    if gated_uncovered or gated_unmapped:
        print("FAIL: changed-line coverage threshold missed under src/ (uncovered or unmapped executable lines)")
        return 1
    if executable == 0:
        print("FAIL: no executable changed lines were instrumented (fail closed)")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
