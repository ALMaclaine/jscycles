#!/usr/bin/env python3
"""Run isolated mutations and reject survivors, stale reviews and unknown timeouts."""

import sys

if sys.version_info < (3, 11):
    raise SystemExit('mutation.py requires Python 3.11+ (tomllib)')

import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tomllib

CARGO_BUILD_JOBS = 2
# Every mutant rebuilds the crate in one shared scratch target directory. With
# debug info on, macOS keeps each build's codegen-unit objects in deps/ (the
# unpacked split-debuginfo default), so hundreds of thousands of stale files
# pile up until listing the directory stalls rustc past the build timeout.
# Mutation outcomes do not depend on debug info.
CARGO_ARGS = ["-C", "--locked", "-C", f"--jobs={CARGO_BUILD_JOBS}",
              "-C", "--config=profile.dev.debug=0"]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def selection_digest(names):
    return hashlib.sha256(json.dumps(sorted(names), separators=(",", ":")).encode()).hexdigest()


def validate_execution_identity(execution):
    if not isinstance(execution, dict) or not isinstance(execution.get("toolchain"), str):
        raise ValueError("mutation campaign is missing its compiler execution identity")
    selected = execution["toolchain"]
    if (not execution.get("cargo_version", "").startswith(f"cargo {selected} ")
            or not execution.get("rustc_verbose", "").startswith(f"rustc {selected} ")):
        raise ValueError("mutation compiler identity does not match the selected toolchain")


