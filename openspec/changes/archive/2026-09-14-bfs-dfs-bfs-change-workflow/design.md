## Context

Driftwatchdog currently uses proposal, design, spec, and task artifacts, but the
repository does not require a consistent impact-analysis order. This change
establishes a documentation-only workflow that remains valid for runtime
memory, Gate, tooling, and context-provider changes.

## Goals / Non-Goals

**Goals:**

- Make impact discovery explicit before deep implementation.
- Make architecture boundaries reviewable before code changes.
- Make regression and completeness review explicit before archive.

**Non-Goals:**

- No new CLI command, schema, or runtime dependency.
- No requirement that every project use a Gate Runtime.

## Decisions

- `proposal.md` records why, scope, dependencies, non-goals, and the BFS impact
  map.
- `design.md` records module ownership, contracts, data flow, persistence,
  process boundaries, failure behavior, security, compatibility, and migration.
- `tasks.md` is divided into `BFS: Impact and Structure`, `DFS: Requirement
  Implementation`, and `BFS: Regression and Completion`.
- `spec.md` remains behavior-focused; it does not duplicate the workflow.
- The final BFS must run before archive and must include local verification plus
  strict OpenSpec validation.

## Risks / Trade-offs

- [Risk] Planning becomes verbose → [Mitigation] keep proposal concise and put
  technical detail in design.
- [Risk] Tasks become generic checklists → [Mitigation] require each task to
  name a verifiable artifact, behavior, caller, or command.
- [Risk] CI is mistaken for local completion → [Mitigation] require local
  verification before archive; CI repeats it later.
