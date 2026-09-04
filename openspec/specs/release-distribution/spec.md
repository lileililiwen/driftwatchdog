# release-distribution Specification

## Purpose
TBD - created by archiving change linux-macos-distribution. Update Purpose after archive.
## Requirements
### Requirement: Supported release artifacts

The project SHALL publish versioned native release archives for Linux x86_64, Linux arm64, and macOS x86_64 (Intel), with one executable named `driftwatchdog` in each archive. Each release SHALL publish a SHA-256 checksum manifest covering every archive.

#### Scenario: Release contains supported artifacts

- **WHEN** a supported version tag is built for release
- **THEN** the release contains archives for `linux-x86_64`, `linux-arm64`, and `darwin-x86_64`, plus a checksum manifest

#### Scenario: Unsupported target is not advertised

- **WHEN** a release is inspected for a target outside the supported set
- **THEN** no installer or package metadata claims that target is supported

### Requirement: Shell installer

The project SHALL provide a non-interactive POSIX shell installer that accepts an optional version and destination directory, detects the host target, downloads the matching release archive over HTTPS, verifies its SHA-256 checksum, and installs `driftwatchdog` without requiring root by default.

#### Scenario: Install on a supported host

- **WHEN** the installer runs on Linux x86_64, Linux arm64, or macOS x86_64 with a valid release
- **THEN** it downloads the matching archive, verifies the checksum, installs the executable to the requested directory or user-writable default, and returns success

#### Scenario: Checksum mismatch

- **WHEN** the downloaded archive does not match the checksum manifest
- **THEN** the installer exits non-zero and does not replace an existing executable

#### Scenario: Unsupported host

- **WHEN** the installer runs on an unsupported operating system or architecture
- **THEN** it exits non-zero with the detected values and the supported target list

### Requirement: npm native launcher

The project SHALL publish an npm package with a CLI executable that selects the matching native release binary for Linux x86_64, Linux arm64, or macOS x86_64, verifies and caches it by version, and forwards all command-line arguments and exit status to that binary.

#### Scenario: npm installation and invocation

- **WHEN** a user installs the package globally and invokes its CLI on a supported host
- **THEN** the launcher obtains the matching verified native binary and forwards the invocation unchanged

#### Scenario: npm unsupported environment

- **WHEN** the launcher runs on an unsupported operating system or architecture
- **THEN** it exits non-zero with an actionable message naming supported targets and direct alternatives

#### Scenario: npm download verification failure

- **WHEN** the launcher cannot download or verify the native binary
- **THEN** it exits non-zero, does not execute the unverified file, and does not leave it as a valid cache entry

### Requirement: Installation documentation and alternatives

The README SHALL document the shell installer, npm installation, direct release downloads, and `cargo install`, including the supported target matrix and the fact that network access is used only during installation or binary acquisition.

#### Scenario: User chooses an installation method

- **WHEN** a user reads the installation section
- **THEN** they can select a documented command for shell, npm, direct download, or Cargo installation and understand the supported platforms

### Requirement: Reproducible release automation

The project SHALL provide automated release packaging for version tags that builds each supported target, creates the documented archive names, generates the checksum manifest from the final files, and publishes all release assets together.

#### Scenario: Tagged release packaging

- **WHEN** a valid version tag triggers the release workflow
- **THEN** all supported artifacts and one matching checksum manifest are built and attached, or the workflow fails before publishing incomplete assets

