## Context

Driftwatchdog is currently distributed as source through Cargo. The repository has a single Rust binary and no release packaging, installer, or npm surface. The distribution change must keep that binary as the only implementation, remain offline after installation, and support the requested Linux and Intel macOS environments.

## Goals / Non-Goals

**Goals:**

- Publish reproducible release archives for Linux x86_64, Linux arm64, and macOS x86_64 (Intel).
- Provide a non-interactive shell installer suitable for `curl -fsSL URL | bash`.
- Provide an npm package whose executable downloads and invokes the matching native binary.
- Verify downloads with a release checksum manifest and fail closed on mismatch.
- Support explicit version selection, clear unsupported-platform errors, and user-writable installation defaults.
- Keep direct release downloads and `cargo install` documented as alternatives.

**Non-Goals:**

- No JavaScript reimplementation of Driftwatchdog.
- No Windows, macOS arm64, Linux musl, package-manager formulas, container images, or system-service installation in this change.
- No runtime auto-update, telemetry, network upload, or change to CLI/state behavior.

## Decisions

### Release artifact contract

Use GitHub Releases for versioned archives and a checksum manifest. Each archive contains the `driftwatchdog` executable and uses stable target names: `driftwatchdog-<version>-linux-x86_64.tar.gz`, `driftwatchdog-<version>-linux-arm64.tar.gz`, and `driftwatchdog-<version>-darwin-x86_64.tar.gz`. The manifest is generated from the final archives and is the integrity source for installers.

Alternative considered: publish binaries only in a package registry. Rejected because direct downloads and the shell installer need a simple public, versioned artifact source.

### Shell installer

Implement a POSIX-compatible shell script hosted from the repository/release surface. It detects `uname -s` and `uname -m`, maps them to the supported artifact names, accepts an optional version and destination directory, downloads the archive and checksum manifest using `curl` or `wget`, verifies SHA-256 before extraction, installs atomically, and prints the resulting path. The default destination is a user-writable directory such as `${XDG_BIN_HOME:-$HOME/.local/bin}`; it must not require root.

Alternative considered: always install to `/usr/local/bin`. Rejected because it requires elevated permissions and makes `curl | bash` unsafe or unusable in locked-down environments.

### npm distribution

Publish a small package under the project package name with a `bin` entry for `driftwatch`. Its launcher detects Linux x86_64, Linux arm64, or macOS x86_64, resolves the requested package version to the matching GitHub Release, downloads and verifies the native archive into an npm-managed cache, and executes it with the original arguments. It must not run arbitrary downloaded content and must provide an actionable error for unsupported platforms or failed verification.

Alternative considered: publish separate npm packages per target. Rejected for the initial release because one package with deterministic target selection is simpler for users and release maintenance.

### Release automation and testing

Use a tag-triggered GitHub Actions workflow with a matrix for the three targets, build in release mode, package, generate checksums, and attach all artifacts to the release. Tests will exercise target mapping and archive/checksum logic locally without requiring a network; an opt-in smoke job can validate the installer against a staged release fixture.

Alternative considered: rely on manual release commands. Rejected because a distribution contract is only useful if artifacts and checksums are repeatable.

## Risks / Trade-offs

- [Release hosting changes] → Centralize repository, release URL, and artifact naming in shared scripts/configuration and test the generated URLs.
- [Architecture detection misses aliases] → Accept common `uname` aliases, reject unknown values explicitly, and cover mappings with table-driven tests.
- [Pipe-to-shell trust and transport failure] → Use HTTPS URLs, checksum verification, strict shell error handling, no shell evaluation of downloaded data, and document the direct-download alternative.
- [npm cache contains stale or partial binaries] → Use versioned cache paths, temporary downloads, checksum verification before rename, and retry-safe cleanup.
- [Native binary compatibility differs across Linux systems] → Build and document glibc-based Linux targets; defer musl support rather than implying universal Linux compatibility.

## Migration Plan

1. Add packaging scripts, npm launcher/package files, release workflow, and tests.
2. Update README installation guidance with the new channels while retaining Cargo installation.
3. Validate locally, then publish a tagged release and inspect all three artifacts and checksums.
4. Roll back by deleting or superseding the release and reverting the workflow/package changes; existing Cargo installation remains available.

## Open Questions

- The npm package name and whether the executable should be named `driftwatch` or `driftwatchdog` must be finalized during implementation. The design assumes package name `driftwatchdog` and command alias `driftwatchdog` unless repository conventions require otherwise.
