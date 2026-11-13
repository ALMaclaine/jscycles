#!/usr/bin/env python3
"""Generated differential testing of jscycles against a reference model.

Each case is a small random project (workspace members, nested packages,
tsconfig file and workspace aliases, every relative-specifier spelling, scan
excludes in several spellings). The reference model reads the generated tree
from disk, resolves file imports with TypeScript's own resolver
(scripts/differential/tsresolve.cjs), applies the product contract's package
rules (docs/product-contract.md R01/R02/R08), and computes cycles by brute
force. Any disagreement with `jscycles --json` is shrunk to a minimal project
and written to the output directory.

Usage:
  python3 scripts/differential/difftest.py --cases 500 [--seed N] [--binary PATH]

Requires Python 3.11+, Node.js and network access the first time (TypeScript
5.9.3 is installed under target/tooling/differential/). Not part of
`cargo xtask ci`; see tooling/checks.md.
"""

from __future__ import annotations

import argparse
import collections
import fnmatch
import json
import os
import posixpath
import random
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

if sys.version_info < (3, 11):
    sys.exit("difftest.py requires Python 3.11+")

REPO = Path(__file__).resolve().parents[2]
WINDOWS = os.name == "nt"
TOOL_DIR = REPO / "target" / "tooling" / "differential"
TYPESCRIPT = "5.9.3"
SOURCE_EXTENSIONS = (".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs")
DEFAULT_EXTENSIONS = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"]
STEM = re.compile(r"(\.d)?\.(ts|tsx|js|jsx|mts|cts|mjs|cjs)$")
BUILTIN_EXCLUDED = {"node_modules", "dist", "build", ".next", ".git"}
SPECIFIER = re.compile(r"""(?:from\s+|import\s*\(\s*|require\s*\(\s*|^\s*import\s+)(['"])([^'"]+)\1""", re.M)


# --------------------------------------------------------------------------- oracle


