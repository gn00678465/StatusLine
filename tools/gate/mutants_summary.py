#!/usr/bin/env python3
"""Summarise a cargo-mutants run from its mutants.out/outcomes.json (fail closed).

Usage:
  mutants_summary.py <mutants.out dir> <cargo-mutants exit code>
      Prints counts; lists every missed mutant; exits 1 when any mutant was
      missed or timed out, or when the baseline failed; exits 2 when zero
      mutants were generated or the outcomes file cannot be read.
  mutants_summary.py --sample-functions N <mutants.out dir> <rc>
      Prints up to N distinct function names that had caught mutants, one per
      line (for the kill-sample re-run). Exits 2 when there are none.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path


def load(out_dir: str) -> dict:
    path = Path(out_dir) / "outcomes.json"
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        print(f"FAIL: cannot read {path} (fail closed): {error}")
        sys.exit(2)


def mutant_of(outcome: dict) -> dict | None:
    scenario = outcome.get("scenario")
    if isinstance(scenario, dict) and "Mutant" in scenario:
        return scenario["Mutant"]
    return None


def describe(m: dict) -> str:
    if m.get("name"):
        return str(m["name"])
    fn = m.get("function") or {}
    start = (m.get("span") or {}).get("start") or {}
    return f'{m.get("file")}:{start.get("line")}:{start.get("column")} {fn.get("function_name")} -> {m.get("replacement")}'


def main() -> int:
    args = sys.argv[1:]
    sample = 0
    if args and args[0] == "--sample-functions":
        sample = int(args[1])
        args = args[2:]
    if len(args) != 2:
        print(__doc__)
        return 2
    out_dir, rc_s = args
    tool_rc = int(rc_s)
    data = load(out_dir)
    outcomes = data.get("outcomes", [])
    counts: dict[str, int] = {}
    missed, timed_out, caught_fns = [], [], []
    baseline_failed = False
    for o in outcomes:
        summary = o.get("summary", "?")
        m = mutant_of(o)
        if m is None:
            if summary not in ("Success",):
                baseline_failed = True
            continue
        counts[summary] = counts.get(summary, 0) + 1
        if summary == "MissedMutant":
            missed.append(describe(m))
        elif summary == "Timeout":
            timed_out.append(describe(m))
        elif summary == "CaughtMutant":
            name = (m.get("function") or {}).get("function_name")
            if name and name not in caught_fns:
                caught_fns.append(name)
    total = data.get("total_mutants", sum(counts.values()))

    if sample:
        if not caught_fns:
            print("FAIL: no caught mutants to sample (fail closed)")
            return 2
        for name in caught_fns[:sample]:
            print(name)
        return 0

    print(f"mutants: total={total} " + " ".join(f"{k}={v}" for k, v in sorted(counts.items())) + f" tool_rc={tool_rc}")
    if total == 0:
        print("FAIL: zero mutants generated (fail closed)")
        return 2
    if baseline_failed:
        print("FAIL: unmutated baseline did not pass; the round's score is void")
        return 1
    for d in missed:
        print(f"MISSED: {d}")
    for d in timed_out:
        print(f"TIMEOUT: {d}")
    if missed or timed_out:
        return 1
    if tool_rc not in (0,):
        print(f"FAIL: cargo-mutants exited {tool_rc} with no missed/timeout recorded (fail closed)")
        return 1
    print(f"ok: {counts.get('CaughtMutant', 0)} caught, 0 missed, 0 timeout ({counts.get('Unviable', 0)} unviable)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
