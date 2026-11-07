#!/usr/bin/env python3
"""Measure the release CLI on a reproducible synthetic import graph, with no timing gate."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile
import time


def create_corpus(root, files, workload, packages, aliases):
    corpus = hashlib.sha256()

    def write(path, text):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        corpus.update(str(path.relative_to(root)).encode() + b"\0" + text.encode())

    # Control configuration explicitly instead of inheriting the caller's YAML.
    # The largest alias fixture intentionally needs more work than the product
    # default; both binaries receive the same finite benchmark-only allowance.
    work_limit = 64_000_000 if workload == "monorepo" else 20_000_000
    write(root / "jscycles.yaml", f"limits:\n  max_work: {work_limit}\n")
    config_arguments = ["--config", str(root / "jscycles.yaml")]
    if workload in ("fan-in", "call-heavy", "directory-index"):
        write(root / "package.json", '{"name":"benchmark"}')
        for number in range(files):
            if workload == "directory-index":
                neighbor = (number + 1) % files
                write(root / f"module{number}" / "index.ts", f"import '../module{neighbor}';\nexport const local = {number};\n")
            else:
                neighbor = 0 if number == 1 else 1
                if workload == "call-heavy":
                    text = f"const cjs = require('./module{neighbor}'); const dynamic = import('./module{neighbor}');\n"
                    text += "\n".join(f"Math.abs({call});" for call in range(12))
                else:
                    text = f"import {{ value }} from './module{neighbor}';\nexport const local = {number};\n"
                write(root / f"module{number}.ts", text)
        return corpus.hexdigest(), [*config_arguments, "--inner", "--no-tsconfig"], 1, 0

    write(root / "package.json", json.dumps({"private": True, "workspaces": ["packages/*"]}))
    mappings = {f"@alias/{number:04}": [f"packages/pkg{number % packages}/src"] for number in range(aliases)}
    write(root / "tsconfig.json", json.dumps({"compilerOptions": {"baseUrl": ".", "paths": mappings}}))
    for package in range(packages):
        directory = root / "packages" / f"pkg{package}"
        write(directory / "package.json", json.dumps({"name": f"@bench/pkg{package}"}))
        write(directory / "tsconfig.json", json.dumps({"extends": "../../tsconfig.json", "compilerOptions": {"baseUrl": ".", "paths": {"@local/*": ["src/*"]}}}))
        count = files // packages + (package < files % packages)
        for number in range(count):
            neighbor = 0 if number == 1 else 1
            text = f"import {{ value }} from '@local/module{neighbor}';\nexport const local = {number};\n"
            if number == 0:
                text += f"import '@bench/pkg{(package + 1) % packages}';\n"
            write(directory / "src" / f"module{number}.ts", text)
    return corpus.hexdigest(), config_arguments, packages, 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/release/jscycles")
    parser.add_argument("--baseline", help="Alternate this baseline binary with --binary on the same corpus")
    parser.add_argument("--files", type=int, default=1000)
    parser.add_argument("--workload", choices=["fan-in", "call-heavy", "directory-index", "monorepo"], default="fan-in")
    parser.add_argument("--packages", type=int, default=16)
    parser.add_argument("--aliases", type=int, default=512)
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--output", default="target/tooling/performance/report.json")
    args = parser.parse_args()
    if not 2 <= args.files <= 10000 or not 3 <= args.samples <= 50:
        parser.error("select 2..10000 files and 3..50 samples")
    if args.workload == "monorepo" and not (2 <= args.packages <= 64 and args.files >= 2 * args.packages and 1 <= args.aliases <= 4096):
        parser.error("monorepo requires 2..64 packages, at least two files per package, and 1..4096 aliases")
    binary = Path(args.binary).resolve()
    elapsed = []
    baseline = Path(args.baseline).resolve() if args.baseline else None
    baseline_elapsed = []
    profile_path = Path(os.environ["HOTPATH_OUTPUT_PATH"]) if os.environ.get("HOTPATH_OUTPUT_PATH") else None
    with tempfile.TemporaryDirectory(prefix="jscycles-benchmark-") as directory:
        root = Path(directory)
        corpus, arguments, expected_file_cycles, expected_package_cycles = create_corpus(root, args.files, args.workload, args.packages, args.aliases)
        expected_output = None
        for sample in range(args.samples + 1):
            runners = [(binary, elapsed)]
            if baseline:
                runners.append((baseline, baseline_elapsed))
                if sample % 2:
                    runners.reverse()
            for executable, timings in runners:
                if profile_path:
                    profile_path.unlink(missing_ok=True)
                command = [str(executable), str(root), *arguments, "--json"]
                start = time.perf_counter()
                result = subprocess.run(command, capture_output=True, text=True, timeout=60)
                seconds = time.perf_counter() - start
                if result.returncode != 1:
                    raise SystemExit(f"expected the known cycle, got {result.returncode}: {result.stderr}")
                data = json.loads(result.stdout)
                if profile_path:
                    profile = json.loads(profile_path.read_text())
                    functions = profile.get("functions_timing", {}).get("data", [])
                    if not any(item.get("name") == "jscycles::run" and item.get("sampled_calls", 0) > 0 for item in functions):
                        raise SystemExit("Hotpath report contains no measured scanner invocation")
                if (not data["has_cycles"] or len(data["file_cycles"]) != expected_file_cycles
                        or any(len(package["cycles"]) != 1 for package in data["file_cycles"])
                        or len(data["package_cycles"]) != expected_package_cycles):
                    raise SystemExit("benchmark output did not preserve the known graph result")
                if args.workload == "directory-index":
                    cycle = data["file_cycles"][0]["cycles"][0]
                    if len(cycle) != args.files + 1 or cycle[0] != cycle[-1] or len(set(cycle[:-1])) != args.files:
                        raise SystemExit("directory-index ring did not retain every file")
                if expected_output is None:
                    expected_output = data
                elif data != expected_output:
                    raise SystemExit("benchmark results differ between binaries or samples")
                if sample:  # one warm-up per binary; subsequent runs use the warm filesystem cache
                    timings.append(seconds)
    report = {"schema": 3, "workload": args.workload,
              "files": args.files, "packages": args.packages if args.workload == "monorepo" else 1,
              "aliases": args.aliases if args.workload == "monorepo" else 0,
              "analysis_work_limit": 64_000_000 if args.workload == "monorepo" else 20_000_000,
              "corpus_sha256": corpus, "warmups": 1,
              "samples_seconds": elapsed, "median_seconds": statistics.median(elapsed),
              "min_seconds": min(elapsed), "max_seconds": max(elapsed),
              "host": platform.platform(), "processor": platform.processor(),
              "rayon_num_threads": os.environ.get("RAYON_NUM_THREADS", "default"),
              "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "limitations": "Warm synthetic workload; no madge comparison, cold-cache result, or regression budget."}
    if baseline:
        report["baseline"] = {
            "binary_sha256": hashlib.sha256(baseline.read_bytes()).hexdigest(),
            "samples_seconds": baseline_elapsed,
            "median_seconds": statistics.median(baseline_elapsed),
        }
        report["comparison"] = {
            "order": "alternating on the same corpus, one warm-up per binary",
            "outputs_equal": True,
            "baseline_median_over_candidate": statistics.median(baseline_elapsed) / statistics.median(elapsed),
        }
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_suffix(".partial")
    temporary.write_text(json.dumps(report, indent=2) + "\n")
    temporary.replace(output)
    print(f"Median {report['median_seconds']:.3f}s; report: {output}")


if __name__ == "__main__":
    main()
