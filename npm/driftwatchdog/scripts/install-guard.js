'use strict';

// npm preinstall guard: fail fast on darwin-arm64 (and other unsupported
// hosts) with the supported target list and alternatives. npm's `os`/`cpu`
// fields cannot express "darwin-x64-only plus linux-arm64", so the
// package installs broadly and this guard enforces the matrix before
// any download happens. Mirrors lib/platform.js.

const { targetFor } = require('../lib/platform');

try {
  targetFor(process.platform, process.arch);
} catch (err) {
  console.error(`driftwatchdog: ${err.message}`);
  console.error('Supported: Linux x86_64, Linux arm64, macOS x86_64.');
  console.error(
    'Alternatives: shell installer (scripts/install.sh) or cargo install driftwatchdog.',
  );
  process.exit(1);
}
