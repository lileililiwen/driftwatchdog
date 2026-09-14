## Context

The existing `driftwatch.toml` configures low-level checkers. A Gate manifest
must express policy without forcing every project to use the same language,
framework, or specification system.

## Goals / Non-Goals

**Goals:**

- Validate a project-local `.ai-gate/gate.yaml` or equivalent manifest.
- Resolve named profiles and explicit project commands.
- Support local rule overrides without copying shared runtime code.
- Make configuration errors actionable and deterministic.

**Non-Goals:**

- Automatic semantic inference as the source of truth.
- OpenSpec-specific fields in the core schema.

## Decisions

- Keep existing `driftwatch.toml` backward compatible; introduce Gate config as
  a separate namespace or explicit section during migration.
- A manifest contains profile, selected checks, blocking statuses, project
  runtime commands, rule-pack version, and context-provider declarations.
- Profiles are runtime-provided defaults; the project may enable/disable checks
  and provide bounded overrides.
- Missing optional concern configuration yields `NOT_APPLICABLE`, while a
  malformed enabled check fails before execution.
- Unknown fields fail with actionable diagnostics.

## Risks / Trade-offs

- [Risk] Two config files confuse users → [Mitigation] document precedence and
  provide `doctor` output naming both.
- [Risk] Profiles hide checks → [Mitigation] expose the resolved plan in dry run.
- [Risk] Local overrides bypass policy → [Mitigation] restrict override fields
  and record manifest/rule-pack hashes.
