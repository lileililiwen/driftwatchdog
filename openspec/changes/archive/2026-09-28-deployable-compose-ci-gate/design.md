# Design: Enforce deployable Compose and CI contracts

## Implementation boundary

The implementation is local to the Rust Driftwatchdog repository:

- `src/gate/concerns.rs`: stable IDs and `DEPLOYABLE_CONCERNS`.
- `src/gate/manifest.rs`: built-in `deployable` profile and diagnostics.
- `src/gate/mod.rs`: public exports.
- `tests/gate_deployable.rs`: manifest and execution integration coverage.
- `templates/deployable/`: starter gate, local CI, Compose, and GitHub Actions
  files.
- `templates/deployable/README.md`: bootstrap integration instructions.

Do not add Docker SDK code, CI-provider APIs, database migrations, or remote
branch-protection automation.

## Language and runtime

Rust on the repository's pinned Cargo toolchain. Verification commands are
`cargo fmt --check`, `cargo test`, `cargo clippy --all-targets --all-features
-- -D warnings`, `sh tests/packaging.sh`, and
`openspec validate --changes --strict --no-interactive`.

## Ownership and shared code

Driftwatchdog owns the generic profile and aggregation contract. The project
under test owns the actual `docker compose config`/smoke command and local CI
command. Bootstrap systems may copy the repository-relative templates. No
sibling project is modified.

## Behavioral model

| State | Meaning | Gate result |
|---|---|---|
| `PASS` | Both required project commands exist and exit zero | Gate may pass |
| `FAIL` | A required command exits non-zero | Gate blocks with `FAIL` |
| `REVIEW_REQUIRED` | A required command is absent, malformed, or cannot execute | Gate blocks by default |
| `NOT_APPLICABLE` | Only valid when the project did not select `deployable` | Existing behavior |

The `deployable` profile selects exactly `ci-contract` and
`compose-contract`, both required. It does not invent commands. A project
must bind each concern in `[[checks]]`; otherwise existing missing-command
aggregation records explicit review evidence and blocks.

## Contract and compatibility

Native manifest example:

```toml
version = 1
profile = "deployable"

[[checks]]
id = "ci-contract"
command = "./scripts/ci.sh"

[[checks]]
id = "compose-contract"
command = "docker compose -f compose.yaml config --quiet"
```

Existing profiles, concern IDs, manifest version, result envelope, and exit
codes are unchanged. The new profile is additive.

## Failure and boundary policy

- Missing command: `REVIEW_REQUIRED`, required check blocks.
- Empty command: manifest validation error before execution.
- Missing executable or non-zero command: existing project-runtime adapter
  maps the result to `REVIEW_REQUIRED` or `FAIL` according to its existing
  contract.
- Compose file missing: the declared command fails; Driftwatchdog does not
  fabricate a file or claim Compose readiness.
- CI workflow missing: the local command or template contract fails; local
  Gate does not claim remote CI success.
- Libraries and non-deployable projects: do not select the profile and remain
  unaffected.

## Verification oracle

- Unit tests verify profile identity, stable IDs, required defaults, and
  missing-command behavior.
- Integration tests verify a passing deployable manifest, a missing binding,
  and a failing Compose/CI command block the Gate.
- Packaging tests verify starter files are repository-relative and the CI
  template invokes the local Gate.
- Strict OpenSpec validation verifies the change package structure.

## Decision ledger

- Use a profile rather than global mandatory checks so non-deployable projects
  are not forced to adopt Docker.
- Keep commands project-owned rather than embedding Docker/CI semantics in
  the Rust core.
- Treat bootstrap templates as scaffolding, not runtime evidence.
- Remote branch protection is deferred to the consuming workspace/provider.
