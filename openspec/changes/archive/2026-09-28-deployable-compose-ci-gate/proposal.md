# Proposal: Enforce deployable Compose and CI contracts

## Why

Deployable projects can currently configure a DriftWatchdog Gate without
declaring a runnable container composition or a repeatable CI entry point.
Bootstrap guidance can create those files, but omission or later drift is not
detected by the shared Gate.

## What Changes

- Add a built-in `deployable` Gate profile with required `compose-contract`
  and `ci-contract` concerns.
- Keep the checks project-owned: DriftWatchdog validates the declared files
  and invokes the declared local commands; it does not become a CI server,
  Docker orchestrator, or remote provider client.
- Add bootstrap-facing templates for `gate.toml`, `scripts/ci.sh`, a Compose
  smoke configuration, and a GitHub Actions workflow that repeats the local
  Gate.
- Make missing mandatory declarations, files, commands, or evidence resolve
  to `REVIEW_REQUIRED`/`FAIL`, never an implicit pass.

## Package Boundary and Split Assessment

This is one package because the profile contract, local Gate execution,
bootstrap templates, and CI repeatability share one owner and one acceptance
boundary: a deployable repository can run the same mandatory contract locally
and in CI. Remote branch-protection configuration remains outside this
repository and is not implemented here.

## Sibling and Shared Architecture Reconnaissance

| Candidate | Evidence path/symbol | Reusable code/config/architecture | Compatibility gap | Owner and release boundary | Decision |
|---|---|---|---|---|---|
| workspace-governance | `.ai-gate/gate.yaml` convention; shared registry policy | Workspace policy and project declarations | Does not execute project commands or normalize Gate results | workspace-governance owns policy; Driftwatchdog owns execution | adapt through a generic adapter |
| Driftwatchdog | `src/gate/manifest.rs`, `src/commands/gate.rs`, `templates/github-actions/` | Manifest resolution, required-check aggregation, local/CI template boundary | Needs deployable profile and templates | Driftwatchdog release | extend shared owner |
| jenkins-local | deployment scripts and Compose discovery conventions | Adapter-specific deployment execution | Not portable and not a Gate contract | jenkins-local release | adapt through a generic adapter |

No sibling implementation is copied. The contract remains generic and
repository-relative so it can be consumed by GitHub Actions, Jenkins, or
another CI provider.

## BFS Impact Map

| Area | Impact |
|---|---|
| Gate vocabulary | Add two stable concern IDs and one built-in profile. |
| Manifest | Preserve existing schema; profile selection supplies required checks. |
| Execution | Run project-owned commands through the existing bounded runtime adapter. |
| Bootstrap | Provide repository-relative starter files and explicit opt-in guidance. |
| CI | Add a workflow that runs the local contract and Gate on push/PR. |
| Failure behavior | Missing command/file or non-zero command blocks required coverage. |
| Persistence | No schema or database change. Existing Gate results persist unchanged. |
| Compatibility | Existing profiles and manifests remain unchanged. |
| Security | No network or provider API is added; command output remains bounded/redacted. |

## Capabilities

- `deployable-compose-ci-contract`

## Non-goals

- Making Compose mandatory for libraries, documentation repositories, or
  non-deployable tools.
- Configuring remote branch protection.
- Starting long-lived services or publishing images.
- Inferring that a workflow file exists means CI passed.
- Replacing project-owned build, test, or deployment commands.
