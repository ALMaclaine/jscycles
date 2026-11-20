#!/usr/bin/env python3
"""Run an owned bounded fuzz target without changing its seed corpus."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

TOOL = '0.13.2'
NIGHTLY = 'nightly-2026-06-20'

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', choices=['configuration', 'source_syntax', 'text_adapters', 'graph_semantics'], default='configuration')
    parser.add_argument('--seconds', type=int, default=30)
    parser.add_argument('--replay', type=Path)
    parser.add_argument('--output', type=Path, default=Path('target/tooling/fuzz/report.json'))
    args = parser.parse_args()
    if not 1 <= args.seconds <= 600: parser.error('select 1..600 seconds')
    root = Path(__file__).resolve().parents[1]
    version = subprocess.check_output(['cargo', 'fuzz', '--version'], cwd=root, text=True).strip()
    if version != f'cargo-fuzz {TOOL}': raise SystemExit(f'Install cargo-fuzz {TOOL} with cargo +1.98.1 install cargo-fuzz --version {TOOL} --locked')
    subprocess.run(['cargo', f'+{NIGHTLY}', 'metadata', '--manifest-path', 'fuzz/Cargo.toml', '--locked', '--no-deps', '--format-version', '1'], cwd=root, check=True, capture_output=True)
    corpus = root / f'target/tooling/fuzz/corpus/{args.target}'
    artifacts = root / f'target/tooling/fuzz/artifacts/{args.target}'
    corpus.mkdir(parents=True, exist_ok=True); artifacts.mkdir(parents=True, exist_ok=True)
    for seed in (root / f'fuzz/corpus/{args.target}').iterdir(): shutil.copyfile(seed, corpus / seed.name)
    locks = {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in [root/'Cargo.lock', root/'fuzz/Cargo.lock']}
    max_length = 16 if args.target == 'graph_semantics' else 32768
    command = ['cargo', f'+{NIGHTLY}', 'fuzz', 'run', args.target, str(args.replay.resolve() if args.replay else corpus), '--fuzz-dir', 'fuzz', '--',
               f'-max_total_time={args.seconds}', '-timeout=5', '-rss_limit_mb=512', f'-max_len={max_length}', '-print_final_stats=1', f'-artifact_prefix={artifacts}/']
    environment = dict(os.environ)
    # Bound ASan's freed-allocation quarantine, whose default overwhelmed the
    # 512 MiB campaign RSS limit despite a 53 MiB single-input reproduction.
    environment['ASAN_OPTIONS'] = environment.get('ASAN_OPTIONS', '') + ':quarantine_size_mb=64'
    inputs = sorted([*[path for path in root.glob('src/*.rs') if path.name != 'main.rs'], root/'Cargo.toml', root/'Cargo.lock', root/'fuzz/Cargo.toml', root/'fuzz/Cargo.lock', root/'fuzz/deny.exceptions.toml', root/'deny.toml', root/'scripts/fuzz.py', *root.glob('fuzz/fuzz_targets/*.rs'), *root.glob(f'fuzz/corpus/{args.target}/*')])
    hashes = {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in inputs}
    compiler = subprocess.check_output([environment.get('CC', 'cc'), '--version'], text=True, timeout=10)
    if 'Apple clang' in compiler:
        raise SystemExit('Native C ASan needs upstream LLVM clang; select CC/CXX as described in docs/maintainers/verification.md')
    environment['CFLAGS'] = environment.get('CFLAGS', '') + ' -fsanitize=fuzzer-no-link,address -fno-omit-frame-pointer'
    start = time.monotonic()
    result = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True, timeout=args.seconds+240)
    unchanged = all(hashlib.sha256((root/path).read_bytes()).hexdigest() == expected for path,expected in locks.items())
    source_unchanged = all(hashlib.sha256((root/path).read_bytes()).hexdigest() == expected for path,expected in hashes.items())
    report = dict(source_unchanged=source_unchanged, tool=version, nightly=NIGHTLY, command=command, seconds=round(time.monotonic()-start, 3),
                  exit=result.returncode, locks_unchanged=unchanged,
                  asan_quarantine_mib=64, native_cflags=environment['CFLAGS'], native_compiler=compiler, output_tail=(result.stdout+result.stderr)[-5000:],
                  input_sha256=hashes)
    output = root / args.output; output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2)+'\n')
    if result.returncode or not unchanged or not source_unchanged:
        raise SystemExit(f'Fuzzing failed (exit={result.returncode}, locks_unchanged={unchanged}); inspect {output} and {artifacts}')
    print(f'Bounded fuzz run passed; report: {output}')

if __name__ == '__main__': main()
