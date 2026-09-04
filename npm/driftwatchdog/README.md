# driftwatchdog

Dependency-free launcher for the [driftwatchdog](https://github.com/lileililiwen/driftwatchdog)
CLI. Detects the host operating system and CPU architecture, downloads the
matching native release archive from GitHub Releases, verifies it against
the bundled SHA-256 checksum manifest, caches it by version, and forwards
all arguments and the native process exit status to the native binary.

The launcher is intentionally not a JavaScript reimplementation of the CLI.
It is a thin acquisition and exec helper. Once the native binary is in the
cache, subsequent invocations run offline.

## Supported platforms

| Operating system | Architecture     | Target suffix   |
| ---------------- | ---------------- | --------------- |
| Linux (glibc)    | x86_64           | `linux-x86_64`  |
| Linux (glibc)    | arm64 / aarch64  | `linux-arm64`   |
| macOS            | x86_64 (Intel)   | `darwin-x86_64` |

macOS arm64, Windows, and musl-based Linux are intentionally not part of
the supported set. The launcher exits with an actionable diagnostic on
unsupported hosts.

## Install

```sh
npm i -g driftwatchdog
```

The launcher exposes the `driftwatch` command.

## Usage

```sh
driftwatch init
driftwatch run cargo test
driftwatch list
driftwatch report --ai > drift.md
```

The launcher forwards every argument and the native process exit status
unchanged, so it is a drop-in replacement for any other driftwatchdog
install.

## How it works

1. Resolve the host target (`process.platform` + `process.arch`).
2. Fetch `driftwatchdog-<version>-<target>.tar.gz` and the
   `driftwatchdog-<version>-checksums.txt` manifest from GitHub Releases.
3. Verify the archive SHA-256 against the manifest entry.
4. Extract to `<cache>/<version>/<target>/driftwatchdog`, `chmod 0755`.
5. Spawn the binary with the original argv and `stdio: 'inherit'`.

The cache lives under `$DRIFTWATCH_CACHE_DIR` if set, otherwise
`$XDG_CACHE_HOME/driftwatchdog` (Linux) or
`~/Library/Caches/driftwatchdog` (macOS). Subsequent invocations reuse
the cache and never touch the network.

If a download or checksum verification fails, the launcher exits non-zero,
does not execute the unverified file, and does not leave a partial file in
the cache.

## Environment variables

| Variable                       | Purpose                                                    |
| ------------------------------ | ---------------------------------------------------------- |
| `DRIFTWATCH_CACHE_DIR`         | Override the cache root (default follows XDG / macOS).     |
| `DRIFTWATCH_RELEASE_BASE`      | Override the release URL base (used by tests).             |
| `DRIFTWATCH_REPOSITORY`        | Override the GitHub `owner/repo` for the release URL.      |
| `DRIFTWATCH_PACKAGE_VERSION`   | Override the package version (test fixture support).       |

## Alternatives

- Shell installer:
  `curl -fsSL https://github.com/lileililiwen/driftwatchdog/releases/latest/download/install.sh | sh`
- Direct download from
  [GitHub Releases](https://github.com/lileililiwen/driftwatchdog/releases).
- `cargo install driftwatchdog`.

## License

MIT — see [LICENSE](https://github.com/lileililiwen/driftwatchdog/blob/main/LICENSE).
