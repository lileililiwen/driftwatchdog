## 1. BFS: Impact and Structure

- [x] 1.1 Update `AGENTS.md` and `HANDOFF.md` with the three-phase workflow.
- [x] 1.2 Document proposal/design/spec/task responsibilities and local-before-CI verification.
- [x] 1.3 Add authoring checks or review checklist for phase presence and task verifiability.

## 2. DFS: Requirement Implementation

- [x] 2.1 Apply the workflow wording to repository guidance without changing runtime behavior.
- [x] 2.2 Add tests or validation fixtures for missing phases and incomplete scenarios.

## 3. BFS: Regression and Completion

- [x] 3.1 Re-read proposal, design, spec, and tasks for consistency.
- [x] 3.2 Verify existing OpenSpec changes and archived workflow documentation are not contradicted.
- [x] 3.3 Run `openspec validate --changes --strict --no-interactive` and `git diff --check`.
- [x] 3.4 Confirm the change remains documentation-only and no implementation task is claimed complete.