@contextmanager
def project_toolchain(project):
    """Select the repository compiler even when the caller exports an override."""
    selected = tomllib.loads((project / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    previous = os.environ.get("RUSTUP_TOOLCHAIN")
    os.environ["RUSTUP_TOOLCHAIN"] = selected
    try:
        execution = {
            "toolchain": selected,
            "cargo_version": subprocess.check_output(
                ["cargo", "--version"], cwd=project, text=True, timeout=30).strip(),
            "rustc_verbose": subprocess.check_output(
                ["rustc", "-vV"], cwd=project, text=True, timeout=30).strip(),
        }
        validate_execution_identity(execution)
        yield execution
    finally:
        if previous is None:
            os.environ.pop("RUSTUP_TOOLCHAIN", None)
        else:
            os.environ["RUSTUP_TOOLCHAIN"] = previous


def input_snapshot(project):
    """Bind each execution to its actual source, including newly added modules/tests."""
    fixed = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "clippy.toml", "rustfmt.toml",
             "README.md", "build.rs", "xtask/Cargo.toml", "scripts/mutation.py", "scripts/test_mutation.py"]
    paths = {project / name for name in fixed if (project / name).is_file()}
    for directory, pattern in [("src", "*.rs"), ("xtask/src", "*.rs"), ("tests", "*")]:
        paths.update(path for path in (project / directory).rglob(pattern) if path.is_file())
    # Some runners keep CARGO_HOME here: registry/git caches are generated state,
    # while only the checkout's immediate Cargo configuration is an input.
    paths.update(path for path in (project / ".cargo").glob("*.toml") if path.is_file())
    policy = project / ".cargo/mutation-review.json"
    if policy.is_file():
        paths.add(policy)
    return {str(path.relative_to(project)): digest(path) for path in sorted(paths)}


@contextmanager
def campaign_lock(destination):
    destination.mkdir(parents=True, exist_ok=True)
    lock = destination / "runner.lock"
    try:
        handle = lock.open("x")
    except FileExistsError as error:
        raise ValueError(f"mutation runner already owns {lock}; inspect its PID before removing a stale lock") from error
    try:
        with handle:
            handle.write(str(os.getpid()))
        yield
    finally:
        lock.unlink(missing_ok=True)


def capture_campaign(project, policy, files):
    if not files:
        raise ValueError("mutation scope must select source files")
    before = input_snapshot(project)
    arguments = [argument for name in files for argument in ("--file", name)]
    candidates = json.loads(subprocess.check_output(
        ["cargo", "mutants", *arguments, "--list", "--json"], cwd=project, text=True, timeout=60))
    names = [candidate["name"] for candidate in candidates]
    if not names or len(names) != len(set(names)):
        raise ValueError("mutation selection must be nonempty with unique case identities")
    if input_snapshot(project) != before:
        raise ValueError("mutation inputs changed during selection")
    return {**policy, "schema": 3 if policy.get("execution") else 2, "files": files, "inputs": before,
            "selected_mutants": len(names), "selection_sha256": selection_digest(names)}


def validate_snapshot(project, expected):
    if input_snapshot(project) != expected:
        raise ValueError("mutation campaign inputs are stale; run a fresh campaign")


def retry_directory(output, item):
    return output / "replays" / digest(output / item["diff_path"]) / "mutants.out"


def verified_retry(output, item):
    replay = retry_directory(output, item)
    path = replay / "outcomes.json"
    if not path.exists():
        return False
    outcomes = json.loads(path.read_text())["outcomes"]
    baselines = [entry for entry in outcomes if entry["scenario"] == "Baseline"]
    mutants = [entry for entry in outcomes if isinstance(entry["scenario"], dict)]
    if len(baselines) != 1 or baselines[0]["summary"] != "Success" or len(mutants) != 1:
        raise ValueError("retry must contain a passing baseline and exactly one mutation")
    retry = mutants[0]
    if (retry["summary"] != "CaughtMutant"
            or retry["scenario"]["Mutant"]["name"] != item["scenario"]["Mutant"]["name"]
            or digest(replay / retry["diff_path"]) != digest(output / item["diff_path"])):
        raise ValueError("retry must catch the exact same mutation and diff")
    phases = retry["phase_results"]
    built = any(phase["phase"] == "Build" and phase["process_status"] == "Success" for phase in phases)
    failed_test = any(phase["phase"] == "Test" and isinstance(phase["process_status"], dict)
                      and isinstance(phase["process_status"].get("Failure"), int)
                      and phase["process_status"]["Failure"] > 0 for phase in phases)
    if not built or not failed_test:
        raise ValueError("retry requires a successful build and an actual test failure")
    return True


def retry_selection(project, output, item):
    """Constrain retries despite 27.1.0's unfiltered struct-field mutations."""
    mutant = item["scenario"]["Mutant"]
    name = mutant["name"]
    destination = retry_directory(output, item).parent
    destination.mkdir(parents=True, exist_ok=True)
    lines = (project / mutant["file"]).read_text().splitlines()
    line = mutant["span"]["start"]["line"]
    if not 1 <= line <= len(lines):
        raise ValueError("retry source position is outside the recorded input")
    text = lines[line - 1]
    scope = destination / "selection.diff"
    # This hunk selects a line without changing the source. Name filtering alone
    # also selects unrelated struct-field deletions in cargo-mutants 27.1.0.
    scope.write_text(f"--- a/{mutant['file']}\n+++ b/{mutant['file']}\n"
                     f"@@ -{line} +{line} @@\n-{text}\n+{text}\n")
    pattern = "^" + re.escape(name).replace(r"\ ", " ") + "$"
    selection = ["cargo", "mutants", "--file", mutant["file"], "--re", pattern,
                 "--in-diff", str(scope)]
    candidates = json.loads(subprocess.check_output(
        [*selection, "--list", "--json"], cwd=project, text=True, timeout=30))
    if len(candidates) != 1 or candidates[0]["name"] != name:
        raise ValueError("retry selection must contain exactly one matching mutation")
    if hashlib.sha256(candidates[0]["diff"].encode()).hexdigest() != digest(output / item["diff_path"]):
        raise ValueError("retry selection has a different mutation diff")
    (destination / "selection.json").write_text(json.dumps(candidates, indent=2) + "\n")
    return selection, destination


def retry_unknown_timeouts(project, output, policy):
    outcomes = json.loads((output / "outcomes.json").read_text())["outcomes"]
    baselines = [item for item in outcomes if item["scenario"] == "Baseline"]
    mutants = [item for item in outcomes if isinstance(item["scenario"], dict)]
    names = [item["scenario"]["Mutant"]["name"] for item in mutants]
    if len(baselines) != 1 or baselines[0]["summary"] != "Success":
        raise ValueError("a failed baseline cannot be repaired by retries")
    if (len(mutants) != policy["selected_mutants"] or len(set(names)) != len(names)
            or selection_digest(names) != policy["selection_sha256"]):
        raise ValueError("an incomplete or different selection cannot be repaired by retries")
    unknown = [item for item in mutants if item["summary"] == "Timeout"
               and digest(output / item["diff_path"]) not in policy["timeouts"]]
    if len(unknown) > 3:
        raise ValueError("too many unknown timeouts; investigate runner capacity")
    for item in unknown:
        if verified_retry(output, item):
            continue
        if not any(phase["phase"] == "Build" and phase["process_status"] == "Success"
                   for phase in item["phase_results"]):
            raise ValueError("build timeouts cannot be retried as detected mutations")
        name = item["scenario"]["Mutant"]["name"]
        selection, destination = retry_selection(project, output, item)
        result = subprocess.run([
            *selection, "--test-tool", "cargo", "--jobs", "1", "--timeout", "60",
            "--build-timeout", "180", "--output", str(destination), *CARGO_ARGS,
        ], cwd=project, check=False)
        if result.returncode != 0 or not verified_retry(output, item):
            raise ValueError(f"mutation timeout remains unresolved: {name}")


def review_results(project, output, policy):
    if policy.get("schema", 0) >= 3:
        validate_execution_identity(policy.get("execution"))
    for name, expected in policy["inputs"].items():
        if digest(project / name) != expected:
            raise ValueError(f"mutation review is stale for {name}; rerun and review the campaign")
    document = json.loads((output / "outcomes.json").read_text())
    if policy.get("tool") and document.get("cargo_mutants_version") != policy["tool"].removeprefix("cargo-mutants "):
        raise ValueError("mutation results use a different tool version")
    results = document["outcomes"]
    baselines = [item for item in results if item["scenario"] == "Baseline"]
    if len(baselines) != 1 or baselines[0]["summary"] != "Success":
        raise ValueError("the unmutated baseline did not pass")
    mutants = [item for item in results if isinstance(item["scenario"], dict)]
    if not mutants or len(mutants) != policy["selected_mutants"]:
        raise ValueError("mutation campaign is empty or incomplete; review a changed selection")
    names = [item["scenario"]["Mutant"]["name"] for item in mutants]
    if len(names) != len(set(names)) or selection_digest(names) != policy["selection_sha256"]:
        raise ValueError("mutation identities differ from the reviewed selection")
    counts = {"caught": 0, "unviable": 0, "reviewed_timeouts": 0}
    for item in mutants:
        status = item["summary"]
        if status == "CaughtMutant":
            counts["caught"] += 1
        elif status == "Unviable":
            counts["unviable"] += 1
        elif status == "Timeout":
            phases = item["phase_results"]
            if not any(p["phase"] == "Build" and p["process_status"] == "Success" for p in phases):
                raise ValueError("a build timeout is not a detected mutation")
            if not any(p["phase"] == "Test" and p["process_status"] == "Timeout" for p in phases):
                raise ValueError("missing test timeout evidence")
            change = digest(output / item["diff_path"])
            if change not in policy["timeouts"]:
                if not verified_retry(output, item):
                    raise ValueError(f"unreviewed timeout: {item['scenario']['Mutant']['name']}")
                counts["caught"] += 1
                counts["replayed_timeouts"] = counts.get("replayed_timeouts", 0) + 1
            else:
                counts["reviewed_timeouts"] += 1
        else:
            raise ValueError(f"unresolved mutation ({status}): {item['scenario']['Mutant']['name']}")
    if counts["caught"] + counts["reviewed_timeouts"] == 0:
        raise ValueError("no runtime mutation was exercised")
    return counts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--review-only", action="store_true", help="Review the already completed local campaign")
    parser.add_argument("--retry-timeouts", action="store_true", help="Resolve unknown timeouts with isolated 60-second retries")
    parser.add_argument("--file", action="append", help="Explicit development scope; defaults to all production modules")
    parser.add_argument("--output", type=Path, default=Path("target/tooling/mutation"), help="Campaign directory")
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    if args.review_only and not args.retry_timeouts:
        run_campaign(project, args)
    else:
        with project_toolchain(project) as execution:
            if args.review_only:
                run_campaign(project, args, execution)
            else:
                with campaign_lock(project / args.output):
                    run_campaign(project, args, execution)


def run_campaign(project, args, execution=None):
    destination = project / args.output
    output = destination / "mutants.out"
    policy = json.loads((project / ".cargo/mutation-review.json").read_text())
    if not args.review_only:
        if execution is None:
            raise ValueError("a fresh mutation campaign requires a selected compiler")
        policy["execution"] = execution
        # Mutant workers each launch Cargo. Bound their nested compilation too,
        # rather than multiplying the host's automatic compiler parallelism.
        policy["cargo_build_jobs"] = CARGO_BUILD_JOBS
        policy["cargo_args"] = CARGO_ARGS
        actual = subprocess.check_output(["cargo", "mutants", "--version"], cwd=project, text=True, timeout=30).strip()
        if actual != policy["tool"]:
            raise ValueError(f"expected {policy['tool']}, got {actual}")
    if args.review_only:
        if args.file:
            raise SystemExit("review uses the recorded campaign scope; omit --file")
        manifest = output / "campaign.json"
        if not manifest.exists():
            raise SystemExit("mutation campaign manifest is missing; run a fresh campaign")
        policy = json.loads(manifest.read_text())
        validate_snapshot(project, policy["inputs"])
        if execution is not None and policy.get("execution") not in (None, execution):
            raise ValueError("mutation retries require the campaign's recorded compiler")
    else:
        policy = capture_campaign(project, policy, args.file or policy["files"])
    before = input_snapshot(project)
    # A failed tool invocation must never reuse a successful previous report.
    if not args.review_only:
        destination.mkdir(parents=True, exist_ok=True)
        (output / "outcomes.json").unlink(missing_ok=True)
        pending = destination / "campaign.pending.json"
        pending.write_text(json.dumps(policy, indent=2) + "\n")
        files = [argument for name in policy["files"] for argument in ("--file", name)]
        result = subprocess.run([
            "cargo", "mutants", *files,
            "--test-tool", "cargo", "--jobs", "2", "--timeout", "60", "--build-timeout", "180",
            "--output", str(destination), *CARGO_ARGS,
        ], cwd=project, check=False)
        if (output / "outcomes.json").exists():
            pending.replace(output / "campaign.json")
        if result.returncode not in (0, 3):
            raise SystemExit(f"cargo-mutants failed with exit {result.returncode}")
        if input_snapshot(project) != before:
            raise SystemExit("mutation inputs changed during the campaign")
    try:
        if not args.review_only or args.retry_timeouts:
            retry_unknown_timeouts(project, output, policy)
        if input_snapshot(project) != before:
            raise ValueError("mutation inputs changed during review or retries")
        counts = review_results(project, output, policy)
    except (ValueError, KeyError, OSError) as error:
        raise SystemExit(str(error)) from error
    print("Mutation review passed: " + json.dumps(counts, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.SubprocessError) as error:
        raise SystemExit(str(error)) from error
