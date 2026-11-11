#!/usr/bin/env node
// Runs the prebuilt jscycles binary from the optional dependency that matches
// this platform. scripts/npm_packages.py publishes this file as `jscycles`.
'use strict';

const { spawnSync } = require('node:child_process');

const PACKAGES = {
  'linux x64': '@jscycles/linux-x64',
  'linux arm64': '@jscycles/linux-arm64',
  'darwin x64': '@jscycles/darwin-x64',
  'darwin arm64': '@jscycles/darwin-arm64',
  'win32 x64': '@jscycles/win32-x64',
};

// Exit status 2 is jscycles' "could not complete the scan" status, so a CI
// job never mistakes a broken install for a clean result.
function fail(message) {
  process.stderr.write(`jscycles: ${message}\n`);
  process.exit(2);
}

function binaryPath() {
  if (process.env.JSCYCLES_BINARY) {
    return process.env.JSCYCLES_BINARY;
  }
  const name = PACKAGES[`${process.platform} ${process.arch}`];
  if (!name) {
    fail(
      `no prebuilt binary for ${process.platform}-${process.arch}. ` +
        'Install with `cargo install jscycles --locked`, then set JSCYCLES_BINARY ' +
        'to its path or run it directly.',
    );
  }
  const executable = process.platform === 'win32' ? 'jscycles.exe' : 'jscycles';
  try {
    return require.resolve(`${name}/bin/${executable}`);
  } catch {
    fail(
      `the ${name} package is not installed. It is an optional dependency of ` +
        'jscycles; reinstall without --no-optional or --omit=optional.',
    );
  }
}

const binary = binaryPath();
const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
if (result.error) {
  fail(`could not run ${binary}: ${result.error.message}`);
}
if (result.signal) {
  process.kill(process.pid, result.signal);
}
process.exit(result.status ?? 2);
