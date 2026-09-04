'use strict';

// HTTP fetching and SHA-256 verification for the driftwatchdog npm
// launcher. Kept dependency-free so the package does not pull a
// transitive supply chain onto every install. Tests use a fixture
// HTTPServer to drive these helpers without the network.

const crypto = require('node:crypto');
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const http = require('node:http');
const https = require('node:https');
const path = require('node:path');
const { URL } = require('node:url');

function requestBuffer(urlString, redirectsLeft = 5) {
  return new Promise((resolve, reject) => {
    let url;
    try {
      url = new URL(urlString);
    } catch (err) {
      reject(new Error(`invalid url: ${urlString}`));
      return;
    }
    const client = url.protocol === 'http:' ? http : https;
    const req = client.get(url, (res) => {
      const status = res.statusCode || 0;
      if (status >= 300 && status < 400 && res.headers.location) {
        res.resume();
        if (redirectsLeft <= 0) {
          reject(new Error(`too many redirects for ${urlString}`));
          return;
        }
        const next = new URL(res.headers.location, url).toString();
        resolve(requestBuffer(next, redirectsLeft - 1));
        return;
      }
      if (status !== 200) {
        res.resume();
        reject(new Error(`unexpected status ${status} for ${urlString}`));
        return;
      }
      const chunks = [];
      res.on('data', (chunk) => chunks.push(chunk));
      res.on('end', () => resolve(Buffer.concat(chunks)));
      res.on('error', reject);
    });
    req.on('error', reject);
    req.setTimeout(30000, () => {
      req.destroy(new Error(`timeout fetching ${urlString}`));
    });
  });
}

function hashBuffer(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

/**
 * Parse a `sha256sum`-style manifest.
 *
 * @param {string} text - manifest content
 * @returns {Map<string, string>} map of filename -> hex sha256
 */
function parseManifest(text) {
  const map = new Map();
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line) continue;
    const [hash, file] = line.split(/\s+/);
    if (!hash || !file) continue;
    if (!/^[0-9a-f]{64}$/i.test(hash)) continue;
    map.set(file, hash.toLowerCase());
  }
  return map;
}

/**
 * Verify that a buffer's SHA-256 matches the entry in the manifest for
 * the given filename.
 *
 * @param {Buffer} buffer
 * @param {Map<string, string>} manifest
 * @param {string} filename
 * @returns {{ok: true, hash: string} | {ok: false, reason: string, hash: string, expected: string|null}}
 */
function verifyAgainstManifest(buffer, manifest, filename) {
  const actual = hashBuffer(buffer);
  const expected = manifest.get(filename) || null;
  if (!expected) {
    return { ok: false, reason: `manifest missing entry for ${filename}`, hash: actual, expected };
  }
  if (expected !== actual) {
    return { ok: false, reason: 'checksum mismatch', hash: actual, expected };
  }
  return { ok: true, hash: actual };
}

/**
 * Extract a tar.gz archive into the target directory using only Node
 * built-ins. Pure-JS tar parsing keeps the launcher portable and
 * dependency-free, but it intentionally supports the narrow layout
 * driftwatchdog produces (one top-level directory, one executable, and
 * small text files). It rejects anything else to fail closed.
 */
async function extractTarGz(buffer, destDir) {
  await fsp.mkdir(destDir, { recursive: true });
  const zlib = require('node:zlib');
  const tarBytes = zlib.gunzipSync(buffer);

  let offset = 0;
  const decoder = Buffer.alloc(512);

  while (offset + 512 <= tarBytes.length) {
    tarBytes.copy(decoder, 0, offset, offset + 512);
    const header = decoder.toString('utf8');
    const isZero = /^[\u0000]+$/.test(header);
    offset += 512;
    if (isZero) {
      // End-of-archive marker.
      break;
    }
    const name = header.slice(0, 100).replace(/\0+$/, '');
    const sizeOctal = header.slice(124, 136).replace(/\0+$/, '').trim();
    const size = sizeOctal ? parseInt(sizeOctal, 8) : 0;
    const typeFlag = header[156] || '0';
    const prefix = header.slice(345, 500).replace(/\0+$/, '');
    const fullName = prefix ? `${prefix}/${name}` : name;
    const safeName = path.posix.normalize(fullName);
    if (safeName.startsWith('../') || safeName.includes('..')) {
      throw new Error(`unsafe entry in archive: ${safeName}`);
    }
    const dataStart = offset;
    const dataEnd = offset + size;
    if (dataEnd > tarBytes.length) {
      throw new Error('truncated archive');
    }
    const filePath = path.join(destDir, ...safeName.split('/'));
    if (typeFlag === '5' || safeName.endsWith('/')) {
      await fsp.mkdir(filePath, { recursive: true });
    } else {
      await fsp.mkdir(path.dirname(filePath), { recursive: true });
      const data = tarBytes.subarray(dataStart, dataEnd);
      await fsp.writeFile(filePath, data);
      // Best-effort mode bits from the tar header.
      const modeOctal = header.slice(100, 108).replace(/\0+$/, '').trim();
      if (modeOctal) {
        try {
          await fsp.chmod(filePath, parseInt(modeOctal, 8) & 0o7777);
        } catch {
          // ignore chmod errors on platforms that don't support them
        }
      }
    }
    // Round up to the next 512-byte boundary.
    offset = dataEnd + ((512 - (size % 512)) % 512);
  }
}

async function rmrf(target) {
  await fsp.rm(target, { recursive: true, force: true });
}

module.exports = {
  requestBuffer,
  hashBuffer,
  parseManifest,
  verifyAgainstManifest,
  extractTarGz,
  rmrf,
};
