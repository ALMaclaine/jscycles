import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import npm_packages  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
NODE = shutil.which('node')


class StageTests(unittest.TestCase):
    def setUp(self):
        self.temp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.temp)
        self.artifacts = self.temp / 'artifacts'
        for name in npm_packages.PLATFORMS:
            (self.artifacts / name).mkdir(parents=True)
            (self.artifacts / name / name).write_bytes(name.encode())
        self.out = self.temp / 'out'
        self.version = npm_packages.cargo_package(ROOT)['version']

    def sums(self, **overrides):
        lines = []
        for name in npm_packages.PLATFORMS:
            value = overrides.get(name, hashlib.sha256(name.encode()).hexdigest())
            lines.append(f'{value}  {name}\n')
        path = self.temp / 'SHA256SUMS'
        path.write_text(''.join(lines))
        return path

    def manifest(self, key):
        return json.loads((self.out / key / 'package.json').read_text())

    def test_platform_packages_come_first_and_carry_one_binary_each(self):
        staged = npm_packages.stage(ROOT, self.artifacts, self.out, self.sums())
        self.assertEqual([path.name for path in staged],
                         ['linux-x64', 'linux-arm64', 'darwin-x64', 'darwin-arm64', 'win32-x64', 'jscycles'])
        linux = self.manifest('linux-x64')
        self.assertEqual((linux['name'], linux['version'], linux['os'], linux['cpu']),
                         ('@jscycles/linux-x64', self.version, ['linux'], ['x64']))
        self.assertEqual(linux['files'], ['bin/jscycles', 'LICENSE-MIT', 'LICENSE-APACHE'])
        self.assertEqual((self.out / 'linux-x64/bin/jscycles').read_bytes(), b'jscycles-linux-x86_64')
        self.assertTrue(os.access(self.out / 'darwin-arm64/bin/jscycles', os.X_OK))
        self.assertEqual(self.manifest('win32-x64')['files'][0], 'bin/jscycles.exe')
        for manifest in map(self.manifest, ['linux-x64', 'win32-x64', 'jscycles']):
            self.assertNotIn('scripts', manifest)
            self.assertEqual(manifest['license'], 'MIT OR Apache-2.0')
            self.assertEqual(manifest['repository']['url'], 'git+https://github.com/ALMaclaine/jscycles.git')

    def test_main_package_depends_on_every_platform_at_its_own_version(self):
        npm_packages.stage(ROOT, self.artifacts, self.out)
        main = self.manifest('jscycles')
        self.assertEqual(main['version'], self.version)
        self.assertEqual(main['bin'], {'jscycles': 'bin/jscycles.js'})
        self.assertEqual(main['optionalDependencies'],
                         {f'@jscycles/{key}': self.version for key, _, _ in npm_packages.PLATFORMS.values()})
        self.assertEqual((self.out / 'jscycles/bin/jscycles.js').read_bytes(),
                         (ROOT / 'npm/jscycles.js').read_bytes())

    def test_release_asset_layout_is_accepted(self):
        flat = self.temp / 'flat'
        flat.mkdir()
        for name in npm_packages.PLATFORMS:
            shutil.copyfile(self.artifacts / name / name, flat / name)
        npm_packages.stage(ROOT, flat, self.out, self.sums())
        self.assertTrue((self.out / 'win32-x64/bin/jscycles.exe').is_file())

    def test_missing_or_altered_binary_is_rejected(self):
        (self.artifacts / 'jscycles-darwin-x86_64' / 'jscycles-darwin-x86_64').unlink()
        with self.assertRaisesRegex(ValueError, 'missing release binary jscycles-darwin-x86_64'):
            npm_packages.stage(ROOT, self.artifacts, self.out)
        (self.artifacts / 'jscycles-darwin-x86_64' / 'jscycles-darwin-x86_64').write_bytes(b'other')
        with self.assertRaisesRegex(ValueError, 'jscycles-darwin-x86_64 does not match SHA256SUMS'):
            npm_packages.stage(ROOT, self.artifacts, self.out, self.sums())


@unittest.skipUnless(NODE and os.name == 'posix', 'needs Node.js and a POSIX shell')
class LauncherTests(unittest.TestCase):
    def run_launcher(self, binary, *arguments):
        environment = {**os.environ, 'JSCYCLES_BINARY': str(binary)}
        return subprocess.run([NODE, str(ROOT / 'npm/jscycles.js'), *arguments],
                              capture_output=True, text=True, env=environment, timeout=60)

    def test_arguments_output_and_exit_status_pass_through(self):
        with tempfile.TemporaryDirectory() as temp:
            binary = Path(temp) / 'jscycles'
            binary.write_text('#!/bin/sh\nprintf "%s|" "$@"\necho oops >&2\nexit 1\n')
            binary.chmod(0o755)
            result = self.run_launcher(binary, '--json', 'a b')
        self.assertEqual((result.returncode, result.stdout, result.stderr), (1, '--json|a b|', 'oops\n'))

    def test_a_binary_that_cannot_run_exits_two(self):
        result = self.run_launcher('/nonexistent/jscycles', '--version')
        self.assertEqual(result.returncode, 2)
        self.assertIn('could not run /nonexistent/jscycles', result.stderr)

    def test_a_missing_platform_package_exits_two(self):
        environment = {key: value for key, value in os.environ.items() if key != 'JSCYCLES_BINARY'}
        with tempfile.TemporaryDirectory() as temp:
            launcher = Path(temp) / 'jscycles.js'
            shutil.copyfile(ROOT / 'npm/jscycles.js', launcher)
            result = subprocess.run([NODE, str(launcher)], capture_output=True, text=True,
                                    env=environment, cwd=temp, timeout=60)
        self.assertEqual(result.returncode, 2)
        self.assertIn('package is not installed', result.stderr)


if __name__ == '__main__':
    unittest.main()
