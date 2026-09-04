## 1. Packaging contract and build tooling

- [x] 1.1 Define the supported target matrix, version source, archive names, release URLs, and checksum manifest format in shared packaging configuration.
- [x] 1.2 Add reproducible scripts to build the Rust binary in release mode, stage each target archive, and generate SHA-256 checksums from the final archives.
- [x] 1.3 Add table-driven tests for target detection, artifact naming, version resolution, and unsupported operating-system/architecture errors.

## 2. Shell installer

- [x] 2.1 Implement a strict POSIX shell installer with optional version and destination arguments, HTTPS downloads, user-writable default destination, and actionable diagnostics.
- [x] 2.2 Add checksum verification, temporary-file handling, atomic replacement, executable permissions, and protection against replacing an existing binary after verification failure.
- [x] 2.3 Add fixture-based installer tests covering supported targets, checksum mismatch, missing tools/download failures, unsupported hosts, and explicit destinations.

## 3. npm launcher

- [x] 3.1 Add npm package metadata, package inclusion rules, and the `driftwatchdog` executable entry point without duplicating CLI behavior in JavaScript.
- [x] 3.2 Implement platform/architecture selection, release URL construction, versioned cache management, checksum verification, and safe cleanup for failed downloads.
- [x] 3.3 Forward all arguments and the native process exit status; add tests for supported targets, unsupported environments, download failures, verification failures, and cache reuse.

## 4. Release automation

- [x] 4.1 Add a tag-triggered GitHub Actions workflow that builds Linux x86_64, Linux arm64, and macOS x86_64 artifacts with the documented names.
- [x] 4.2 Publish the generated archives and checksum manifest together, failing before publication when any matrix build or checksum step fails.
- [x] 4.3 Add a release smoke test against staged/local fixtures so packaging can be checked without relying on a live release.

## 5. Documentation and verification

- [x] 5.1 Update README.md with shell, npm, direct-download, and Cargo installation commands plus the supported-platform matrix and installation network note.
- [x] 5.2 Run formatting, unit/integration tests, clippy with warnings denied, and strict OpenSpec validation; record results in HANDOFF.md when implementation is complete.
