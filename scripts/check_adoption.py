#!/usr/bin/env python3
"""Check committed Rust-OE configuration and content integrity without source access.

This project gate checks schema-1 config fingerprints, inventory, rendered hashes,
bundle dependency closure, and the managed agent block. Rust-OE owns updates and
the full provenance/publication contract. Requires Python 3.11+ (tomllib).
"""

import sys

if sys.version_info < (3, 11):
    raise SystemExit('check_adoption.py requires Python 3.11+ (tomllib)')

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import tomllib


def require(condition, message):
    if not condition:
        raise ValueError(message)


def relative(value):
    require(isinstance(value, str), "path must be text")
    parts = value.split("/")
    require(0 < len(parts) <= 32 and all(part not in ("", ".", "..") for part in parts),
            f"invalid relative path: {value!r}")
    require(not any(character in value for character in '\\:\x00<>|?*"'), f"invalid path: {value!r}")
    require(all(part.lower() != ".git" for part in parts), "path enters Git metadata")
    return PurePosixPath(value)


def read(root, name, limit):
    path = root
    for part in relative(name).parts:
        path = path / part
        require(not path.is_symlink(), f"symlink refused: {name}")
    info = path.stat()
    require(stat.S_ISREG(info.st_mode) and info.st_size <= limit, f"invalid/oversized file: {name}")
    with path.open("rb") as stream:
        data = stream.read(limit + 1)
    require(len(data) <= limit, f"file exceeds limit: {name}")
    return data


def digest(data):
    return hashlib.sha256(data).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def configuration(root):
    data = tomllib.loads(read(root, "rust-oe.toml", 65536).decode())
    # Field order and defaults are the schema-1 Config serialization contract at
    # the adopted Rust-OE revision. Reordered TOML has the same fingerprint.
    defaults = {
        "schema": 1,
        "source": {"repository": "", "revision": "HEAD", "catalog": "rust-oe-bundles.toml"},
        "project": {"directory": ".rust-oe", "bundles": ["standards", "agents"], "notes": []},
        "agents": {"enabled": False, "path": "AGENTS.md"},
        "limits": {"max_file_bytes": 2097152, "max_total_bytes": 33554432,
                   "max_files": 2048, "git_timeout_seconds": 60},
    }

    def merge(given, expected):
        require(isinstance(given, dict) and not given.keys() - expected.keys(), "unknown config field/table")
        output = {}
        for key, default in expected.items():
            value = given.get(key, default)
            if isinstance(default, dict):
                output[key] = merge(value, default)
            else:
                require(type(value) is type(default), f"invalid config type: {key}")
                if isinstance(value, list):
                    require(all(isinstance(item, str) for item in value), f"invalid list: {key}")
                output[key] = value
        return output

    require("schema" in data and "repository" in data.get("source", {}), "schema/source required")
    config = merge(data, defaults)
    require(config["schema"] == 1, "unsupported config schema")
    for key, maximum in [("max_file_bytes", 16777216), ("max_total_bytes", 268435456),
                         ("max_files", 10000), ("git_timeout_seconds", 600)]:
        require(0 < config["limits"][key] <= maximum, f"invalid limit: {key}")
    require(len(relative(config["project"]["directory"]).parts) == 1, "managed directory must be top-level")
    return config


def check(root):
    config = configuration(root)
    lock = json.loads(read(root, "rust-oe.lock.json", 16777216), object_pairs_hook=unique_object)
    require(type(lock["schema"]) is int and lock["schema"] == 1, "unsupported adoption lock schema")
    encoded = json.dumps(config, ensure_ascii=False, separators=(",", ":")).encode()
    require(digest(encoded) == lock["config_sha256"], "config drift; review rust-oe plan and sync")
    for key, expected in {"directory": config["project"]["directory"],
                          "repository": config["source"]["repository"], "catalog": config["source"]["catalog"],
                          "selections": config["project"]["bundles"], "notes": config["project"]["notes"]}.items():
        require(lock[key] == expected, f"lock/config mismatch: {key}")
    for note in lock["notes"]:
        read(root, note, config["limits"]["max_file_bytes"])

    bundles = lock["bundles"]
    require(isinstance(bundles, dict) and len(bundles) <= 128, "invalid bundle inventory")
    closure = set()

    def visit(name, active):
        require(name in bundles and name not in active, "missing/cyclic bundle dependency")
        if name in closure:
            return
        for dependency in bundles[name]["requires"]:
            visit(dependency, active | {name})
        closure.add(name)

    for selection in lock["selections"]:
        visit(selection, set())
    require(closure == set(bundles), "bundle inventory differs from selected dependency closure")
    expected = {"INDEX.md": lock["files"]["INDEX.md"]}
    for name, bundle in bundles.items():
        require(re.fullmatch(r"[a-z][a-z0-9_-]*", name), "invalid bundle name")
        require(re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", bundle["revision"]), "bundle must pin a commit")
        for path, record in bundle["files"].items():
            relative(path)
            expected[f"bundles/{name}/{path}"] = record["sha256"]
    require(expected == lock["files"], "bundle/file inventory mismatch")
    require(len(expected) <= config["limits"]["max_files"], "too many adopted files")

    directory = config["project"]["directory"]
    expected_dirs = {str(parent) for path in expected for parent in relative(path).parents if str(parent) != "."}
    stack = [root / directory]
    require(not stack[0].is_symlink(), "managed directory is a symlink")
    while stack:
        with os.scandir(stack.pop()) as entries:
            for entry in entries:
                name = Path(entry.path).relative_to(root / directory).as_posix()
                require(not entry.is_symlink(), f"managed symlink refused: {name}")
                if entry.is_dir(follow_symlinks=False):
                    require(name in expected_dirs, f"unmanaged directory: {name}")
                    stack.append(Path(entry.path))
                else:
                    require(name in expected, f"unmanaged file: {name}")
    total = 0
    for path, expected_hash in expected.items():
        data = read(root, f"{directory}/{path}", config["limits"]["max_file_bytes"])
        total += len(data)
        require(digest(data) == expected_hash, f"managed content drift: {path}")
    require(total <= config["limits"]["max_total_bytes"], "adopted content exceeds total budget")
    if config["agents"]["enabled"]:
        agent = lock["agent"]
        require(agent["path"] == config["agents"]["path"], "agent path mismatch")
        text = read(root, agent["path"], 1048576).decode()
        start, end = "<!-- rust-oe:start -->", "<!-- rust-oe:end -->"
        require(text.count(start) == text.count(end) == 1, "missing/ambiguous agent markers")
        first, last = text.index(start), text.index(end) + len(end)
        require(first < last and digest(text[first:last].encode()) == agent["sha256"], "agent block drift")
    else:
        require(lock["agent"] is None, "disabled agent integration remains locked")
    return len(expected)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    try:
        count = check(args.project.resolve())
    except (OSError, ValueError, KeyError, TypeError, RecursionError) as error:
        print(f"Rust-OE integrity failed: {error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"Rust-OE integrity passed: {count} managed files, configuration, and agent block")
