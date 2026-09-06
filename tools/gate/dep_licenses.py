#!/usr/bin/env python3
"""License check for crates added by the change (fail closed).

Usage: dep_licenses.py <cargo metadata json> <lock-added.txt>
lock-added.txt holds `name = "<crate>"` lines. Every added crate must resolve in
the metadata and carry an MIT or Apache-2.0 license expression. Exit 1 on any
violation, 2 when inputs cannot be read.
"""
import json
import sys
from pathlib import Path


def main() -> int:
    try:
        meta = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
        added = [line.split('"')[1] for line in Path(sys.argv[2]).read_text(encoding="utf-8").splitlines() if '"' in line]
    except (OSError, ValueError, IndexError) as error:
        print(f"FAIL: cannot read inputs (fail closed): {error}")
        return 2
    if not added:
        print("new deps: none")
        return 0
    by_name = {}
    for p in meta.get("packages", []):
        by_name.setdefault(p["name"], []).append(p)
    rc = 0
    for name in added:
        pkgs = by_name.get(name)
        if not pkgs:
            print(f"FAIL: added crate {name} not found in cargo metadata")
            rc = 1
            continue
        for p in pkgs:
            lic = p.get("license") or ""
            ok = "MIT" in lic or "Apache-2.0" in lic
            print(f"new dep: {name} {p['version']} license={lic!r} {'ok' if ok else 'FAIL'}")
            if not ok:
                rc = 1
    return rc


if __name__ == "__main__":
    sys.exit(main())
