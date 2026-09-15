# Proposal: Toolchain management and execution

## Why

Gate adapters must not assume Semgrep, Gitleaks, OSV, Playwright, or similar
tools are already installed. Driftwatchdog needs reproducible managed,
container, native, and project-runtime execution with doctor/bootstrap support.

## What Changes

- Add versioned tool manifests, cache, checksum/signature verification, and locking.
- Add managed, container, native, and project-runtime execution backends.
- Add bootstrap and doctor diagnostics with fail-closed behavior.

## Capabilities

### New Capabilities

- `toolchain-management-and-execution`: reproducible tool preparation and execution.

### Modified Capabilities

## Impact

Affects process runner, doctor, configuration, release/distribution, platform
support, network policy, and tests. Depends on generic gate contract and project
configuration.

## Non-Goals

- No silent network access during ordinary checks.
- No installation of project runtimes such as .NET, Rust, or Python.
