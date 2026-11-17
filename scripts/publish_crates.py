"""Publish every publishable workspace crate to crates.io, dependencies first.

A crate whose current version is already on crates.io is skipped, so a run
that stopped part way (a rate limit, a network failure) is resumed by running
it again. crates.io limits how fast new crates can be created; a rate-limited
publish is retried after a wait instead of failing the release.

    python3 -B scripts/publish_crates.py [--dry-run]
"""
import argparse
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request

USER_AGENT = 'jscycles-release (github.com/ALMaclaine/jscycles)'
RATE_LIMIT_WAIT_SECONDS = 600
RATE_LIMIT_ATTEMPTS = 12


def publish_order(metadata):
    """Publishable workspace packages, each after the workspace packages it
    depends on (normal and build dependencies; dev-dependencies need not be
    published first)."""
    members = set(metadata['workspace_members'])
    packages = {package['id']: package for package in metadata['packages'] if package['id'] in members}
    publishable = {identifier: package for identifier, package in packages.items()
                   if package.get('publish') is None or package['publish']}
    by_name = {package['name']: identifier for identifier, package in publishable.items()}
    order, visiting, done = [], set(), set()

    def visit(identifier):
        if identifier in done:
            return
        if identifier in visiting:
            raise ValueError(f'dependency cycle through {publishable[identifier]["name"]}')
        visiting.add(identifier)
        for dependency in sorted(publishable[identifier]['dependencies'], key=lambda d: d['name']):
            if dependency.get('kind') == 'dev':
                continue
            target = by_name.get(dependency['name'])
            if target is not None:
                visit(target)
        visiting.discard(identifier)
        done.add(identifier)
        order.append(publishable[identifier])

    for identifier in sorted(publishable, key=lambda i: publishable[i]['name']):
        visit(identifier)
    return [(package['name'], package['version']) for package in order]


def is_published(name, version):
    request = urllib.request.Request(f'https://crates.io/api/v1/crates/{name}/{version}',
                                     headers={'User-Agent': USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=30):
            return True
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return False
        raise


def rate_limited(output):
    return '429' in output or 'rate limit' in output.lower() or 'too many' in output.lower()


def publish(name, dry_run, run=subprocess.run, sleep=time.sleep):
    command = ['cargo', 'publish', '--locked', '--package', name] + (['--dry-run'] if dry_run else [])
    for attempt in range(1, RATE_LIMIT_ATTEMPTS + 1):
        result = run(command, capture_output=True, text=True)
        sys.stdout.write(result.stdout)
        sys.stderr.write(result.stderr)
        if result.returncode == 0:
            return
        if not rate_limited(result.stdout + result.stderr) or attempt == RATE_LIMIT_ATTEMPTS:
            raise SystemExit(f'publishing {name} failed')
        print(f'{name}: rate limited; retrying in {RATE_LIMIT_WAIT_SECONDS} s', flush=True)
        sleep(RATE_LIMIT_WAIT_SECONDS)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--dry-run', action='store_true', help='package and verify without uploading')
    args = parser.parse_args()
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'], text=True))
    for name, version in publish_order(metadata):
        if not args.dry_run and is_published(name, version):
            print(f'{name} {version} is already on crates.io', flush=True)
            continue
        publish(name, args.dry_run)


if __name__ == '__main__':
    main()
