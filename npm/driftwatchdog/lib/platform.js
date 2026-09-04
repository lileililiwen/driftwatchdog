'use strict';

// Host platform detection for the driftwatchdog npm launcher.
// Mirrors scripts/lib/platform.sh so the supported target set stays in
// one place (the two are kept in sync by the test suite; see
// tests/packaging/target_mapping.test.mjs).

const SUPPORTED_TARGETS = Object.freeze([
  'linux-x86_64',
  'linux-arm64',
  'darwin-x86_64',
]);

/**
 * Map a (os, arch) tuple to a driftwatchdog artifact suffix.
 *
 * @param {NodeJS.Platform} os - 'linux' or 'darwin' (case-insensitive)
 * @param {string} arch - 'x64'/'x86_64'/'amd64' or 'arm64'/'aarch64'
 * @returns {string} the artifact suffix (one of SUPPORTED_TARGETS)
 * @throws {Error} if the host is outside the supported set
 */
function targetFor(os, arch) {
  const normalizedOs = String(os || '').toLowerCase();
  let normalizedArch = String(arch || '').toLowerCase();
  if (normalizedArch === 'amd64') normalizedArch = 'x86_64';
  if (normalizedArch === 'x64') normalizedArch = 'x86_64';
  if (normalizedArch === 'aarch64') normalizedArch = 'arm64';

  switch (`${normalizedOs}/${normalizedArch}`) {
    case 'linux/x86_64':
      return 'linux-x86_64';
    case 'linux/arm64':
      return 'linux-arm64';
    case 'darwin/x86_64':
      return 'darwin-x86_64';
    case 'darwin/arm64':
      throw new Error('macOS arm64 is not in the supported target set');
    default:
      throw new Error(
        `unsupported host: os=${normalizedOs} arch=${normalizedArch} ` +
          `(supported: ${SUPPORTED_TARGETS.join(', ')})`,
      );
  }
}

/**
 * Detect the artifact suffix for the current process.
 *
 * @returns {string}
 * @throws {Error} if the host is outside the supported set
 */
function detectTarget() {
  return targetFor(process.platform, process.arch);
}

module.exports = { SUPPORTED_TARGETS, targetFor, detectTarget };
