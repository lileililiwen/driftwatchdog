## ADDED Requirements

### Requirement: Every change records three execution phases

Every implementation change MUST record BFS impact analysis, DFS behavior
implementation, and BFS regression/completion review in `tasks.md`.

#### Scenario: Missing phase

- **WHEN** a change task list omits one of the three phases
- **THEN** the change is not ready for implementation and validation reports the
  missing planning requirement through repository review

#### Scenario: Completion before regression review

- **WHEN** implementation tasks are complete but the final BFS tasks are open
- **THEN** the change MUST NOT be archived or represented as complete

### Requirement: Planning artifacts have distinct responsibilities

`proposal.md` MUST describe the impact map, `design.md` MUST define boundaries,
and `spec.md` MUST define observable behavior and scenarios.

#### Scenario: Boundary missing from design

- **WHEN** a change crosses a process, storage, or external-provider boundary
  without documenting ownership and failure behavior
- **THEN** the design is incomplete and the change remains planning-only

#### Scenario: Scenario lacks verification

- **WHEN** a requirement scenario has no executable verification path
- **THEN** the final BFS review MUST record the gap and block completion
