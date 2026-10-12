# Tasks: mandatory-governance-quality-all-profiles

## Phase 1 — BFS: map and pin behavior

- [x] Record profile resolution ownership, fallback scope, unbound failure
  behavior, and explicit non-goals in the proposal and design.
- [x] Preserve unknown-profile typo rejection and existing declared
  project-profile resolution.
- [x] Pin all supported profiles, each workspace-only profile, one declared
  project profile, unknown typo rejection, and a required unbound fallback.

## Phase 2 — DFS: implement the mandatory fallback

- [x] Add required `governance-quality` once to every supported built-in,
  workspace, and project-defined profile that does not already include it.
- [x] Keep product, release, deployable and language-specific concern sets
  unchanged.
- [x] Verify an unbound fallback concern remains a required `UNBOUND` result.

## Phase 3 — BFS: regressions and evidence

- [x] Run formatting, full tests, clippy, strict OpenSpec validation, and the
  Driftwatchdog self Gate; record the three pre-existing self-Gate findings.
- [x] Build the local release binary and inspect Actoria's Gate plan; it
  schedules required `governance-quality` and leaves it `UNBOUND` until the
  workspace binder writes the project-owned command.
- [x] Record test outcomes and the observed Gate findings in `HANDOFF.md`.

This change remains active and is not archived, as directed by the workspace
handover.