class Resolver:
    """A long-lived Node process answering TypeScript resolution queries."""

    def __init__(self) -> None:
        if not (TOOL_DIR / "node_modules" / "typescript" / "package.json").is_file():
            TOOL_DIR.mkdir(parents=True, exist_ok=True)
            subprocess.run(
                # `npm` is `npm.cmd` on Windows, which needs a resolved path.
                [shutil.which("npm") or "npm", "install", "--silent", "--no-audit", "--no-fund", "--ignore-scripts",
                 "--prefix", str(TOOL_DIR), f"typescript@{TYPESCRIPT}"],
                check=True,
            )
        self.process = subprocess.Popen(
            ["node", str(Path(__file__).with_name("tsresolve.cjs")), str(TOOL_DIR)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1,
        )

    def resolve(self, source: Path, spec: str, config: Path | None) -> Path | None:
        assert self.process.stdin and self.process.stdout
        self.process.stdin.write(json.dumps({"from": str(source), "spec": spec,
                                             "config": str(config) if config else None}) + "\n")
        answer = json.loads(self.process.stdout.readline())
        return Path(answer["resolved"]) if answer["resolved"] else None

    def reset(self) -> None:
        assert self.process.stdin and self.process.stdout
        self.process.stdin.write('{"reset": true}\n')
        self.process.stdout.readline()


def glob_regex(pattern: str) -> re.Pattern[str]:
    """The glob crate's default matching: `*` and `**` both cross `/`."""
    out, i = "", 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out, i = out + "(?:.*/)?", i + 3
        elif pattern[i] == "*":
            out, i = out + ".*", i + (2 if pattern.startswith("**", i) else 1)
        elif pattern[i] == "?":
            out, i = out + ".", i + 1
        else:
            out, i = out + re.escape(pattern[i]), i + 1
    return re.compile(out)


def nearest_tsconfig(directory: Path, root: Path) -> Path | None:
    while True:
        candidate = directory / "tsconfig.json"
        if candidate.is_file():
            return candidate
        if directory == root or directory.parent == directory:
            return None
        directory = directory.parent


def is_source(path: Path) -> bool:
    return path.name.endswith(SOURCE_EXTENSIONS)


def package_name_of(specifier: str) -> str:
    parts = specifier.split("/")
    if specifier.startswith("@") and len(parts) > 1 and parts[1]:
        return "/".join(parts[:2])
    return parts[0]


def segment_match(pattern: str, relative: str) -> bool:
    """Glob match where `*` does not cross `/` (workspace patterns)."""
    left = [part for part in pattern.split("/") if part not in ("", ".")]
    right = [part for part in relative.split("/") if part not in ("", ".")]
    if "**" in left:
        return fnmatch.fnmatchcase(relative, pattern.replace("**/", "*"))
    return len(left) == len(right) and all(fnmatch.fnmatchcase(r, l) for l, r in zip(left, right))


def normalize_pattern(text: str) -> tuple[bool, str]:
    """(absolute, spelling) with `./` and trailing `/` removed, as jscycles does."""
    if os.path.isabs(text):
        return True, os.path.normpath(text)
    if WINDOWS:
        text = text.replace("\\", "/")
    while text.startswith("./"):
        text = text[2:].lstrip("/")
    return False, text.rstrip("/") or "."


@dataclass
class Expected:
    packages: dict[str, Path] = field(default_factory=dict)  # selected name -> canonical dir
    file_edges: dict[str, set[tuple[Path, Path]]] = field(default_factory=dict)
    package_edges: set[tuple[str, str]] = field(default_factory=set)
    error: str | None = None


def case_args(root: Path) -> dict:
    path = root / "ARGS.json"
    return json.loads(path.read_text(encoding="utf-8-sig")) if path.is_file() else {}


def cli_args(args: dict) -> list[str]:
    out: list[str] = []
    for name in args.get("only", []):
        out += ["--only", name]
    for name in args.get("exclude", []):
        out += ["--exclude", name]
    if args.get("no_tsconfig"):
        out.append("--no-tsconfig")
    if args.get("extensions"):
        out += ["--extensions", args["extensions"]]
    if args.get("skip_type_imports"):
        out.append("--skip-type-imports")
    return out + list(args.get("operands", []))


def model(root: Path, resolver: Resolver) -> Expected:
    root = root.resolve()
    expected = Expected()
    resolver.reset()
    args = case_args(root)
    config_path = root / "jscycles.yaml"
    config = json.loads(config_path.read_text(encoding="utf-8-sig")) if config_path.is_file() else {}
    patterns = [normalize_pattern(p) for p in config.get("scan", {}).get("exclude", [])]
    ignores = {name: [glob_regex(normalize_pattern(p)[1]) for p in spec.get("ignore", [])]
               for name, spec in config.get("packages", {}).items()}

    def extensions_for(name: str) -> list[str]:
        # --extensions on the command line > packages.<name>.extensions >
        # defaults.extensions > built-in default.
        if args.get("extensions"):
            return args["extensions"].split(",")
        package_extensions = config.get("packages", {}).get(name, {}).get("extensions")
        if package_extensions is not None:
            return package_extensions
        return config.get("defaults", {}).get("extensions", DEFAULT_EXTENSIONS)

    def configured(walk_root: Path, directory: Path) -> bool:
        relative = os.path.relpath(directory, walk_root).replace(os.sep, "/")
        for absolute, pattern in patterns:
            # jscycles matches absolute patterns case-sensitively except for a
            # Windows drive letter.
            if absolute:
                spelled = os.path.normpath(str(directory))
                if WINDOWS:
                    spelled, pattern = spelled[:1].upper() + spelled[1:], pattern[:1].upper() + pattern[1:]
                if spelled == pattern:
                    return True
                continue
            if pattern != "." and fnmatch.fnmatchcase(relative, pattern):
                return True
        return False

    def builtin(walk_root: Path, directory: Path) -> bool:
        relative = os.path.relpath(directory, walk_root).replace(os.sep, "/")
        return any(part in BUILTIN_EXCLUDED for part in relative.split("/"))

    def selected(name: str) -> bool:
        only = args.get("only", [])
        if only and not any(fnmatch.fnmatchcase(name, p) for p in only):
            return False
        return not any(fnmatch.fnmatchcase(name, p) for p in args.get("exclude", []))

    # Discovery from each walk root (the cwd, or explicit directory operands).
    walk_roots = [Path(os.path.normpath(root / op)) for op in args.get("operands", [])] or [root]
    includes = [] if args.get("operands") else [normalize_pattern(p) for p in config.get("scan", {}).get("include", ["."])]

    def included(directory: Path) -> bool:
        if not includes:
            return True
        relative = os.path.relpath(directory, root).replace(os.sep, "/")
        parts = [] if relative == "." else relative.split("/")
        for depth in range(len(parts) + 1):
            ancestor = "/".join(parts[:depth]) or "."
            for absolute, pattern in includes:
                spelled = os.path.normpath(root / ancestor) if absolute else ancestor
                if pattern == "." and spelled == "." or segment_match(pattern, spelled) and pattern != ".":
                    return True
        return False
    discovered: dict[Path, str] = {}  # canonical dir -> name, including filtered
    boundaries: set[Path] = set()
    for walk_root in walk_roots:
        for directory, subdirs, files in os.walk(walk_root):
            here = Path(directory)
            if here != walk_root and (builtin(walk_root, here) or configured(walk_root, here)):
                if configured(walk_root, here) and not builtin(walk_root, here):
                    boundaries.add(here.resolve())
                subdirs[:] = []
                continue
            if "package.json" in files:
                try:
                    manifest = json.loads((here / "package.json").read_text(encoding="utf-8-sig"))
                except ValueError:
                    expected.error = "bad manifest"
                    return expected
                if isinstance(manifest.get("name"), str) and included(here):
                    name = manifest["name"]
                    canonical = here.resolve()
                    if discovered.get(canonical, name) != name:
                        expected.error = "two names"
                        return expected
                    if selected(name):
                        if expected.packages.get(name, canonical) != canonical:
                            expected.error = "duplicate package"
                            return expected
                        expected.packages[name] = canonical
                    else:
                        boundaries.add(canonical)
                    discovered[canonical] = name

    # Workspace members: root package.json workspaces or pnpm-workspace.yaml.
    workspace_patterns: list[str] = []
    root_manifest = root / "package.json"
    pnpm = root / "pnpm-workspace.yaml"
    if pnpm.is_file():
        workspace_patterns = [json.loads(l.strip()[2:]) for l in pnpm.read_text(encoding="utf-8-sig").splitlines() if l.strip().startswith("- ")]
    elif root_manifest.is_file():
        declared = json.loads(root_manifest.read_text(encoding="utf-8-sig")).get("workspaces", []) or []
        workspace_patterns = declared.get("packages", []) if isinstance(declared, dict) else declared
    members: dict[str, Path] = {}
    named_dirs: set[Path] = set()
    for directory, subdirs, files in os.walk(root):
        here = Path(directory)
        if here != root and builtin(root, here):
            subdirs[:] = []
            continue
        if here != root and "package.json" in files:
            relative = os.path.relpath(here, root).replace(os.sep, "/")
            name = json.loads((here / "package.json").read_text(encoding="utf-8-sig")).get("name")
            if isinstance(name, str):
                named_dirs.add(here.resolve())
            positive = [p for p in workspace_patterns if not p.startswith("!")]
            negative = [p[1:] for p in workspace_patterns if p.startswith("!")]
            if (isinstance(name, str) and any(segment_match(normalize_pattern(p)[1], relative) for p in positive)
                    and not any(segment_match(normalize_pattern(p)[1], relative) for p in negative)):
                if name in members and not args.get("inner_only"):
                    expected.error = "duplicate workspace member"
                    return expected
                members[name] = here.resolve()
    workspace = bool(workspace_patterns) and bool(members)

    # Root tsconfig: exact aliases into a member package are workspace aliases.
    tsconfig = root / "tsconfig.json"
    tsconfig_path = tsconfig if tsconfig.is_file() and not args.get("no_tsconfig") else None
    aliases: dict[str, str] = {}
    if workspace and tsconfig_path:
        paths = json.loads(tsconfig.read_text(encoding="utf-8-sig")).get("compilerOptions", {}).get("paths", {})
        for key in paths:
            if "*" in key or key in members:
                continue
            target = resolver.resolve(root / "__probe__.ts", key, tsconfig_path)
            if target is None:
                # R10: an exact alias whose target does not resolve (an unbuilt
                # output, say) still names the member its first replacement
                # lies in.
                replacement = paths[key][0] if paths[key] else None
                if replacement is None:
                    continue
                target = Path(os.path.normpath(root / replacement))
            owners = [(len(str(d)), n) for n, d in members.items() if target.is_relative_to(d)]
            if owners:
                member = members[max(owners)[1]]
                # A named package nearer than the member owns the target.
                if not any(d != member and d.is_relative_to(member) and target.is_relative_to(d) for d in named_dirs):
                    aliases[key] = max(owners)[1]

    def owning_member(target: Path, source: Path, package_dir: Path, nested: set[Path]) -> str | None:
        """The other package that owns `target`, if any: the nearest scanned
        package, workspace member or excluded boundary containing it decides.
        Package edges from files need no workspace."""
        if target.is_relative_to(package_dir) and not any(target.is_relative_to(d) for d in nested):
            return None
        named = list(expected.packages.items()) + (list(members.items()) if workspace else [])
        candidates = [(n, d) for n, d in named if target.is_relative_to(d)]
        if not candidates:
            return None
        owner, owner_dir = max(candidates, key=lambda item: len(item[1].parts))
        if any(target.is_relative_to(r) and len(r.parts) > len(owner_dir.parts) for r in skip):
            return None  # an excluded or filtered-out package that is not a member
        # The location rule (a source inside a member and no nearer member
        # belongs to it) applies to workspace members only.
        own = owner_dir == package_dir or (
            members.get(owner) == owner_dir and source.is_relative_to(owner_dir)
            and not any(d != owner_dir and d.is_relative_to(owner_dir) and source.is_relative_to(d)
                        for d in members.values()))
        return None if own else owner

    def split(specifier: str) -> tuple[str, str | None] | None:
        name = package_name_of(specifier)
        if name in members:
            return name, None
        prefix = specifier
        while True:
            if prefix in aliases:
                return aliases[prefix], None
            if "/" not in prefix:
                return None
            prefix = prefix.rsplit("/", 1)[0]

    # The CLI rejects a scan that selects no package (status 2) rather than
    # reporting it clean.
    if not expected.packages and not expected.error:
        expected.error = "no packages selected"
        return expected

    # Each selected package walks its own directory, leaving nested packages
    # (including filtered ones) and configured-exclude boundaries alone.
    skip = boundaries | set(discovered)
    for name, package_dir in expected.packages.items():
        edges: set[tuple[Path, Path]] = set()
        expected.file_edges[name] = edges
        nested = {d for d in skip if d != package_dir and d.is_relative_to(package_dir)}
        package_tsconfig = None if args.get("no_tsconfig") else nearest_tsconfig(package_dir, root)
        ignored = ignores.get(name, [])
        scanned = extensions_for(name)
        # CLI flag > packages.<name>.skip_type_imports > defaults > off.
        skip_types = bool(args.get("skip_type_imports")) or config.get("packages", {}).get(name, {}).get(
            "skip_type_imports", config.get("defaults", {}).get("skip_type_imports", False))

        def ignored_path(path: Path) -> bool:
            relative = os.path.relpath(path, package_dir).replace(os.sep, "/")
            return any(regex.fullmatch(relative) for regex in ignored)

        for directory, subdirs, files in os.walk(package_dir):
            here = Path(directory)
            if here != package_dir and (builtin(package_dir, here) or here.resolve() in skip or ignored_path(here)):
                subdirs[:] = []
                continue
            for file_name in sorted(files):
                spelled = here / file_name
                # The last suffix, as Path::extension sees it.
                if "." not in file_name or file_name.rsplit(".", 1)[1] not in scanned or ignored_path(spelled):
                    continue
                source = spelled.resolve()
                if spelled.is_symlink():
                    # A link belongs to the nearest owner of its target.
                    owners = [d for d in skip | {package_dir} if source.is_relative_to(d)]
                    if owners and max(owners, key=lambda d: len(d.parts)) != package_dir:
                        continue
                # One tsconfig per package: the nearest at or above the package
                # directory (a config nested inside the package is not consulted).
                tsconfig_path = package_tsconfig
                declaration = re.search(r"\.d\.[mc]?ts$", source.name) is not None
                if skip_types and declaration:
                    continue
                lines = source.read_text(encoding="utf-8-sig").splitlines()
                for line, match in ((line, m) for line in lines for m in SPECIFIER.finditer(line)):
                    if skip_types and line.lstrip().startswith(("import type ", "export type ")):
                        continue
                    specifier = match.group(2)
                    if workspace and not specifier.startswith((".", "/")):
                        split_result = split(specifier)
                        target_dir = members.get(split_result[0]) if split_result else None
                        own = target_dir == package_dir or (
                            target_dir is not None and source.is_relative_to(target_dir)
                            and not any(d != target_dir and d.is_relative_to(target_dir) and source.is_relative_to(d)
                                        for d in members.values()))
                        if split_result and not own:
                            if split_result[0] != name:  # package edges are keyed by name
                                expected.package_edges.add((name, split_result[0]))
                            continue
                    target = resolver.resolve(source, specifier, tsconfig_path)
                    if target is None:
                        continue
                    member = owning_member(Path(os.path.realpath(target)), source, package_dir, nested)
                    if member is not None:
                        # R02: a file in another member, reached by a relative
                        # path or a file alias, is an edge to that member.
                        if member != name:
                            expected.package_edges.add((name, member))
                        continue
                    edges.add((source, target))
    return expected


def sccs(edges: set[tuple[Path, Path]]) -> list[set[Path]]:
    graph: dict[Path, set[Path]] = {}
    for a, b in edges:
        graph.setdefault(a, set()).add(b)
        graph.setdefault(b, set())
    index: dict[Path, int] = {}
    low: dict[Path, int] = {}
    stack: list[Path] = []
    on: set[Path] = set()
    out: list[set[Path]] = []
    counter = [0]

    def strong(v: Path) -> None:
        index[v] = low[v] = counter[0]
        counter[0] += 1
        stack.append(v)
        on.add(v)
        for w in sorted(graph[v]):
            if w not in index:
                strong(w)
                low[v] = min(low[v], low[w])
            elif w in on:
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            component = set()
            while True:
                w = stack.pop()
                on.discard(w)
                component.add(w)
                if w == v:
                    break
            if len(component) > 1 or v in graph[v]:
                out.append(component)

    for v in sorted(graph):
        if v not in index:
            strong(v)
    return out


def elementary_cycles(edges: set[tuple[str, str]]) -> set[tuple[str, ...]]:
    graph: dict[str, set[str]] = {}
    for a, b in edges:
        graph.setdefault(a, set()).add(b)
    found: set[tuple[str, ...]] = set()

    def walk(start: str, node: str, path: list[str]) -> None:
        for nxt in graph.get(node, ()):
            if nxt == start:
                found.add(tuple(path))
            elif nxt > start and nxt not in path:
                walk(start, nxt, path + [nxt])

    for start in graph:
        walk(start, start, [start])
    return found


# --------------------------------------------------------------------------- comparison


STATS: dict[str, int] = collections.defaultdict(int)


def compare(root: Path, binary: Path, resolver: Resolver) -> list[str]:
    """Return mismatch signatures (empty when jscycles agrees with the model)."""
    expected = model(root, resolver)
    STATS["errors"] += bool(expected.error)
    STATS["edges"] += sum(len(e) for e in expected.file_edges.values())
    STATS["package_edges"] += len(expected.package_edges)
    extra = cli_args(case_args(root))
    run = subprocess.run([str(binary), "--json", *extra], cwd=root, capture_output=True, text=True, timeout=60)
    if expected.error:
        return [] if run.returncode == 2 else [f"expected-error:{expected.error}:exit{run.returncode}"]
    if run.returncode == 2:
        return [f"unexpected-error:{run.stderr.strip().splitlines()[-1][:120] if run.stderr.strip() else ''}"]
    report = json.loads(run.stdout)
    problems: list[str] = []
    if "warning: ignored tsconfig" in run.stderr:
        problems.append("tsconfig-warning:" + run.stderr.split("warning: ignored tsconfig", 1)[1].splitlines()[0][:160])
    reported = {entry["name"]: entry for entry in report["file_cycles"]}
    if set(reported) != set(expected.packages):
        problems.append(f"packages:{sorted(reported)}!={sorted(expected.packages)}")
    any_cycle = False
    for name, edges in expected.file_edges.items():
        components = sccs(edges)
        any_cycle |= bool(components)
        entry = reported.get(name)
        if entry is None:
            continue
        base = (root / entry["path"]).resolve()
        cycles = entry["cycles"]
        if len(cycles) != len(components):
            problems.append(f"file-cycle-count:{name}:{len(cycles)}!={len(components)}")
            continue
        for cycle in cycles:
            nodes = [Path(os.path.realpath(base / item)) for item in cycle]
            if nodes[0] != nodes[-1] or not any(set(nodes) <= c for c in components):
                problems.append(f"file-cycle-shape:{name}")
            elif any((a, b) not in edges for a, b in zip(nodes, nodes[1:])):
                problems.append(f"file-cycle-edge:{name}")
    STATS["file_cycles"] += any_cycle
    expected_package = {cycle for cycle in elementary_cycles(expected.package_edges)}
    STATS["package_cycles"] += bool(expected_package)
    actual_package = {tuple(c["packages"][:-1]) for c in report["package_cycles"]}
    any_cycle |= bool(expected_package)
    if expected_package != actual_package:
        problems.append(f"package-cycles:{sorted(actual_package)}!={sorted(expected_package)}")
    if run.returncode != (1 if any_cycle else 0):
        problems.append(f"exit:{run.returncode}")
    # Modes must agree with the combined report on their own half.
    for flag, key in (("--inner", "file_cycles"), ("--outer", "package_cycles")):
        mode = subprocess.run([str(binary), "--json", flag, *extra], cwd=root, capture_output=True, text=True, timeout=60)
        try:
            if json.loads(mode.stdout)[key] != report[key]:
                problems.append(f"mode{flag}:{key}")
        except ValueError:
            problems.append(f"mode{flag}:exit{mode.returncode}")
    return problems


# --------------------------------------------------------------------------- generator


def generate(rng: random.Random, root: Path) -> dict[str, str]:
    files: dict[str, str] = {}
    count = rng.randint(1, 4)
    packages: list[dict] = []
    for i in range(count):
        parent = rng.choice(packages) if packages and rng.random() < 0.2 else None
        directory = f"{parent['dir']}/nested{i}" if parent else f"{rng.choice(['packages', 'apps'])}/p{i}"
        name = f"@s/p{i}" if rng.random() < 0.5 else f"p{i}"
        if packages and rng.random() < 0.03:
            name = packages[0]["name"]  # duplicate names must fail (exit 2)
        packages.append({"dir": directory, "name": name, "files": []})
    root_named = rng.random() < 0.5
    root_pkg = {"dir": ".", "name": "root", "files": []}
    owners = packages + ([root_pkg] if root_named else [])
    for package in owners:
        for n in range(rng.randint(1, 5)):
            ext = rng.choices([".ts", ".tsx", ".js", ".jsx", ".d.ts", ".mts", ".cts", ".mjs", ".cjs", ".d.mts", ".d.cts"],
                              [12, 4, 4, 2, 2, 2, 1, 1, 1, 1, 1])[0]
            shape = rng.choice(["src/f{n}", "src/d{n}/index", "lib/f{n}", "f{n}", "src/deep/f{n}",
                                "src/f{n}.test", "src/a.b{n}"])
            if rng.random() < 0.05:
                shape = rng.choice(["dist/f{n}", "build/f{n}"])
            rel = shape.format(n=n) + ext
            prefix = "" if package["dir"] == "." else package["dir"] + "/"
            package["files"].append(prefix + rel)
    # Workspace declaration lists every member explicitly or by pattern.
    member_dirs = [p["dir"] for p in packages]
    if rng.random() < 0.3:
        patterns = sorted({d.split("/")[0] + "/*" for d in member_dirs if d.count("/") == 1})
        patterns += [d for d in member_dirs if d.count("/") > 1]
    else:
        patterns = [rng.choice(["", "./"]) + d for d in member_dirs]
    if rng.random() < 0.1 and packages:
        patterns.append("!" + rng.choice(member_dirs))
    manifest: dict = {"private": True}
    if root_named:
        manifest["name"] = "root"
    layout = rng.random()
    if layout < 0.1:
        # No workspace: packages are found by discovery alone, and package
        # edges come only from imports into another package's files.
        STATS["no_workspace"] += 1
    elif layout < 0.325:
        files["pnpm-workspace.yaml"] = "packages:\n" + "".join(f"  - {json.dumps(p)}\n" for p in patterns)
    else:
        manifest["workspaces"] = {"packages": patterns} if rng.random() < 0.15 else patterns
    files["package.json"] = json.dumps(manifest)
    for package in packages:
        files[package["dir"] + "/package.json"] = json.dumps({"name": package["name"]})
    # Aliases.
    paths: dict[str, list[str]] = {}
    wildcard: dict[int, str] = {}
    exact: dict[int, str] = {}
    for i, package in enumerate(packages):
        if rng.random() < 0.4:
            wildcard[i] = f"@w{i}/"
            paths[f"@w{i}/*"] = [f"{package['dir']}/src/*"] + ([f"{package['dir']}/lib/*"] if rng.random() < 0.3 else [])
        if rng.random() < 0.3 and package["files"]:
            key = rng.choice([f"@alias/p{i}", f"lib{i}/core", f"@x/y/p{i}"])
            target = rng.choice(package["files"])
            stem = STEM.sub("", target)
            exact[i] = key
            paths[key] = [stem]
    if paths or rng.random() < 0.3:
        if rng.random() < 0.3:
            # Without baseUrl, paths are relative to the config and must be spelled `./`.
            relative_paths = {k: ["./" + v for v in values] for k, values in paths.items()}
            files["tsconfig.json"] = json.dumps({"compilerOptions": {"paths": relative_paths}})
        else:
            files["tsconfig.json"] = json.dumps({"compilerOptions": {"baseUrl": ".", "paths": paths}})
    self_alias: dict[int, str] = {}
    for i, package in enumerate(packages):
        if rng.random() < 0.2:
            own = {"compilerOptions": {"baseUrl": ".", "paths": {f"@self{i}/*": ["src/*"]}}}
            inherit = "tsconfig.json" in files and rng.random() < 0.3
            if inherit:
                own = {}  # inherits the root's paths with their original origin
            if "tsconfig.json" in files and (inherit or rng.random() < 0.5):
                up = "/".join([".."] * package["dir"].count("/")) + "/../tsconfig.json"
                own["extends"] = up if rng.random() < 0.5 else [up]
                # A child `paths` replaces the parent's, as in TypeScript.
            files[package["dir"] + "/tsconfig.json"] = json.dumps(own)
            if not inherit:
                self_alias[i] = f"@self{i}/"
        if rng.random() < 0.05:
            files[package["dir"] + "/src/tsconfig.json"] = json.dumps({"compilerOptions": {}})
    all_files = [f for p in owners for f in p["files"]]
    add_node_modules_base(rng, files, packages)

    def relative_spelling(source: str, target: str) -> str:
        stem = STEM.sub("", target)
        ext = target[len(stem):]
        if stem.endswith("/index") and rng.random() < 0.5:
            stem = stem[: -len("/index")]
            rel = posixpath.relpath(stem, posixpath.dirname(source) or ".")
            rel = rel if rel.startswith(".") else "./" + rel
            return rel + rng.choice(["", "/"])
        # Specifiers are `/`-separated on every platform.
        rel = posixpath.relpath(stem, posixpath.dirname(source) or ".")
        rel = rel if rel.startswith(".") else "./" + rel
        if rng.random() < 0.03:
            head, _, last = rel.rpartition("/")
            rel = head + "/" + last[:1].swapcase() + last[1:]
            STATS["case_variants"] += 1
        mapped = {".ts": ".js", ".tsx": ".jsx", ".d.ts": ".js", ".js": ".js", ".jsx": ".jsx", ".mts": ".mjs",
                  ".cts": ".cjs", ".mjs": ".mjs", ".cjs": ".cjs", ".d.mts": ".mjs", ".d.cts": ".cjs"}[ext]
        return rel + rng.choice(["", "", mapped, ext if not ext.startswith(".d.") else ""])

    for package in owners:
        for source in package["files"]:
            lines = []
            for _ in range(rng.randint(0, 3)):
                kind = rng.choices(["rel", "cross", "pkg", "wild", "exact", "missing"], [8, 1, 3, 2, 2, 1])[0]
                spec = None
                if kind == "rel" and package["files"]:
                    spec = relative_spelling(source, rng.choice(package["files"]))
                elif kind == "cross":
                    spec = relative_spelling(source, rng.choice(all_files))
                elif kind == "pkg":
                    other = rng.choice(packages)
                    spec = other["name"] + rng.choice(["", "", "/sub"])
                elif kind == "wild" and wildcard:
                    i = rng.choice(list(wildcard))
                    src_files = [f for f in packages[i]["files"] if f.startswith(packages[i]["dir"] + "/src/")]
                    if src_files:
                        target = rng.choice(src_files)[len(packages[i]["dir"] + "/src/"):]
                        stem = STEM.sub("", target)
                        spec = wildcard[i] + stem
                elif kind == "exact" and exact:
                    spec = exact[rng.choice(list(exact))]
                elif kind == "wild" and not wildcard and self_alias and package in packages:
                    i = packages.index(package)
                    src_files = [f for f in package["files"] if f.startswith(package["dir"] + "/src/")]
                    if i in self_alias and src_files:
                        target = rng.choice(src_files)[len(package["dir"] + "/src/"):]
                        spec = self_alias[i] + STEM.sub("", target)
                elif kind == "missing":
                    spec = "./missing" + str(rng.randint(0, 9))
                if spec is None:
                    continue
                forms = ["import '{s}';", "export * from '{s}';", "import x{n} from '{s}';",
                         "void import('{s}');", "require('{s}');"]
                if re.search(r"\.[mc]?tsx?$", source):  # type syntax is TypeScript-only
                    forms += ["import type {{ T{n} }} from '{s}';", "export type {{ T{n} }} from '{s}';"]
                form = rng.choice(forms)
                lines.append(form.format(s=spec, n=len(lines)))
            files[source] = "\n".join(lines) + "\n" if lines else "export {};\n"
    # Optional configuration (JSON is valid YAML): scan.exclude in several
    # spellings, and package ignore globs.
    config: dict = {}
    directories = sorted({os.path.dirname(f) for f in all_files if os.path.dirname(f)})
    if rng.random() < 0.2 and directories:
        chosen = rng.choice(directories)
        spellings = [chosen, "./" + chosen, chosen + "/", str(root.resolve() / chosen)]
        if WINDOWS:
            spellings += [".\\" + chosen.replace("/", "\\") + "\\", str(root.resolve() / chosen).lower()[:1] + str(root.resolve() / chosen)[1:]]
        spelling = rng.choice(spellings)
        config["scan"] = {"exclude": [spelling]}
    if rng.random() < 0.15:
        package = rng.choice(owners)
        config["packages"] = {package["name"]: {"ignore": [rng.choice(["**/*.test.ts", "src/deep/**", "lib/*", "src/a.b*"])]}}
    extension_sets = [["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"], ["ts", "mts"], ["ts"], ["ts", "tsx", "mts", "cts"],
                      ["js", "mjs", "cjs"]]
    if rng.random() < 0.08:
        config["defaults"] = {"extensions": rng.choice(extension_sets)}
    if rng.random() < 0.08:
        package = rng.choice(owners)
        config.setdefault("packages", {}).setdefault(package["name"], {})["extensions"] = rng.choice(extension_sets)
    if config:
        files["jscycles.yaml"] = json.dumps(config)
    # Symlinks: a file link or a directory link to elsewhere in the project.
    if rng.random() < 0.15 and all_files:
        target = rng.choice(all_files)
        place = rng.choice(owners)
        link = (place["dir"] + "/" if place["dir"] != "." else "") + "link" + os.path.splitext(target)[1]
        if link not in files:
            files[link] = "@@symlink:" + os.path.relpath(target, os.path.dirname(link) or ".")
    if rng.random() < 0.1 and directories:
        target = rng.choice(directories)
        place = rng.choice(owners)
        link = (place["dir"] + "/" if place["dir"] != "." else "") + "linkdir"
        files[link] = "@@symlink:" + os.path.relpath(target, os.path.dirname(link) or ".")
    # Optional CLI selection: operands in several spellings and name filters.
    args: dict = {}
    if rng.random() < 0.3:
        absolute = str(root.resolve())
        if rng.random() < 0.5:
            parent = packages[0]["dir"].split("/")[0] + "/.."
            args["operands"] = [rng.choice([".", "./", parent, absolute])]
        else:
            chosen = rng.sample(packages, rng.randint(1, len(packages)))
            args["operands"] = [rng.choice([p["dir"], "./" + p["dir"], p["dir"] + "/",
                                            absolute + "/" + p["dir"], p["dir"] + "/../" + p["dir"].split("/")[-1]]
                                           + ([".\\" + p["dir"].replace("/", "\\") + "\\"] if WINDOWS else []))
                                for p in chosen]
    if rng.random() < 0.2:
        names = [p["name"] for p in packages] + (["root"] if root_named else [])
        if rng.random() < 0.5:
            args["exclude"] = [rng.choice(names)]
        else:
            args["only"] = rng.sample(names, rng.randint(1, len(names))) if rng.random() < 0.7 else ["@s/*"]
    if rng.random() < 0.1:
        args["no_tsconfig"] = True
    if rng.random() < 0.12:
        args["extensions"] = ",".join(rng.choice(extension_sets))
    if rng.random() < 0.15:
        args["skip_type_imports"] = True
    if rng.random() < 0.08:
        config.setdefault("defaults", {})["skip_type_imports"] = rng.random() < 0.7
    if rng.random() < 0.05:
        package = rng.choice(owners)
        config.setdefault("packages", {}).setdefault(package["name"], {})["skip_type_imports"] = rng.random() < 0.5
    if config:
        files["jscycles.yaml"] = json.dumps(config)
    if "operands" not in args and rng.random() < 0.1:
        top = packages[0]["dir"].split("/")[0]
        include = rng.choice([top + "/*", top + "/", "./" + top, top + "/*/", str(root.resolve() / top), "."])
        config.setdefault("scan", {})["include"] = [include]
        files["jscycles.yaml"] = json.dumps(config)
    if rng.random() < 0.05:
        for key in list(files):
            if not files[key].startswith("@@symlink:") and key != "ARGS.json" and rng.random() < 0.5:
                files[key] = "\ufeff" + files[key].replace("\n", "\r\n")
                STATS["bom_files"] += 1
    if args:
        files["ARGS.json"] = json.dumps(args)
    STATS["custom_extensions"] += bool(args.get("extensions") or "defaults" in config
                                       or any("extensions" in v for v in config.get("packages", {}).values()))
    STATS["module_sources"] += any(re.search(r"\.[mc][tj]s$", f) for f in all_files)
    return files


def add_node_modules_base(rng: random.Random, files: dict[str, str], packages: list[dict]) -> None:
    """Make some tsconfigs extend a shared base installed under node_modules,
    spelled the ways TypeScript accepts: a file path with or without `.json`, a
    bare package (its `tsconfig.json`, its package.json `tsconfig` field, or its
    `exports`), and an exported subpath. The base sets no paths, so a base that
    fails to load shows up as lost aliases (and a warning)."""
    configs = [key for key in files if key == "tsconfig.json" or re.fullmatch(r"(packages|apps)/[^@]*tsconfig\.json", key)]
    configs = [key for key in configs if "/src/" not in key]
    if not configs or rng.random() > 0.2:
        return
    name = rng.choice(["@cfg/base", "tsconfig-base"])
    home = rng.choice(["", rng.choice(packages)["dir"] + "/"])
    base = f"{home}node_modules/{name}/"
    body = '{\n  // shared base\n  "$schema": "https://json.schemastore.org/tsconfig",\n  "display": "base",\n  "compilerOptions": {"strict": true,},\n}\n'
    variant = rng.choice(["file", "file-noext", "dir", "tsconfig-field", "exports-root", "exports-sub", "exports-conditions",
                          "exports-order"])
    if variant in ("file", "file-noext"):
        files[base + "tsconfig.base.json"] = body
        spec = name + ("/tsconfig.base.json" if variant == "file" else "/tsconfig.base")
        manifest = {"name": name}
    elif variant == "dir":
        files[base + "tsconfig.json"] = body
        spec, manifest = name, {"name": name}
    elif variant == "tsconfig-field":
        files[base + "configs/main.json"] = body
        spec, manifest = name, {"name": name, "tsconfig": "./configs/main.json"}
    elif variant == "exports-root":
        files[base + "configs/main.json"] = body
        spec, manifest = name, {"name": name, "exports": {".": "./configs/main.json"}}
    elif variant == "exports-sub":
        files[base + "configs/strict.json"] = body
        spec, manifest = name + "/strict", {"name": name, "exports": {"./strict": "./configs/strict.json"}}
    elif variant == "exports-order":
        # Conditions are tried in written order; the later one is broken.
        files[base + "configs/main.json"] = body
        files[base + "configs/broken.json"] = "{"
        spec, manifest = name, {"name": name, "exports": {".": {"require": "./configs/main.json", "node": "./configs/broken.json"}}}
    else:
        files[base + "configs/main.json"] = body
        files[base + "index.d.ts"] = "export {};\n"
        spec, manifest = name, {"name": name, "exports": {".": {"types": "./index.d.ts", "default": "./configs/main.json"}}}
    files[base + "package.json"] = json.dumps(manifest)
    STATS["nm_" + variant.replace("-", "_")] += 1
    for key in configs:
        # node_modules is found by walking up from the config's directory.
        if home and not (key.startswith(home) or key == home + "tsconfig.json"):
            continue
        if rng.random() < 0.6:
            document = json.loads(files[key])
            existing = document.get("extends")
            document["extends"] = spec if existing is None else [spec] + (existing if isinstance(existing, list) else [existing])
            files[key] = json.dumps(document)


def materialize(root: Path, files: dict[str, str]) -> None:
    if root.exists():
        shutil.rmtree(root)
    links = []
    for relative, content in files.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        if content.startswith("@@symlink:"):
            links.append((path, content[len("@@symlink:"):]))
        else:
            path.write_text(content, encoding="utf-8", newline="")
    for path, target in links:
        if (path.parent / target).exists() and not path.exists():
            try:
                path.symlink_to(target, target_is_directory=(path.parent / target).is_dir())
            except OSError:
                if not WINDOWS:
                    raise


def shrink(root: Path, files: dict[str, str], signature: str, binary: Path, resolver: Resolver) -> dict[str, str]:
    """Greedy shrink: drop files, then import lines, while the first signature persists."""
    kind = signature.split(":")[0]

    def still_fails(candidate: dict[str, str]) -> bool:
        materialize(root, candidate)
        try:
            return any(p.split(":")[0] == kind for p in compare(root, binary, resolver))
        except (subprocess.TimeoutExpired, ValueError, KeyError):
            return False

    changed = True
    while changed:
        changed = False
        for key in sorted(files):
            if key == "ARGS.json" or "node_modules/" in key:
                continue  # selection and installed bases are not shrunk
            candidate = {k: v for k, v in files.items() if k != key}
            if still_fails(candidate):
                files, changed = candidate, True
        for key in sorted(files):
            lines = files[key].splitlines()
            if len(lines) <= 1 or not is_source(Path(key)):
                continue
            for i in range(len(lines)):
                candidate = dict(files)
                candidate[key] = "\n".join(lines[:i] + lines[i + 1:]) + "\n"
                if still_fails(candidate):
                    files, changed = candidate, True
                    break
    materialize(root, files)
    return files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--cases", type=int, default=200)
    parser.add_argument("--seed", type=int, default=None)
    parser.add_argument("--binary", type=Path, default=REPO / "target" / "debug" / ("jscycles.exe" if WINDOWS else "jscycles"))
    parser.add_argument("--output", type=Path, default=TOOL_DIR / "failures")
    parser.add_argument("--max-failures", type=int, default=5)
    parser.add_argument("--replay", type=int, default=None, help="regenerate one case seed into --into and compare")
    parser.add_argument("--into", type=Path, default=None)
    args = parser.parse_args()
    if args.replay is not None:
        root = (args.into or TOOL_DIR / "replay").resolve()
        materialize(root, generate(random.Random(args.replay), root))
        print("\n".join(compare(root, args.binary, Resolver())) or "agree")
        print(root)
        return 0
    seed = args.seed if args.seed is not None else random.randrange(1 << 30)
    resolver = Resolver()
    args.output.mkdir(parents=True, exist_ok=True)
    failures = 0
    signatures: dict[str, int] = {}
    with tempfile.TemporaryDirectory(prefix="jscycles-diff-") as scratch:
        # Resolved: Windows runners spell TEMP with 8.3 names (RUNNER~1), and
        # absolute selectors are compared lexically with the cwd.
        scratch = Path(scratch).resolve()
        for case in range(args.cases):
            case_seed = seed + case
            root = scratch / "case"
            files = generate(random.Random(case_seed), root)
            materialize(root, files)
            problems = compare(root, args.binary, resolver)
            if not problems:
                continue
            kind = problems[0].split(":")[0]
            signatures[kind] = signatures.get(kind, 0) + 1
            if signatures[kind] > 1:
                continue  # one shrunk example per mismatch kind
            failures += 1
            small = shrink(root, files, problems[0], args.binary, resolver)
            destination = args.output / f"seed-{case_seed}-{kind}"
            materialize(destination, small)
            (destination / "MISMATCH.txt").write_text("\n".join(compare(destination, args.binary, resolver)) + "\n")
            # Uploaded artifacts flatten symlinks, so keep what jscycles saw.
            report = subprocess.run([str(args.binary), "--json", *cli_args(case_args(destination))], cwd=destination,
                                    capture_output=True, text=True, timeout=60)
            (destination / "REPORT.txt").write_text(f"exit {report.returncode}\n{report.stdout}{report.stderr}")
            (destination / "LINKS.txt").write_text("".join(f"{path.relative_to(destination)} -> {os.readlink(path)}\n"
                                                           for path in sorted(destination.rglob("*")) if path.is_symlink()))
            print(f"case {case} seed {case_seed}: {problems[0]} -> {destination}", flush=True)
            if failures >= args.max_failures:
                break
    print(f"seed {seed}: {args.cases} cases, mismatch kinds {signatures or 'none'}; coverage {dict(sorted(STATS.items()))}")
    return 1 if signatures else 0


if __name__ == "__main__":
    sys.exit(main())
