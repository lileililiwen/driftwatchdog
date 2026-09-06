# Proposal: Add CI test/lint gates
## Why
Only release.yml and auto-tag.yml exist; no workflow runs `cargo test`, `cargo clippy`, or `cargo fmt` on push/PR, even though tests exist (tests/, src/*/tests). Also src/fingerprint/mod.rs notes ignore config is 'not implemented in v1' and silently ignored.
## What Changes
- Add .github/workflows/ci.yml running fmt --check, clippy -D warnings, and cargo test --workspace.
- Include the npm launcher tests in test/.

## Capabilities
### New Capabilities
- `ci-test-gates`: Rust and npm tests run automatically on every push/PR with lint gates.

### Modified Capabilities
None.

## Impact
Affects: .github/workflows/ci.yml.
