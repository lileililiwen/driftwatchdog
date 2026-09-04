'use strict';

// Tests for the npm launcher's lib helpers. Uses node:test and runs
// without touching the network. Drives verify.js, release.js, and
// platform.js through their public surface, including a small
// in-process HTTP server for the full acquire/run path.

const test = require('node:test');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');

const { targetFor, detectTarget, SUPPORTED_TARGETS } = require('../lib/platform.js');
const { releaseUrls, DEFAULT_REPOSITORY } = require('../lib/release.js');
const {
  parseManifest,
  verifyAgainstManifest,
  hashBuffer,
  extractTarGz,
} = require('../lib/verify.js');

test('platform: SUPPORTED_TARGETS is the documented set', () => {
  assert.deepEqual(
    [...SUPPORTED_TARGETS],
    ['linux-x86_64', 'linux-arm64', 'darwin-x86_64'],
  );
});

test('platform: targetFor normalizes uname-style aliases', () => {
  assert.equal(targetFor('linux', 'x86_64'), 'linux-x86_64');
  assert.equal(targetFor('linux', 'amd64'), 'linux-x86_64');
  assert.equal(targetFor('linux', 'aarch64'), 'linux-arm64');
  assert.equal(targetFor('linux', 'arm64'), 'linux-arm64');
  assert.equal(targetFor('darwin', 'x86_64'), 'darwin-x86_64');
  assert.equal(targetFor('darwin', 'amd64'), 'darwin-x86_64');
  assert.equal(targetFor('Darwin', 'X86_64'), 'darwin-x86_64');
  assert.throws(() => targetFor('darwin', 'arm64'), /macOS arm64/);
  assert.throws(() => targetFor('darwin', 'aarch64'), /macOS arm64/);
  assert.throws(() => targetFor('windows', 'x86_64'), /unsupported host/);
  assert.throws(() => targetFor('linux', 'ppc64le'), /unsupported host/);
});

test('platform: detectTarget returns the host target or throws unsupported', () => {
  // detectTarget has no env override; it always reflects process.platform
  // and process.arch. The supported matrix is the only contract.
  const detected = (() => {
    try {
      return detectTarget();
    } catch (err) {
      return null;
    }
  })();
  if (detected !== null) {
    assert.ok(SUPPORTED_TARGETS.includes(detected), `unexpected target: ${detected}`);
  }
});

test('platform: targetFor correctly maps darwin/arm64 and aarch64 to unsupported', () => {
  assert.throws(() => targetFor('darwin', 'arm64'), /macOS arm64/);
  assert.throws(() => targetFor('darwin', 'aarch64'), /macOS arm64/);
});

test('release: releaseUrls shape and version normalization', () => {
  const v = releaseUrls('0.1.0', 'linux-x86_64');
  assert.equal(v.base, 'https://github.com/lileililiwen/driftwatchdog/releases/download/v0.1.0');
  assert.equal(v.manifest, 'https://github.com/lileililiwen/driftwatchdog/releases/download/v0.1.0/driftwatchdog-0.1.0-checksums.txt');
  assert.equal(v.archive, 'https://github.com/lileililiwen/driftwatchdog/releases/download/v0.1.0/driftwatchdog-0.1.0-linux-x86_64.tar.gz');

  const v2 = releaseUrls('v0.1.0', null, { base: 'https://example.test/release' });
  assert.equal(v2.base, 'https://example.test/release');
  assert.equal(v2.manifest, 'https://example.test/release/driftwatchdog-0.1.0-checksums.txt');
  assert.equal(v2.archive, null);
});

test('verify: parseManifest accepts sha256sum output and ignores garbage', () => {
  const text = [
    '# comment line',
    '',
    'a'.repeat(64) + '  driftwatchdog-0.1.0-linux-x86_64.tar.gz',
    'b'.repeat(64) + '  driftwatchdog-0.1.0-darwin-x86_64.tar.gz',
    'not-a-hash  driftwatchdog-0.1.0-linux-arm64.tar.gz',
  ].join('\n');
  const map = parseManifest(text);
  assert.equal(map.size, 2);
  assert.equal(map.get('driftwatchdog-0.1.0-linux-x86_64.tar.gz'), 'a'.repeat(64));
});

