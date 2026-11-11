"""Release inventory, tag authorization, source consistency, and digest contracts."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import subprocess
from release_record import ARTIFACTS, binary_identity, build_record, collect_records, digest, elf_interpreter, release_identity


def executable_header(name):
    """Minimal format headers; deliberately not executable fixture programs."""
    header = bytearray(64)
    if '-linux-' in name:
        header[:7] = b'\x7fELF\x02\x01\x01'
        header[18:20] = (183 if name.endswith('aarch64') else 62).to_bytes(2, 'little')
    elif '-darwin-' in name:
        header[:4] = b'\xcf\xfa\xed\xfe'
        header[4:8] = (0x0100000c if name.endswith('aarch64') else 0x01000007).to_bytes(4, 'little')
    else:
        header[:2] = b'MZ'
        header[60:64] = (64).to_bytes(4, 'little')
        header += b'PE\0\0\x64\x86' + bytes(18) + b'\x0b\x02'
    return bytes(header)


EXPECTED = {
    'jscycles-linux-x86_64': ({'format': 'ELF64', 'arch': 'x86_64'}, {'os': 'Linux', 'arch': 'X64'}),
    'jscycles-linux-aarch64': ({'format': 'ELF64', 'arch': 'aarch64'}, {'os': 'Linux', 'arch': 'ARM64'}),
    'jscycles-darwin-x86_64': ({'format': 'Mach-O64', 'arch': 'x86_64'}, {'os': 'macOS', 'arch': 'X64'}),
    'jscycles-darwin-aarch64': ({'format': 'Mach-O64', 'arch': 'aarch64'}, {'os': 'macOS', 'arch': 'ARM64'}),
    'jscycles-windows-x86_64.exe': ({'format': 'PE32+', 'arch': 'x86_64'}, {'os': 'Windows', 'arch': 'X64'}),
}

SMOKE = {'has_cycles': True, 'has_file_cycles': True, 'has_package_cycles': False,
         'file_cycles': [{'name': '@test/simple-cycle', 'path': 'fixture', 'cycles': [['src/a.ts', 'src/b.ts', 'src/a.ts']]}],
         'package_cycles': []}

class ReleaseRecordTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / 'Cargo.toml').write_text('[package]\nversion="0.2.0"\n')
        (self.root / 'Cargo.lock').write_text('lock graph')
        self.env = dict(GITHUB_REF_TYPE='tag', GITHUB_REF_NAME='v0.2.0', GITHUB_SHA='a'*40,
                        GITHUB_WORKFLOW_REF='owner/repo/.github/workflows/release.yml@refs/tags/v0.2.0',
                        GITHUB_REPOSITORY='owner/repo', GITHUB_RUN_ID='10', GITHUB_RUN_ATTEMPT='1', GITHUB_ACTOR='maintainer')
        self.artifacts = self.root / 'artifacts'
        for name, target in ARTIFACTS.items():
            folder = self.artifacts / name
            folder.mkdir(parents=True)
            binary = folder / name
            binary.write_bytes(executable_header(name))
            binary_format, runner = EXPECTED[name]
            record = dict(release_identity(self.root, self.env), artifact=name, target=target,
                          sha256=digest(binary), smoke_verified=True, binary_identity=binary_format,
                          runner=runner, binary_version='jscycles 0.2.0')
            (folder / 'build-info.json').write_text(json.dumps(record))

    def test_complete_inventory_passes(self):
        self.assertEqual(len(collect_records(self.root, self.artifacts, self.env)['artifacts']), 5)

    def test_missing_artifact_is_rejected_then_repaired(self):
        name = next(iter(ARTIFACTS)); path = self.artifacts / name / name; original = path.read_bytes()
        path.unlink()
        with self.assertRaisesRegex(ValueError, 'inventory'): collect_records(self.root, self.artifacts, self.env)
        path.write_bytes(original)
        self.assertEqual(len(collect_records(self.root, self.artifacts, self.env)['artifacts']), 5)

    def test_modified_binary_is_rejected(self):
        name = next(iter(ARTIFACTS)); (self.artifacts / name / name).write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'digest'): collect_records(self.root, self.artifacts, self.env)

    def test_mixed_revision_is_rejected(self):
        name = next(iter(ARTIFACTS)); path = self.artifacts / name / 'build-info.json'; record = json.loads(path.read_text())
        record['source_revision'] = 'b'*40; path.write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, 'identity'): collect_records(self.root, self.artifacts, self.env)

    def test_partial_rerun_accepts_earlier_attempt_of_the_same_run(self):
        later = dict(self.env, GITHUB_RUN_ATTEMPT='2')
        self.assertEqual(len(collect_records(self.root, self.artifacts, later)['artifacts']), 5)

    def test_other_run_or_future_attempt_is_rejected(self):
        for change in (dict(GITHUB_RUN_ID='11'), dict(GITHUB_RUN_ATTEMPT='0'), dict(GITHUB_REPOSITORY='fork/repo')):
            with self.subTest(change=change):
                with self.assertRaisesRegex(ValueError, 'identity'):
                    collect_records(self.root, self.artifacts, dict(self.env, **change))

    def test_branch_or_wrong_tag_cannot_replace_release(self):
        for change in [dict(GITHUB_REF_TYPE='branch'), dict(GITHUB_REF_NAME='v9.0.0')]:
            with self.assertRaisesRegex(ValueError, 'matching version tag'): release_identity(self.root, dict(self.env, **change))

    def test_unverified_binary_is_rejected(self):
        name = next(iter(ARTIFACTS)); path = self.artifacts / name / 'build-info.json'; record = json.loads(path.read_text())
        record['smoke_verified'] = False; path.write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, 'build record'): collect_records(self.root, self.artifacts, self.env)

    def test_native_format_and_architecture_are_read_from_all_five_headers(self):
        for name, (expected, _) in EXPECTED.items():
            with self.subTest(name=name):
                self.assertEqual(binary_identity(self.artifacts / name / name), expected)

    def test_malformed_or_unsupported_headers_are_rejected(self):
        elf = executable_header('jscycles-linux-x86_64')
        mach = executable_header('jscycles-darwin-aarch64')
        pe = executable_header('jscycles-windows-x86_64.exe')
        samples = [b'', b'not a binary', elf[:63], mach[:31], pe[:89],
                   elf[:4] + b'\x01' + elf[5:], elf[:5] + b'\x02' + elf[6:],
                   elf[:18] + b'\x03\0' + elf[20:], mach[:4] + bytes(4) + mach[8:],
                   mach[:4][::-1] + mach[4:], pe[:60] + bytes(4) + pe[64:],
                   pe[:60] + b'\xff\xff\xff\xff' + pe[64:],
                   pe[:64] + b'PX' + pe[66:], pe[:68] + b'\x4c\x01' + pe[70:],
                   pe[:88] + b'\x0b\x01']
        binary = self.root / 'bad-binary'
        for data in samples:
            with self.subTest(header=data.hex()):
                binary.write_bytes(data)
                with self.assertRaisesRegex(ValueError, 'header'): binary_identity(binary)

    def test_renamed_architecture_is_rejected_even_with_matching_digest(self):
        name = 'jscycles-linux-x86_64'
        binary = self.artifacts / name / name
        binary.write_bytes(executable_header('jscycles-linux-aarch64'))
        self.change_record(name, sha256=digest(binary))
        with self.assertRaisesRegex(ValueError, 'binary identity'): collect_records(self.root, self.artifacts, self.env)

    def change_record(self, name, **updates):
        path = self.artifacts / name / 'build-info.json'
        record = json.loads(path.read_text())
        record.update(updates)
        path.write_text(json.dumps(record))

    def test_recorded_format_runner_and_version_are_checked_independently(self):
        name = 'jscycles-darwin-aarch64'
        path = self.artifacts / name / 'build-info.json'
        original = path.read_text()
        for change in [{'binary_identity': {'format': 'ELF64', 'arch': 'aarch64'}},
                       {'runner': {'os': 'macOS', 'arch': 'X64'}}, {'binary_version': 'jscycles 9.0.0'}]:
            with self.subTest(change=change):
                path.write_text(original)
                self.change_record(name, **change)
                with self.assertRaisesRegex(ValueError, 'binary identity'): collect_records(self.root, self.artifacts, self.env)

    def build(self, name='jscycles-linux-x86_64', outputs=None, runner=None, mutate=False):
        _, expected_runner = EXPECTED[name]
        selected_runner = runner or expected_runner
        environment = dict(self.env, RUNNER_OS=selected_runner['os'], RUNNER_ARCH=selected_runner['arch'])
        binary = self.artifacts / name / name
        outputs = outputs or [(0, b'jscycles 0.2.0\n', b''), (0, b'Usage: jscycles --json --version\n', b''),
                              (1, json.dumps(SMOKE).encode(), b'')]
        calls = iter(outputs)

        def run(command, **kwargs):
            code, stdout, stderr = next(calls)
            if mutate and '--json' in command:
                binary.write_bytes(binary.read_bytes() + b'replaced')
            if kwargs.get('check') and code:
                raise subprocess.CalledProcessError(code, command, stdout, stderr)
            return subprocess.CompletedProcess(command, code, stdout, stderr)

        with patch('release_record.subprocess.run', side_effect=run), patch('release_record.subprocess.check_output', return_value='rustc fixture'):
            return build_record(self.root, binary, name, environment)

    def test_build_checks_all_native_targets(self):
        for name in ARTIFACTS:
            with self.subTest(name=name):
                record = self.build(name)
                self.assertEqual(record['binary_version'], 'jscycles 0.2.0')
                self.assertEqual(record['binary_identity'], EXPECTED[name][0])
                self.assertEqual(record['runner'], EXPECTED[name][1])
                self.assertTrue(record['smoke_verified'])

    def test_linux_binaries_must_be_static(self):
        name = 'jscycles-linux-aarch64'
        binary = self.artifacts / name / name
        header = bytearray(executable_header(name))
        header[32:40] = (64).to_bytes(8, 'little')
        header[54:56] = (56).to_bytes(2, 'little')
        header[56:58] = (2).to_bytes(2, 'little')
        load, interp = bytearray(56), bytearray(56)
        load[:4] = (1).to_bytes(4, 'little')
        interp[:4] = (3).to_bytes(4, 'little')
        binary.write_bytes(bytes(header) + load + load)
        self.assertFalse(elf_interpreter(binary))
        self.assertEqual(self.build(name)['target'], 'aarch64-unknown-linux-musl')
        binary.write_bytes(bytes(header) + load + interp)
        self.assertTrue(elf_interpreter(binary))
        with self.assertRaisesRegex(ValueError, 'statically linked'):
            self.build(name)
        binary.write_bytes(bytes(header) + load)
        with self.assertRaisesRegex(ValueError, 'Truncated ELF program header'):
            elf_interpreter(binary)
        header[54:56] = (32).to_bytes(2, 'little')
        binary.write_bytes(bytes(header) + load + load)
        with self.assertRaisesRegex(ValueError, 'program header size'):
            elf_interpreter(binary)

    def test_successful_wrong_version_or_extra_output_is_rejected(self):
        for version, stderr in [(b'jscycles 9.0.0\n', b''), (b'other 0.2.0\n', b''),
                                (b'jscycles 0.2.0\nextra', b''), (b'jscycles 0.2.0\n', b'warning')]:
            with self.subTest(version=version, stderr=stderr):
                with self.assertRaisesRegex(ValueError, 'version'):
                    self.build(outputs=[(0, version, stderr)])

    def test_successful_empty_or_unrelated_help_is_rejected(self):
        for help_text in [b'', b'help for some other program', b'Usage: jscycles --version']:
            with self.subTest(help=help_text):
                with self.assertRaisesRegex(ValueError, 'help'):
                    self.build(outputs=[(0, b'jscycles 0.2.0\n', b''), (0, help_text, b'')])

    def test_native_runner_and_binary_target_must_agree(self):
        with self.assertRaisesRegex(ValueError, 'native runner'):
            self.build(runner={'os': 'Linux', 'arch': 'ARM64'})
        name = 'jscycles-linux-x86_64'
        (self.artifacts / name / name).write_bytes(executable_header('jscycles-darwin-x86_64'))
        with self.assertRaisesRegex(ValueError, 'architecture/format'): self.build()

    def test_binary_replaced_during_smokes_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'changed during'): self.build(mutate=True)

    def test_smoke_requires_cycle_status_and_typed_agreeing_json(self):
        for status, report in [(0, SMOKE), (2, SMOKE), (1, []), (1, dict(SMOKE, has_cycles='yes')),
                               (1, dict(SMOKE, has_file_cycles=1)), (1, dict(SMOKE, has_package_cycles=0)),
                               (1, dict(SMOKE, file_cycles=[])), (1, dict(SMOKE, package_cycles=[{}])),
                               (1, dict(SMOKE, file_cycles={'invented': True})),
                               (1, dict(SMOKE, file_cycles=[{}])),
                               (1, dict(SMOKE, file_cycles=[{'name': '@test/simple-cycle', 'cycles': [[1, 2, 1]]}])),
                               (1, dict(SMOKE, file_cycles=[{'name': '@test/simple-cycle', 'cycles': [['a', 'b', 'a']]}]))]:
            with self.subTest(status=status, report=report):
                with self.assertRaisesRegex(ValueError, 'known-cycle'):
                    self.build(outputs=[(0, b'jscycles 0.2.0\n', b''), (0, b'Usage: --json --version', b''),
                                        (status, json.dumps(report).encode(), b'')])

    def test_windows_native_path_separators_preserve_the_known_cycle(self):
        report = dict(SMOKE, file_cycles=[{'name': '@test/simple-cycle', 'cycles': [['src\\a.ts', 'src\\b.ts', 'src\\a.ts']]}])
        record = self.build('jscycles-windows-x86_64.exe', outputs=[(0, b'jscycles 0.2.0\n', b''),
                            (0, b'Usage: --json --version', b''), (1, json.dumps(report).encode(), b'')])
        self.assertTrue(record['smoke_verified'])

    def test_failed_version_process_cannot_create_a_record(self):
        with self.assertRaises(subprocess.CalledProcessError): self.build(outputs=[(2, b'', b'failed')])
