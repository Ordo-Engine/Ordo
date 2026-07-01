#!/usr/bin/env node
// Thin launcher: exec the downloaded native `ordo` binary, forwarding all args
// and stdio (so the MCP stdio server and interactive prompts work unchanged).

const path = require('path');
const fs = require('fs');
const { spawnSync } = require('child_process');

const bin = path.join(__dirname, process.platform === 'win32' ? 'ordo-native.exe' : 'ordo-native');

if (!fs.existsSync(bin)) {
  console.error(
    'ordo: native binary not found. Reinstall the package (its postinstall downloads it),\n' +
      '  or build from source: cargo install --git https://github.com/Ordo-Engine/Ordo ordo-cli'
  );
  process.exit(1);
}

const res = spawnSync(bin, process.argv.slice(2), { stdio: 'inherit' });
if (res.error) {
  console.error(`ordo: ${res.error.message}`);
  process.exit(1);
}
process.exit(res.status === null ? 1 : res.status);