test('verify: verifyAgainstManifest reports match and mismatch', () => {
  const buffer = Buffer.from('hello, driftwatchdog');
  const expected = hashBuffer(buffer);
  const manifest = new Map([['driftwatchdog-0.1.0-linux-x86_64.tar.gz', expected]]);
  const ok = verifyAgainstManifest(buffer, manifest, 'driftwatchdog-0.1.0-linux-x86_64.tar.gz');
  assert.equal(ok.ok, true);
  assert.equal(ok.hash, expected);

  const bad = verifyAgainstManifest(
    Buffer.from('tampered'),
    manifest,
    'driftwatchdog-0.1.0-linux-x86_64.tar.gz',
  );
  assert.equal(bad.ok, false);
  assert.equal(bad.reason, 'checksum mismatch');
  assert.equal(bad.expected, expected);

  const missing = verifyAgainstManifest(
    buffer,
    new Map(),
    'driftwatchdog-0.1.0-linux-x86_64.tar.gz',
  );
  assert.equal(missing.ok, false);
  assert.match(missing.reason, /manifest missing entry/);
});

async function buildArchiveFixture(tmp, version, target, payload) {
  const pkgDir = path.join(tmp, `driftwatchdog-${version}-${target}`);
  await fsp.mkdir(pkgDir, { recursive: true });
  const bin = path.join(pkgDir, 'driftwatchdog');
  await fsp.writeFile(bin, payload);
  await fsp.chmod(bin, 0o755);
  await fsp.writeFile(path.join(pkgDir, 'VERSION'), `${version}\n`);
  const tarPath = path.join(tmp, `driftwatchdog-${version}-${target}.tar.gz`);
  // Build a real tar.gz using the same lib the launcher uses, but
  // round-tripped through extractTarGz + a fresh writeFile would be
  // complex. We shell out: tar is part of every supported target.
  const { spawnSync } = require('node:child_process');
  const result = spawnSync('tar', ['-czf', tarPath, '-C', tmp, `driftwatchdog-${version}-${target}`]);
  if (result.status !== 0) {
    throw new Error(`tar failed: ${result.stderr?.toString() || ''}`);
  }
  return tarPath;
}

test('verify: extractTarGz reproduces the documented layout', async () => {
  const tmp = await fsp.mkdtemp(path.join(os.tmpdir(), 'driftwatchdog-verify-'));
  try {
    const tarPath = await buildArchiveFixture(tmp, '9.9.9', 'linux-x86_64', '#!/bin/sh\necho hi\n');
    const buffer = await fsp.readFile(tarPath);
    const out = path.join(tmp, 'out');
    await extractTarGz(buffer, out);
    const extracted = path.join(out, 'driftwatchdog-9.9.9-linux-x86_64', 'driftwatchdog');
    const stat = await fsp.stat(extracted);
    assert.ok(stat.isFile());
    const versionFile = path.join(out, 'driftwatchdog-9.9.9-linux-x86_64', 'VERSION');
    assert.equal((await fsp.readFile(versionFile, 'utf8')).trim(), '9.9.9');
  } finally {
    await fsp.rm(tmp, { recursive: true, force: true });
  }
});

test('verify: extractTarGz refuses path traversal entries', async () => {
  // GNU tar refuses to produce archives whose members escape with
  // `..`, so we hand-build a minimal gzipped tar containing a
  // `../escape.sh` entry to exercise the extractor's safety check.
  const tmp = await fsp.mkdtemp(path.join(os.tmpdir(), 'driftwatchdog-traversal-'));
  try {
    const header = Buffer.alloc(512);
    header.write('../escape.sh', 0, 'utf8');
    const sizeOct = '00000000020';
    header.write(sizeOct, 124, 'utf8');
    header.write('0', 156, 'utf8');
    header.write('0000755', 100, 'utf8');
    header.write('ustar  \0', 257, 'utf8');
    const payload = Buffer.from('#!/bin/sh\necho pwned\n');
    const rounded = Buffer.alloc(512);
    payload.copy(rounded);
    const padded = Buffer.concat([header, rounded, Buffer.alloc(1024)]);
    const zlib = require('node:zlib');
    const archive = zlib.gzipSync(padded);
    await assert.rejects(extractTarGz(archive, path.join(tmp, 'out')), /unsafe entry/);
  } finally {
    await fsp.rm(tmp, { recursive: true, force: true });
  }
});

