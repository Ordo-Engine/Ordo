#!/usr/bin/env node
// postinstall: download the platform-matched `ordo` static binary from the
// matching GitHub Release into ./bin. Failures are non-fatal so `npm install`
// still succeeds (the bin shim prints a clear message if the binary is missing).

const fs = require('fs');
const path = require('path');
const https = require('https');
const { version } = require('./package.json');

const REPO = 'Ordo-Engine/Ordo';
const TAG = `cli-v${version}`;

// node platform-arch -> release asset name (see .github/workflows/release-cli.yml)
const ASSETS = {
  'linux-x64': 'ordo-x86_64-unknown-linux-musl',
  'linux-arm64': 'ordo-aarch64-unknown-linux-musl',
  'darwin-x64': 'ordo-x86_64-apple-darwin',
  'darwin-arm64': 'ordo-aarch64-apple-darwin',
  'win32-x64': 'ordo-x86_64-pc-windows-msvc.exe',
};

const key = `${process.platform}-${process.arch}`;
const asset = ASSETS[key];
if (!asset) {
  console.error(
    `ordo: no prebuilt binary for ${key}. Build from source:\n` +
      `  cargo install --git https://github.com/${REPO} ordo-cli`
  );
  process.exit(0);
}

const binDir = path.join(__dirname, 'bin');
fs.mkdirSync(binDir, { recursive: true });
const out = path.join(binDir, process.platform === 'win32' ? 'ordo-native.exe' : 'ordo-native');
const url = `https://github.com/${REPO}/releases/download/${TAG}/${asset}`;

function download(from, dest, redirects = 0) {
  return new Promise((resolve, reject) => {
    https
      .get(from, { headers: { 'User-Agent': 'ordo-cli-installer' } }, (res) => {
        if ([301, 302, 307, 308].includes(res.statusCode) && res.headers.location) {
          if (redirects > 5) return reject(new Error('too many redirects'));
          res.resume();
          return resolve(download(res.headers.location, dest, redirects + 1));
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`HTTP ${res.statusCode} for ${from}`));
        }
        const file = fs.createWriteStream(dest, { mode: 0o755 });
        res.pipe(file);
        file.on('finish', () => file.close(resolve));
        file.on('error', reject);
      })
      .on('error', reject);
  });
}

download(url, out)
  .then(() => {
    fs.chmodSync(out, 0o755);
    console.log(`ordo ${version} installed (${asset})`);
  })
  .catch((e) => {
    console.error(`ordo: failed to download the binary (${e.message}).`);
    console.error(`  You can build from source: cargo install --git https://github.com/${REPO} ordo-cli`);
    process.exit(0);
  });
