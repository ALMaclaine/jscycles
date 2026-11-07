#!/usr/bin/env python3
"""Build and verify release identity records; never publish artifacts."""
import sys

if sys.version_info < (3, 11):
    raise SystemExit('release_record.py requires Python 3.11+ (tomllib, hashlib.file_digest)')

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import tomllib

ARTIFACTS = {
    'jscycles-linux-x86_64': 'x86_64-unknown-linux-gnu',
    'jscycles-linux-aarch64': 'aarch64-unknown-linux-gnu',
    'jscycles-darwin-x86_64': 'x86_64-apple-darwin',
    'jscycles-darwin-aarch64': 'aarch64-apple-darwin',
    'jscycles-windows-x86_64.exe': 'x86_64-pc-windows-msvc',
}
TARGETS = {
    'x86_64-unknown-linux-gnu': ('ELF64', 'x86_64', 'Linux', 'X64'),
    'aarch64-unknown-linux-gnu': ('ELF64', 'aarch64', 'Linux', 'ARM64'),
    'x86_64-apple-darwin': ('Mach-O64', 'x86_64', 'macOS', 'X64'),
    'aarch64-apple-darwin': ('Mach-O64', 'aarch64', 'macOS', 'ARM64'),
    'x86_64-pc-windows-msvc': ('PE32+', 'x86_64', 'Windows', 'X64'),
}

def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()

def binary_identity(path):
    """Read format/CPU headers, not an ABI or executable-validity proof.

    Reads are fixed-size, including the offset-selected PE header. Thin, little
    endian 64-bit images are the only release formats this project distributes.
    """
    with path.open('rb') as source:
        metadata = os.fstat(source.fileno())
        if not stat.S_ISREG(metadata.st_mode):
            raise ValueError('Release binary must be a regular file')
        header = source.read(64)
        if len(header) == 64 and header[:7] == b'\x7fELF\x02\x01\x01':
            architecture = {62: 'x86_64', 183: 'aarch64'}.get(int.from_bytes(header[18:20], 'little'))
            if architecture:
                return {'format': 'ELF64', 'arch': architecture}
        if len(header) >= 32 and header[:4] == b'\xcf\xfa\xed\xfe':
            architecture = {0x01000007: 'x86_64', 0x0100000c: 'aarch64'}.get(int.from_bytes(header[4:8], 'little'))
            if architecture:
                return {'format': 'Mach-O64', 'arch': architecture}
        if len(header) == 64 and header[:2] == b'MZ':
            offset = int.from_bytes(header[60:64], 'little')
            if 64 <= offset <= metadata.st_size - 26:
                source.seek(offset)
                pe = source.read(26)
                if len(pe) == 26 and pe[:6] == b'PE\0\0\x64\x86' and pe[24:26] == b'\x0b\x02':
                    return {'format': 'PE32+', 'arch': 'x86_64'}
    raise ValueError('Unsupported or truncated release binary header')

def target_identity(target):
    binary_format, architecture, runner_os, runner_arch = TARGETS[target]
    return {'format': binary_format, 'arch': architecture}, {'os': runner_os, 'arch': runner_arch}

def known_cycle(report):
    if not isinstance(report, dict):
        return False
    if (report.get('has_cycles') is not True or report.get('has_file_cycles') is not True
            or report.get('has_package_cycles') is not False or report.get('package_cycles') != []):
        return False
    files = report.get('file_cycles')
    if not isinstance(files, list) or len(files) != 1 or not isinstance(files[0], dict):
        return False
    if files[0].get('name') != '@test/simple-cycle':
        return False
    cycles = files[0].get('cycles')
    if not isinstance(cycles, list) or len(cycles) != 1 or not isinstance(cycles[0], list):
        return False
    if not all(isinstance(path, str) for path in cycles[0]):
        return False
    return [path.replace('\\', '/') for path in cycles[0]] == ['src/a.ts', 'src/b.ts', 'src/a.ts']

def release_identity(root, environment):
    version = 'v' + tomllib.loads((root / 'Cargo.toml').read_text())['package']['version']
    if environment.get('GITHUB_REF_TYPE') != 'tag' or environment.get('GITHUB_REF_NAME') != version:
        raise ValueError('Release requires the matching version tag; branch dispatch cannot replace a release')
    revision = environment['GITHUB_SHA']
    if len(revision) != 40 or any(char not in '0123456789abcdef' for char in revision):
        raise ValueError('Release requires a full source revision')
    return {'schema': 2, 'version': version, 'source_revision': revision,
            'lock_sha256': digest(root / 'Cargo.lock'),
            'build_definition': environment['GITHUB_WORKFLOW_REF'],
            'builder': {'repository': environment['GITHUB_REPOSITORY'],
                        'run_id': environment['GITHUB_RUN_ID'],
                        'run_attempt': environment['GITHUB_RUN_ATTEMPT']},
            'authorization_actor': environment['GITHUB_ACTOR']}

