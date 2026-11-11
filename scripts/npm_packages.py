#!/usr/bin/env python3
"""Stage and publish the npm packages from verified release binaries.

`jscycles` is a small Node launcher. Each platform binary is its own package
(`@jscycles/<platform>-<arch>`), listed as an optional dependency with `os` and
`cpu` fields, so npm installs only the one that matches. No package runs an
install script.
"""
import sys

if sys.version_info < (3, 11):
    raise SystemExit('npm_packages.py requires Python 3.11+ (tomllib, hashlib.file_digest)')

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
SCOPE = '@jscycles'
REPOSITORY = {'type': 'git', 'url': 'git+https://github.com/ALMaclaine/jscycles.git'}
# Release artifact name -> (npm platform key, `process.platform`, `process.arch`)
PLATFORMS = {
    'jscycles-linux-x86_64': ('linux-x64', 'linux', 'x64'),
    'jscycles-linux-aarch64': ('linux-arm64', 'linux', 'arm64'),
    'jscycles-darwin-x86_64': ('darwin-x64', 'darwin', 'x64'),
    'jscycles-darwin-aarch64': ('darwin-arm64', 'darwin', 'arm64'),
    'jscycles-windows-x86_64.exe': ('win32-x64', 'win32', 'x64'),
}
LICENSES = ['LICENSE-MIT', 'LICENSE-APACHE']


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def cargo_package(root):
    """The crate's `[package]` table with `field.workspace = true` resolved."""
    manifest = tomllib.loads((root / 'Cargo.toml').read_text())
    inherited = manifest.get('workspace', {}).get('package', {})
    return {key: inherited[key] if value == {'workspace': True} else value
            for key, value in manifest['package'].items()}


def find_artifact(artifacts, name):
    """Accept both the download-artifact layout (`name/name`) and a flat
    directory of release assets (`name`)."""
    for candidate in (artifacts / name / name, artifacts / name):
        if candidate.is_file():
            return candidate
    raise ValueError(f'missing release binary {name} in {artifacts}')


def read_sums(path):
    sums = {}
    for line in path.read_text().splitlines():
        value, _, name = line.partition('  ')
        if len(value) != 64 or not name:
            raise ValueError(f'malformed SHA256SUMS line: {line!r}')
        sums[name] = value
    return sums


def platform_manifest(package, key, platform, arch, version):
    executable = 'jscycles.exe' if platform == 'win32' else 'jscycles'
    return {
        'name': f'{SCOPE}/{key}',
        'version': version,
        'description': f'The jscycles binary for {platform}-{arch}. Install `jscycles` instead.',
        'license': package['license'],
        'repository': REPOSITORY,
        'homepage': package['repository'],
        'os': [platform],
        'cpu': [arch],
        'files': [f'bin/{executable}', *LICENSES],
        'preferUnplugged': True,
    }


def main_manifest(package, version):
    return {
        'name': 'jscycles',
        'version': version,
        'description': package['description'],
        'keywords': ['circular', 'dependencies', 'imports', 'cycles', 'typescript',
                     'javascript', 'monorepo', 'lint'],
        'license': package['license'],
        'repository': REPOSITORY,
        'homepage': package['repository'],
        'bugs': f"{package['repository']}/issues",
        'bin': {'jscycles': 'bin/jscycles.js'},
        'files': ['bin/jscycles.js', *LICENSES],
        'engines': {'node': '>=18'},
        'optionalDependencies': {f'{SCOPE}/{key}': version for key, _, _ in PLATFORMS.values()},
    }


def write_manifest(directory, manifest):
    (directory / 'package.json').write_text(json.dumps(manifest, indent=2) + '\n')


def stage(root, artifacts, out, sums=None):
    """Write every package under `out`; returns the directories in publish
    order (platform packages first, so the launcher never names a missing
    version)."""
    package = cargo_package(root)
    version = package['version']
    expected = read_sums(sums) if sums else None
    if out.exists():
        shutil.rmtree(out)
    staged = []
    for name, (key, platform, arch) in PLATFORMS.items():
        binary = find_artifact(artifacts, name)
        if expected is not None and expected.get(name) != digest(binary):
            raise ValueError(f'{name} does not match SHA256SUMS')
        directory = out / key
        (directory / 'bin').mkdir(parents=True)
        executable = directory / 'bin' / ('jscycles.exe' if platform == 'win32' else 'jscycles')
        shutil.copyfile(binary, executable)
        executable.chmod(0o755)
        for license_file in LICENSES:
            shutil.copyfile(root / license_file, directory / license_file)
        (directory / 'README.md').write_text(
            f'# {SCOPE}/{key}\n\nThe prebuilt [jscycles]({package["repository"]}) binary for '
            f'{platform}-{arch}. Install `jscycles`, which selects this package automatically.\n')
        write_manifest(directory, platform_manifest(package, key, platform, arch, version))
        staged.append(directory)
    main = out / 'jscycles'
    (main / 'bin').mkdir(parents=True)
    launcher = main / 'bin' / 'jscycles.js'
    shutil.copyfile(root / 'npm' / 'jscycles.js', launcher)
    launcher.chmod(0o755)
    for license_file in LICENSES:
        shutil.copyfile(root / license_file, main / license_file)
    shutil.copyfile(root / 'README.md', main / 'README.md')
    write_manifest(main, main_manifest(package, version))
    staged.append(main)
    return staged


def published(name, version, npm='npm'):
    result = subprocess.run([npm, 'view', f'{name}@{version}', 'version'],
                            capture_output=True, text=True, timeout=120)
    return result.returncode == 0 and result.stdout.strip() == version


def publish(directories, npm='npm', dry_run=False):
    """Publish in order, skipping versions already on the registry so a
    re-run after a partial failure finishes the set."""
    for directory in directories:
        manifest = json.loads((directory / 'package.json').read_text())
        if not dry_run and published(manifest['name'], manifest['version'], npm):
            print(f"{manifest['name']}@{manifest['version']} is already published")
            continue
        command = [npm, 'publish', '--access', 'public']
        if dry_run:
            command.append('--dry-run')
        subprocess.run(command, cwd=directory, check=True, timeout=600)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=['stage', 'publish'])
    parser.add_argument('--artifacts', type=Path, default=Path('artifacts'),
                        help='Release binaries, as download-artifact or release assets lay them out')
    parser.add_argument('--sums', type=Path, help='SHA256SUMS to verify the binaries against')
    parser.add_argument('--out', type=Path, default=Path('target/npm'))
    parser.add_argument('--dry-run', action='store_true', help='Pass --dry-run to npm publish')
    args = parser.parse_args()
    directories = stage(ROOT, args.artifacts, args.out, args.sums)
    if args.operation == 'publish':
        publish(directories, dry_run=args.dry_run)
    else:
        for directory in directories:
            print(directory)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        raise SystemExit(str(error)) from error
