#!/usr/bin/env python3
"""Compare jscycles with madge and dpdm on public repositories.

  python3 scripts/compare_tools.py [--runs 3] [--output report.json]

Clones each repository below at a pinned commit into target/tooling/bench/,
installs madge and dpdm there (no project scripts run), and times each tool on
the same checkout: one warm-up run, then the median of --runs timed runs.
Cycle counts are reported as each tool defines them and are not directly
comparable: jscycles reports one representative cycle per strongly connected
component, madge and dpdm list many overlapping cycles. Needs git, Node.js and
network access the first time. Results depend on hardware and cache state.
"""

from __future__ import annotations

import argparse
import json
import platform
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

if sys.version_info < (3, 11):
    sys.exit("compare_tools.py requires Python 3.11+")

REPO = Path(__file__).resolve().parents[1]
BENCH = REPO / "target" / "tooling" / "bench"
TOOLS = {"madge": "8.0.0", "dpdm": "3.14.0"}
EXTENSIONS = "ts,tsx,mts,cts,js,jsx,mjs,cjs"
# name, URL, commit, directory to analyze (relative to the checkout)
REPOSITORIES = [
    ("nest", "https://github.com/nestjs/nest.git", "ed90b8357b40c6e94a466f17f0e6ade88cf80048", "packages"),
    ("typeorm", "https://github.com/typeorm/typeorm.git", "5a572a40bae2f85a1d5f7b87a0aa88bbbaec5ab3", "packages"),
    ("excalidraw", "https://github.com/excalidraw/excalidraw.git", "701b901a0f1ba3d0b390d9cdf63d112e25abac4b", "packages"),
    ("ts-morph", "https://github.com/dsherret/ts-morph.git", "f288183ddb496adc6f4c5b6929830b5b73437185", "packages"),
]


def sh(command: list[str], cwd: Path | None = None, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=check)


def checkout(name: str, url: str, commit: str) -> Path:
    path = BENCH / "repos" / name
    if not (path / ".git").is_dir():
        path.parent.mkdir(parents=True, exist_ok=True)
        sh(["git", "clone", "--quiet", "--filter=blob:none", "--no-checkout", url, str(path)])
    sh(["git", "fetch", "--quiet", "--depth", "1", "origin", commit], cwd=path, check=False)
    sh(["git", "checkout", "--quiet", "--detach", commit], cwd=path)
    return path


def install_tools() -> dict[str, Path]:
    bins = {name: BENCH / "node_modules" / ".bin" / name for name in TOOLS}
    if not all(path.exists() for path in bins.values()):
        sh([shutil.which("npm") or "npm", "install", "--silent", "--no-audit", "--no-fund", "--ignore-scripts",
            "--prefix", str(BENCH), *(f"{name}@{version}" for name, version in TOOLS.items())])
    return bins


def timed(command: list[str], cwd: Path, runs: int) -> tuple[float, subprocess.CompletedProcess[str]]:
    """Median wall time. Stdout goes to a file: Node tools that exit right
    after writing truncate output larger than a pipe buffer."""
    sink = BENCH / "stdout.txt"
    times, code = [], 0
    for run in range(runs + 1):  # the first run is a warm-up
        with sink.open("w") as out:
            start = time.perf_counter()
            code = subprocess.run(command, cwd=cwd, stdout=out, stderr=subprocess.DEVNULL, check=False).returncode
            elapsed = time.perf_counter() - start
        if run:
            times.append(elapsed)
    return statistics.median(times), subprocess.CompletedProcess(command, code, sink.read_text(), "")


def count_jscycles(result: subprocess.CompletedProcess[str]) -> int | None:
    if result.returncode not in (0, 1):
        return None
    report = json.loads(result.stdout)
    return sum(len(entry["cycles"]) for entry in report["file_cycles"]) + len(report["package_cycles"])


def count_json_list(result: subprocess.CompletedProcess[str]) -> int | None:
    try:
        value = json.loads(result.stdout)
    except ValueError:
        return None
    return len(value) if isinstance(value, list) else None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--binary", type=Path, default=REPO / "target" / "release" / "jscycles")
    parser.add_argument("--output", type=Path, default=BENCH / "report.json")
    args = parser.parse_args()
    bins = install_tools()
    rows = []
    for name, url, commit, directory in REPOSITORIES:
        root = checkout(name, url, commit)
        target = root / directory
        files = int(sh(["git", "ls-files", "--", f"{directory}/*.ts", f"{directory}/*.tsx", f"{directory}/*.js",
                        f"{directory}/*.mts", f"{directory}/*.cts"], cwd=root).stdout.count("\n"))
        tsconfig = next((p for p in (target / "tsconfig.json", root / "tsconfig.json") if p.is_file()), None)
        madge = [str(bins["madge"]), "--circular", "--json", "--no-spinner", "--extensions", EXTENSIONS]
        if tsconfig:
            madge += ["--ts-config", str(tsconfig)]
        commands = {
            "jscycles": ([str(args.binary), "--json", directory], count_jscycles),
            "madge": (madge + [directory], count_json_list),
            "dpdm": ([str(bins["dpdm"]), "--no-tree", "--no-warning", "--no-progress", "--exit-code", "circular:0",
                      "--output", str(BENCH / f"dpdm-{name}.json"), f"{directory}/**/*.{{ts,tsx,mts,cts,js,jsx}}"],
                     None),
        }
        row = {"repository": name, "url": url, "commit": commit, "directory": directory, "source_files": files}
        for tool, (command, counter) in commands.items():
            seconds, result = timed(command, root, args.runs)
            if tool == "dpdm":
                try:
                    count = len(json.loads((BENCH / f"dpdm-{name}.json").read_text())["circulars"])
                except (OSError, ValueError, KeyError):
                    count = None
            else:
                count = counter(result) if counter else None
            row[tool] = {"seconds": round(seconds, 3), "exit": result.returncode, "cycles": count}
            print(f"{name:11} {tool:9} {seconds:8.2f}s  exit {result.returncode}  cycles {count}", flush=True)
        rows.append(row)
    report = {"host": {"system": platform.system(), "machine": platform.machine(), "python": platform.python_version()},
              "jscycles": sh([str(args.binary), "--version"]).stdout.strip(), "tools": TOOLS, "runs": args.runs,
              "results": rows}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