function startFixtureServer(tmp) {
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      const url = req.url || '/';
      // Strip leading slash and decode.
      const rel = decodeURIComponent(url.replace(/^\/+/, ''));
      const file = path.join(tmp, rel);
      if (!file.startsWith(tmp)) {
        res.statusCode = 403;
        res.end('forbidden');
        return;
      }
      fsp
        .stat(file)
        .then((stat) => {
          if (!stat.isFile()) {
            res.statusCode = 404;
            res.end('not a file');
            return;
          }
          res.setHeader('content-type', 'application/octet-stream');
          res.setHeader('content-length', stat.size);
          fs.createReadStream(file).pipe(res);
        })
        .catch(() => {
          res.statusCode = 404;
          res.end('not found');
        });
    });
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      resolve({ server, port });
    });
  });
}

test('launcher: full acquire + exec round-trip against a local fixture', async () => {
  const tmp = await fsp.mkdtemp(path.join(os.tmpdir(), 'driftwatchdog-launcher-'));
  const cache = path.join(tmp, 'cache');
  try {
    const version = '9.9.9';
    const target = process.platform === 'darwin' ? 'darwin-x86_64' : 'linux-x86_64';

    // Build archive + manifest in tmp.
    const tarPath = await buildArchiveFixture(tmp, version, target, '#!/bin/sh\necho launch-fixture-ok\n');
    const archiveHash = crypto.createHash('sha256').update(await fsp.readFile(tarPath)).digest('hex');
    const manifestName = `driftwatchdog-${version}-checksums.txt`;
    const manifestPath = path.join(tmp, manifestName);
    await fsp.writeFile(manifestPath, `${archiveHash}  driftwatchdog-${version}-${target}.tar.gz\n`);

    const { server, port } = await startFixtureServer(tmp);
    try {
      const prevBase = process.env.DRIFTWATCH_RELEASE_BASE;
      const prevCache = process.env.DRIFTWATCH_CACHE_DIR;
      const prevVersion = process.env.DRIFTWATCH_PACKAGE_VERSION;
      const prevTarget = process.env.DRIFTWATCH_TARGET;
      process.env.DRIFTWATCH_RELEASE_BASE = `http://127.0.0.1:${port}`;
      process.env.DRIFTWATCH_CACHE_DIR = cache;
      process.env.DRIFTWATCH_PACKAGE_VERSION = version;
      process.env.DRIFTWATCH_TARGET = target;
      try {
        // Clear require cache so a previous test's launcher state is reset.
        const launcherPath = require.resolve('../bin/driftwatch.js');
        delete require.cache[launcherPath];

        const { spawn } = require('node:child_process');
        await new Promise((resolveRun, rejectRun) => {
          const child = spawn(
            process.execPath,
            [launcherPath, '--hello'],
            { stdio: ['ignore', 'pipe', 'pipe'] },
          );
          let stdout = '';
          let stderr = '';
          child.stdout.on('data', (b) => { stdout += b.toString(); });
          child.stderr.on('data', (b) => { stderr += b.toString(); });
          child.on('error', rejectRun);
          child.on('exit', (code) => {
            try {
              assert.equal(code, 0, `launcher exit code: stderr=${stderr}`);
              assert.match(stdout, /launch-fixture-ok/);
              resolveRun();
            } catch (err) {
              rejectRun(err);
            }
          });
        });

        // Cache reuse: the second invocation must not redownload.
        // We tear down the server to prove it.
        await new Promise((r) => server.close(r));
        const { spawn: spawn2 } = require('node:child_process');
        await new Promise((resolveRun, rejectRun) => {
          const child = spawn2(
            process.execPath,
            [launcherPath, '--again'],
            { stdio: ['ignore', 'pipe', 'pipe'] },
          );
          let stdout = '';
          child.stdout.on('data', (b) => { stdout += b.toString(); });
          child.on('error', rejectRun);
          child.on('exit', (code) => {
            try {
              assert.equal(code, 0);
              assert.match(stdout, /launch-fixture-ok/);
              resolveRun();
            } catch (err) {
              rejectRun(err);
            }
          });
        });
      } finally {
        if (prevBase === undefined) delete process.env.DRIFTWATCH_RELEASE_BASE;
        else process.env.DRIFTWATCH_RELEASE_BASE = prevBase;
        if (prevCache === undefined) delete process.env.DRIFTWATCH_CACHE_DIR;
        else process.env.DRIFTWATCH_CACHE_DIR = prevCache;
        if (prevVersion === undefined) delete process.env.DRIFTWATCH_PACKAGE_VERSION;
        else process.env.DRIFTWATCH_PACKAGE_VERSION = prevVersion;
        if (prevTarget === undefined) delete process.env.DRIFTWATCH_TARGET;
        else process.env.DRIFTWATCH_TARGET = prevTarget;
      }
    } catch (err) {
      try { await new Promise((r) => server.close(r)); } catch {}
      throw err;
    }
  } finally {
    await fsp.rm(tmp, { recursive: true, force: true });
  }
});

