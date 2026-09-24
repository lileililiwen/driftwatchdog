# Proposal: Add a language-agnostic product-quality Gate contract

## Why

Driftwatchdog can already execute arbitrary project commands and aggregate
their results, but the shared profiles do not identify the quality boundary
needed by AI-assisted projects: tests must stay out of product code and
placeholder debt must stay below an explicit threshold. Without a named
contract, projects either omit the check or invent incompatible local rules.

## What Changes

- Add a versioned generic Gate concern contract for
  `product-code-boundary` and `placeholder-threshold`.
- Add profile guidance so product, Rust-product, and other language profiles
  can select these concerns without embedding parsers in Driftwatchdog.
- Preserve arbitrary command execution: each concern remains bound to a
  project-owned checker command and uses the existing adapter/evidence path.
- Normalize command exit/report states into PASS, FAIL, REVIEW_REQUIRED, and
  NOT_APPLICABLE without allowing missing commands to appear as PASS.
- Add contract fixtures and integration tests for Workspace Governance's
  machine-readable report, malformed reports, missing commands, optional
  checks, and blocked aggregate behavior.

## BFS Impact Map

- **Capabilities:** Gate manifest resolution, profile selection, adapter
  normalization, aggregation, JSON output, and doctor readiness.
- **Users and flows:** project-local `driftwatch gate`, CI repetition, and
  consumers such as Forge that read the Gate JSON document.
- **Contracts/data/persistence:** add stable concern IDs and result mapping;
  existing `gate_runs` storage remains compatible and records the manifest
  digest/rule-pack identity already used by the Gate.
- **Integrations/configuration:** `.ai-gate/gate.yaml` and native manifests
  can bind commands; Workspace Governance owns the checker implementation.
- **Callers:** generic command adapter, aggregator, doctor, Gate JSON renderer,
  Forge's existing DriftWatch adapter, and project profiles.
- **Failure/boundary behavior:** command failure blocks; malformed output is
  `REVIEW_REQUIRED`; missing required command is not a pass; optional absence
  remains explicit and non-blocking; no network or tool installation occurs.
- **Tests:** manifest/profile fixtures, adapter output matrix, aggregate
  status matrix, JSON parity, and no-provider/no-command boundaries.
- **Dependencies:** Workspace Governance checker contract is an external
  command dependency; Driftwatchdog remains usable without it.
- **Compatibility/security:** existing generic checks and manifests remain
  valid; paths and diagnostics stay bounded and use existing redaction.

## Capabilities

- `product-quality-gate-contract`: Driftwatchdog recognizes and reports the
  shared product-quality concern IDs through the generic Gate pipeline.

## Non-goals

- No AST parser, language implementation, or source scanner inside
  Driftwatchdog.
- No automatic migration of every project manifest in this repository.
- No forced execution of a checker when a project has not selected the concern.
- No claim that a Gate pass proves runtime, integration, release, or product
  correctness.
- No Forge-specific API or storage change.
