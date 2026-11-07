#!/usr/bin/env python3
"""Record or check jscycles results on real repositories.

  python3 scripts/differential/real_repos.py record BASELINE.json REPO...
  python3 scripts/differential/real_repos.py check  BASELINE.json [--config REPO=CONFIG ...]

`record` stores, per repository, the exit status, the error line (if any), and
the normalized default/--inner/--outer reports. `check` reruns every recorded
repository and prints each difference, exiting 1 when anything changed. A
change is not necessarily a regression; review it and re-record deliberately.
Baselines name local paths, so keep them outside the repository.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

if sys.version_info < (3, 11):
    sys.exit("real_repos.py requires Python 3.11+")

REPO = Path(__file__).resolve().parents[2]


def run(binary: Path, repo: Path, flags: list[str]) -> dict:
    result = subprocess.run([str(binary), "--json", *flags], cwd=repo, capture_output=True, text=True, timeout=600)
    entry: dict = {"exit": result.returncode}
    if result.returncode in (0, 1):
        report = json.loads(result.stdout)
        entry["file_cycles"] = {e["name"]: e["cycles"] for e in report["file_cycles"]}
        entry["package_cycles"] = [c["packages"] for c in report["package_cycles"]]
    else:
        lines = result.stderr.strip().splitlines()
        entry["error"] = lines[-1] if lines else ""
    warnings = sorted(line for line in result.stderr.splitlines() if line.startswith("warning:"))
    if warnings:
        entry["warnings"] = warnings
    return entry


def snapshot(binary: Path, repo: Path, config: str | None) -> dict:
    extra = ["--config", config] if config else []
    return {mode or "default": run(binary, repo, extra + ([mode] if mode else [])) for mode in ("", "--inner", "--outer")}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("action", choices=["record", "check"])
    parser.add_argument("baseline", type=Path)
    parser.add_argument("repos", nargs="*", type=Path)
    parser.add_argument("--binary", type=Path,
                        default=REPO / "target" / "debug" / ("jscycles.exe" if sys.platform == "win32" else "jscycles"))
    parser.add_argument("--config", action="append", default=[], help="REPO=CONFIG to pass --config for a repository")
    args = parser.parse_args()
    configs = dict(item.split("=", 1) for item in args.config)
    if args.action == "record":
        baseline = {}
        for repo in args.repos:
            key = str(repo.resolve())
            baseline[key] = {"config": configs.get(str(repo)) or configs.get(key),
                             "results": snapshot(args.binary, repo.resolve(), configs.get(str(repo)) or configs.get(key))}
            print(f"recorded {key}: exit {baseline[key]['results']['default']['exit']}")
        args.baseline.write_text(json.dumps(baseline, indent=2, sort_keys=True) + "\n")
        return 0
    baseline = json.loads(args.baseline.read_text())
    changed = 0
    for repo, recorded in baseline.items():
        current = snapshot(args.binary, Path(repo), recorded.get("config"))
        for mode, before in recorded["results"].items():
            after = current[mode]
            if before == after:
                continue
            changed += 1
            print(f"CHANGED {repo} [{mode}]")
            for key in sorted(set(before) | set(after)):
                if before.get(key) != after.get(key):
                    if key == "file_cycles":
                        for name in sorted(set(before.get(key, {})) | set(after.get(key, {}))):
                            b, a = before.get(key, {}).get(name), after.get(key, {}).get(name)
                            if b != a:
                                print(f"  {name}: {len(b or [])} -> {len(a or [])} cycles")
                    else:
                        print(f"  {key}: {before.get(key)!r} -> {after.get(key)!r}")
        if not any(current[m] != recorded["results"][m] for m in current):
            print(f"same     {repo}")
    return 1 if changed else 0


if __name__ == "__main__":
    sys.exit(main())