def build_record(root, binary, name, environment):
    identity = release_identity(root, environment)
    if name not in ARTIFACTS:
        raise ValueError('Unknown release artifact')
    target = ARTIFACTS[name]
    expected_binary, expected_runner = target_identity(target)
    if binary_identity(binary) != expected_binary:
        raise ValueError('Release binary architecture/format disagrees with its target')
    runner = {'os': environment['RUNNER_OS'], 'arch': environment['RUNNER_ARCH']}
    if runner != expected_runner:
        raise ValueError('Release binary must be smoke tested on its native runner')
    before = digest(binary)
    version = subprocess.run([str(binary.resolve()), '--version'], check=True, capture_output=True, timeout=30)
    expected_version = f"jscycles {identity['version'][1:]}"
    if version.stdout != (expected_version + '\n').encode() or version.stderr:
        raise ValueError('Release binary version disagrees with Cargo.toml')
    help_output = subprocess.run([str(binary.resolve()), '--help'], check=True, capture_output=True, timeout=30)
    if help_output.stderr or any(token not in help_output.stdout for token in [b'Usage:', b'--json', b'--version']):
        raise ValueError('Release binary failed its help smoke test')
    scan = subprocess.run([str(binary.resolve()), '--inner', '--json', '--no-tsconfig',
                           str(root / 'tests/fixtures/simple-cycle')], capture_output=True, timeout=30)
    report = json.loads(scan.stdout)
    if scan.returncode != 1 or scan.stderr or not known_cycle(report):
        raise ValueError('Release binary failed its known-cycle smoke test')
    compiler = subprocess.check_output(['rustc', '-vV'], text=True, timeout=30)
    if digest(binary) != before:
        raise ValueError('Release binary changed during smoke tests')
    identity.update(artifact=name, target=target, sha256=before, rustc=compiler,
                    runner=runner, binary_identity=expected_binary,
                    binary_version=expected_version, smoke_verified=True)
    return identity

def same_release_build(record, identity):
    """Every identity field must match, except that artifacts may come from an
    earlier attempt of the same workflow run ("Re-run failed jobs" reuses them)."""
    for key, value in identity.items():
        if key != 'builder' and record.get(key) != value:
            return False
    builder, current = record.get('builder'), identity['builder']
    if not isinstance(builder, dict) or set(builder) != set(current):
        return False
    if builder['repository'] != current['repository'] or builder['run_id'] != current['run_id']:
        return False
    attempt = builder['run_attempt']
    return (isinstance(attempt, str) and attempt.isdigit() and attempt.isascii()
            and 1 <= int(attempt) <= int(current['run_attempt']))

def collect_records(root, artifacts, environment):
    identity = release_identity(root, environment)
    actual = {path.relative_to(artifacts).as_posix() for path in artifacts.rglob('*') if path.is_file()}
    expected = {f'{name}/{file}' for name in ARTIFACTS for file in (name, 'build-info.json')}
    if actual != expected:
        raise ValueError(f'Artifact inventory mismatch: missing={sorted(expected-actual)}, unexpected={sorted(actual-expected)}')
    records = {}
    for name, target in ARTIFACTS.items():
        record = json.loads((artifacts / name / 'build-info.json').read_text())
        if not same_release_build(record, identity):
            raise ValueError(f'Mixed source/build identity for {name}')
        if record.get('artifact') != name or record.get('target') != target or record.get('smoke_verified') is not True:
            raise ValueError(f'Invalid build record for {name}')
        if record.get('sha256') != digest(artifacts / name / name):
            raise ValueError(f'Artifact digest mismatch for {name}')
        expected_binary, expected_runner = target_identity(target)
        if (binary_identity(artifacts / name / name) != expected_binary
                or record.get('binary_identity') != expected_binary
                or record.get('runner') != expected_runner
                or record.get('binary_version') != f"jscycles {identity['version'][1:]}"):
            raise ValueError(f'Invalid binary identity for {name}')
        records[name] = record
    return {**identity, 'artifacts': records,
            'verification': 'Required reusable CI plus native release-binary smoke tests; no SLSA/reproducibility claim'}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=['build', 'collect'])
    parser.add_argument('--root', type=Path, default=Path('.'))
    parser.add_argument('--artifacts', type=Path, default=Path('artifacts'))
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--name')
    args = parser.parse_args()
    if args.operation == 'build':
        record = build_record(args.root, args.binary, args.name, os.environ)
        Path('build-info.json').write_text(json.dumps(record, indent=2) + '\n')
    else:
        record = collect_records(args.root, args.artifacts, os.environ)
        Path('release-metadata.json').write_text(json.dumps(record, indent=2) + '\n')
        Path('SHA256SUMS').write_text(''.join(f"{entry['sha256']}  {name}\n" for name, entry in sorted(record['artifacts'].items())))

if __name__ == '__main__':
    main()
