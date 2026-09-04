'use strict';

// URL construction for the driftwatchdog npm launcher. Mirrors
// scripts/lib/release.sh so the release URL contract is identical
// regardless of installation channel.

const DEFAULT_REPOSITORY = 'lileililiwen/driftwatchdog';

/**
 * @param {string} version - bare or v-prefixed release tag (e.g. "0.1.0" or "v0.1.0")
 * @param {string} [target] - artifact suffix (e.g. "linux-x86_64")
 * @param {{repository?: string, base?: string}} [opts]
 * @returns {{base: string, manifest: string, archive: string|null}}
 */
function releaseUrls(version, target, opts = {}) {
  const repository = opts.repository || DEFAULT_REPOSITORY;
  const baseOverride = opts.base;
  const tag = version.startsWith('v') ? version : `v${version}`;
  const base = baseOverride || `https://github.com/${repository}/releases/download/${tag}`;
  const bare = tag.slice(1);
  const manifest = `${base}/driftwatchdog-${bare}-checksums.txt`;
  const archive = target ? `${base}/driftwatchdog-${bare}-${target}.tar.gz` : null;
  return { base, manifest, archive };
}

module.exports = { DEFAULT_REPOSITORY, releaseUrls };
