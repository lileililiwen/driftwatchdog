#!/usr/bin/env node
'use strict';

// driftwatch npm launcher.
//
// Selects the matching driftwatchdog release binary for the host,
// downloads it (and the checksum manifest) from GitHub Releases,
// verifies the archive against the manifest, and executes the native
// binary with the original arguments and exit status. The launcher
// intentionally contains zero reimplementation of the driftwatchdog
// CLI: it is a thin, dependency-free downloader and exec helper.
//
// Behavior contract:
//   * Supported hosts: Linux x86_64, Linux arm64, macOS x86_64.
//   * Unsupported hosts exit non-zero with an actionable message.
//   * On download or verification failure the launcher exits non-zero,
//     never executes the unverified file, and does not leave it as a
//     valid cache entry.
//   * The launcher forwards all CLI arguments and the native process's
//     exit status unchanged.

const path = require('node:path');
const os = require('node:os');
const fsp = require('node:fs/promises');
const { spawn } = require('node:child_process');

const { detectTarget } = require('../lib/platform.js');
const { releaseUrls, DEFAULT_REPOSITORY } = require('../lib/release.js');
const {
  requestBuffer,
  parseManifest,
  verifyAgainstManifest,
  extractTarGz,
  rmrf,
} = require('../lib/verify.js');

const PACKAGE_VERSION = (() => {
  // Test hook: DRIFTWATCH_PACKAGE_VERSION lets the test suite simulate
  // a "just-published" package against a fixture manifest/archive set.
  // Not advertised in README; production users always get the bundled
  // package.json version.
  if (process.env.DRIFTWATCH_PACKAGE_VERSION) {
    return process.env.DRIFTWATCH_PACKAGE_VERSION.replace(/^v/, '');
  }
  return require('../package.json').version;
})();

function cacheRoot() {
  if (process.env.DRIFTWATCH_CACHE_DIR) return process.env.DRIFTWATCH_CACHE_DIR;
  // XDG cache on Linux; ~/Library/Caches on macOS. os.homedir() avoids
  // hard-coding a path so the launcher works in locked-down environments.
  const xdg = process.env.XDG_CACHE_HOME;
  if (xdg) return path.join(xdg, 'driftwatchdog');
  if (process.platform === 'darwin') {
    return path.join(os.homedir(), 'Library', 'Caches', 'driftwatchdog');
  }
  return path.join(os.homedir(), '.cache', 'driftwatchdog');
}

function binaryPath(target, version) {
  return path.join(cacheRoot(), version, target, 'driftwatchdog');
}

function stagePath(target, version) {
  return path.join(cacheRoot(), `${version}-${target}-stage`);
}

function log(msg) {
  process.stderr.write(`driftwatch: ${msg}\n`);
}

function fail(msg) {
  log(msg);
  process.exit(1);
}

async function acquire(target, version, opts = {}) {
  const { manifest, archive } = releaseUrls(version, target, {
    repository: opts.repository || DEFAULT_REPOSITORY,
    base: opts.base,
  });
  log(`target=${target} version=v${version}`);
  log(`fetching manifest ${manifest}`);
  const manifestBytes = await requestBuffer(manifest);
  const manifestEntries = parseManifest(manifestBytes.toString('utf8'));
  const archiveName = `driftwatchdog-${version}-${target}.tar.gz`;
  const archiveUrl = archive;
  log(`fetching archive ${archiveUrl}`);
  const archiveBytes = await requestBuffer(archiveUrl);
  const result = verifyAgainstManifest(archiveBytes, manifestEntries, archiveName);
  if (!result.ok) {
    fail(
      `verification failed: ${result.reason} (expected ${result.expected || '<missing>'}, ` +
        `got ${result.hash})`,
    );
  }
  const finalBinary = binaryPath(target, version);
  const stage = stagePath(target, version);
  await rmrf(stage);
  await fsp.mkdir(stage, { recursive: true });
  try {
    await extractTarGz(archiveBytes, stage);
    const extracted = path.join(stage, `driftwatchdog-${version}-${target}`, 'driftwatchdog');
    await fsp.chmod(extracted, 0o755);
    // Move into the versioned cache atomically.
    await rmrf(path.join(cacheRoot(), version, target));
    await fsp.mkdir(path.dirname(finalBinary), { recursive: true });
    await fsp.rename(extracted, finalBinary);
  } catch (err) {
    await rmrf(stage);
    await rmrf(path.join(cacheRoot(), version, target));
    throw err;
  } finally {
    await rmrf(stage);
  }
  return finalBinary;
}

async function ensureBinary(target, version, opts) {
  const candidate = binaryPath(target, version);
  try {
    const stat = await fsp.stat(candidate);
    if (stat.isFile()) return candidate;
  } catch {
    // not cached
  }
  return acquire(target, version, opts);
}

async function run() {
  let target;
  try {
    target = detectTarget();
  } catch (err) {
    fail(
      `${err.message}\n` +
        `Supported: Linux x86_64, Linux arm64, macOS x86_64.\n` +
        `Alternatives: cargo install driftwatchdog, or download from ` +
        `https://github.com/${DEFAULT_REPOSITORY}/releases.`,
    );
  }

  const opts = {};
  if (process.env.DRIFTWATCH_RELEASE_BASE) {
    opts.base = process.env.DRIFTWATCH_RELEASE_BASE;
  }
  if (process.env.DRIFTWATCH_REPOSITORY) {
    opts.repository = process.env.DRIFTWATCH_REPOSITORY;
  }

  const binary = await ensureBinary(target, PACKAGE_VERSION, opts);
  const child = spawn(binary, process.argv.slice(2), {
    stdio: 'inherit',
  });
  child.on('error', (err) => {
    log(`failed to launch native binary: ${err.message}`);
    process.exit(1);
  });
  child.on('exit', (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
    } else {
      process.exit(code === null ? 1 : code);
    }
  });
}

run().catch((err) => {
  if (err && err.message) {
    log(err.message);
  } else {
    log(String(err));
  }
  process.exit(1);
});