test('launcher: download verification failure does not leave a valid cache entry', async () => {
  const tmp = await fsp.mkdtemp(path.join(os.tmpdir(), 'driftwatchdog-bad-'));
  const cache = path.join(tmp, 'cache');
  try {
    const version = '9.9.9';
    const target = process.platform === 'darwin' ? 'darwin-x86_64' : 'linux-x86_64';

    await buildArchiveFixture(tmp, version, target, '#!/bin/sh\necho unused\n');
    // Write a manifest with an obviously-wrong hash.
    await fsp.writeFile(
      path.join(tmp, `driftwatchdog-${version}-checksums.txt`),
      `${'0'.repeat(64)}  driftwatchdog-${version}-${target}.tar.gz\n`,
    );

    const { server, port } = await startFixtureServer(tmp);
    try {
      const launcherPath = require.resolve('../bin/driftwatch.js');
      delete require.cache[launcherPath];
      const { spawn } = require('node:child_process');
      const prevBase = process.env.DRIFTWATCH_RELEASE_BASE;
      const prevCache = process.env.DRIFTWATCH_CACHE_DIR;
      const prevVersion = process.env.DRIFTWATCH_PACKAGE_VERSION;
      const prevTarget = process.env.DRIFTWATCH_TARGET;
      process.env.DRIFTWATCH_RELEASE_BASE = `http://127.0.0.1:${port}`;
      process.env.DRIFTWATCH_CACHE_DIR = cache;
      process.env.DRIFTWATCH_PACKAGE_VERSION = version;
      process.env.DRIFTWATCH_TARGET = target;
      try {
        await new Promise((resolveRun, rejectRun) => {
          const child = spawn(process.execPath, [launcherPath], { stdio: ['ignore', 'pipe', 'pipe'] });
          let stderr = '';
          child.stderr.on('data', (b) => { stderr += b.toString(); });
          child.on('error', rejectRun);
          child.on('exit', (code) => {
            try {
              assert.notEqual(code, 0);
              assert.match(stderr, /verification failed/);
              resolveRun();
            } catch (err) {
              rejectRun(err);
            }
          });
        });
        // The cache must not contain a runnable binary for this version.
        const versionDir = path.join(cache, version);
        const entries = await fsp.readdir(versionDir).catch(() => []);
        for (const entry of entries) {
          const candidate = path.join(versionDir, entry, 'driftwatchdog');
          await assert.rejects(fsp.stat(candidate), /ENOENT/, `expected ${candidate} to be absent`);
        }
      } finally {
        if (prevBase === undefined) delete process.env.DRIFTWATCH_RELEASE_BASE;
        else process.env.DRIFTWATCH_RELEASE_BASE = prevBase;
        if (prevCache === undefined) delete process.env.DRIFTWATCH_CACHE_DIR;
        else process.env.DRIFTWATCH_CACHE_DIR = prevCache;
        if (prevVersion === undefined) delete process.env.DRIFTWATCH_PACKAGE_VERSION;
        else process.env.DRIFTWATCH_PACKAGE_VERSION = prevVersion;
        if (prevTarget === undefined) delete process.env.DRIFTWATCH_TARGET;
        else process.env.DRIFTWATCH_TARGET = prevTarget;
      }
    } finally {
      await new Promise((r) => server.close(r));
    }
  } finally {
    await fsp.rm(tmp, { recursive: true, force: true });
  }
});
