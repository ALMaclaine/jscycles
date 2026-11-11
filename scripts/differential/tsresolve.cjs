// TypeScript resolution oracle for scripts/differential/difftest.py.
// Reads JSON lines {"from": absFile, "spec": specifier, "config": tsconfigPath|null}
// and writes JSON lines {"resolved": absPath|null}. Uses TypeScript's own
// resolveModuleName with the selected config's options, Bundler resolution and
// allowJs, so jscycles' file resolution can be compared with tsc's.
"use strict";
const path = require("path");
const fs = require("fs");
const readline = require("readline");
const ts = require(path.join(process.argv[2], "node_modules", "typescript"));

const configs = new Map();
function options(configPath) {
  const key = configPath || "";
  if (!configs.has(key)) {
    let opts = {};
    if (configPath) {
      const read = ts.readConfigFile(configPath, ts.sys.readFile);
      opts = ts.parseJsonConfigFileContent(read.config || {}, ts.sys, path.dirname(configPath)).options;
    }
    opts.allowJs = true;
    opts.moduleResolution = ts.ModuleResolutionKind.Bundler;
    opts.module = ts.ModuleKind.ESNext;
    configs.set(key, opts);
  }
  return configs.get(key);
}

const rl = readline.createInterface({ input: process.stdin, terminal: false });
rl.on("line", (line) => {
  if (!line.trim()) return;
  const query = JSON.parse(line);
  if (query.reset) {
    configs.clear();
    process.stdout.write("{}\n");
    return;
  }
  let resolved = null;
  try {
    const result = ts.resolveModuleName(query.spec, query.from, options(query.config), ts.sys);
    const file = result.resolvedModule && result.resolvedModule.resolvedFileName;
    // TypeScript reports `/` separators on Windows too.
    if (file && !/[\\/]node_modules[\\/]/.test(file)) resolved = fs.realpathSync.native(file);
  } catch (error) {
    resolved = null;
  }
  process.stdout.write(JSON.stringify({ resolved }) + "\n");
});
