## Context

The current `check` command runs configured external checkers and persists
snapshots. A broader Gate command should orchestrate resolved plans locally,
persist typed results, and remain compatible with existing history and reports.

## Goals / Non-Goals

**Goals:**

- Make local Gate execution the first completion verification.
- Add `gate`, `gate --dry-run`, and machine-readable output.
- Store Gate runs, result summaries, evidence references, and change identity.
- Expose Gate status through doctor, reports, export, and read-only MCP where safe.

**Non-Goals:**

- CI-only enforcement.
- Mutating OpenSpec or project files.
- Arbitrary agent execute tools through MCP.

## Decisions

- `driftwatch gate` resolves configuration, context, tools, adapters, and
  evaluators, then applies aggregate blocking policy.
- `driftwatch check` remains a compatibility entry point for legacy checker-only
  projects.
- Gate snapshots reference project root, revision, manifest/rule digests, tool
  versions, statuses, and evidence ids.
- Local gate failure returns nonzero before archive; CI templates repeat the same
  command and may publish safe summaries/artifacts.
- Reports distinguish historical runtime failures from current Gate failures and
  never treat a heuristic relationship as root-cause proof.

## Risks / Trade-offs

- [Risk] Gate storage complicates current schema → [Mitigation] additive,
  transactional migrations and read-compatible exports.
- [Risk] Gate command becomes a second orchestration path → [Mitigation] reuse
  common plan/result builders and keep `check` as an adapter.
- [Risk] Local tool unavailable → [Mitigation] doctor/bootstrap diagnostics and
  explicit blocked status before completion.
