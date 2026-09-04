## Why

Driftwatchdog currently assumes users can build or install the Rust project themselves, which creates unnecessary friction for a CLI intended to be used inside many different project toolchains. A small, reproducible release distribution for Linux and Intel macOS will make the first successful command available through familiar shell and npm workflows while preserving the single native implementation.

## What Changes

- Add release artifacts for Linux x86_64, Linux arm64, macOS x86_64, and macOS arm64.
- Add a versioned `curl`/shell installer that detects the supported operating system and architecture, downloads a release artifact, verifies its checksum, and installs the binary to a user-writable location by default.
- Add an npm package that selects and downloads the matching native binary, exposes the `driftwatch` command, and reports unsupported environments clearly.
- Publish checksums and make release artifacts consumable from direct download URLs.
- Add release automation and local packaging tests for artifact naming, checksum verification, platform selection, and installer behavior.
- Document shell installation, npm installation, direct downloads, and `cargo install` in README.md.

## Capabilities

### New Capabilities

- `release-distribution`: Define supported release artifacts, platform selection, checksums, installation channels, and failure behavior for Linux and macOS.

### Modified Capabilities

<!-- No existing capability requirements change. -->

## Impact

- Adds release/packaging scripts, npm package metadata and launcher code, installer documentation, and CI/release workflow configuration.
- Establishes a public artifact naming and download URL contract for the native CLI.
- Requires a release tag/version source and reproducible checksum generation, but introduces no runtime network calls after installation and no changes to local state or command semantics.
