## Context

The correlator must produce useful leads without pretending that text similarity proves causation. Inputs include bug canonical message/summary, alert message, source file, symbol, and run tags.

## Goals / Non-Goals

**Goals:**
- Calculate a deterministic score using token overlap/Jaccard plus Jaro-Winkler or Levenshtein-style similarity.
- Use the initial weighting: 0.50 message similarity, 0.20 symbol, 0.20 file, 0.10 tag.
- Show matches only at score >= 0.65, with manual links taking precedence.
- Implement `link`, `unlink`, and `report --ai` sections: project context, recurring failures, current violations, possible relationships, and task for AI.

**Non-Goals:**
- Embeddings, vector databases, LLM APIs, automatic edits, or root-cause claims.

## Decisions

Persist algorithm version and score with each correlation so changes can be explained and recalculated. Use labels “Possible relationship”, “Possible match”, and “Heuristic correlation”. Manual links are explicit user assertions and remain visible even when a heuristic score is below threshold. The AI report instructs agents not to fix only the latest occurrence or silence a warning by changing the specification.

## Risks / Trade-offs

String similarity can create false positives, especially for generic errors. The threshold, component scores, source/symbol matching, and cautious wording make uncertainty visible. Algorithm versioning allows future changes without corrupting historical evidence.

## ADDED Requirements

### Requirement: Calculate heuristic correlations
The correlator MUST calculate a deterministic score from bug/alert message, symbol, source file, and tag similarity using weights 0.50, 0.20, 0.20, and 0.10 respectively, and MUST expose the component scores and algorithm version.

#### Scenario: Match above threshold
- **WHEN** a bug and alert score 0.78
- **THEN** a correlation is persisted and displayed as a possible relationship with score 0.78, not as a root cause

#### Scenario: Match below threshold
- **WHEN** a candidate scores below 0.65 and has no manual link
- **THEN** it is not displayed as an automatic relationship

### Requirement: Support manual links
`driftwatch link bug:<id> spec:<alert-or-source>` MUST create an explicit link, and `unlink` MUST remove only the selected link after validating its target.

#### Scenario: Manual link overrides heuristic
- **WHEN** a user links a bug to an alert below the automatic threshold
- **THEN** the relationship is displayed as user-confirmed/manual and remains distinguishable from heuristic correlations

### Requirement: Generate AI context report
`driftwatch report --ai` MUST generate Markdown containing project context, recurring failures, current spec violations, possible relationships, and a task section instructing the agent to investigate recurrence, review related specs, inspect prior implementations, add/update regression tests, and not modify specs merely to silence warnings.

#### Scenario: AI report with correlated failure
- **WHEN** recurring bugs and alerts exist
- **THEN** the report includes stable IDs, counts, recent commits, alert sources/messages, scores, cautious relationship labels, and the investigation instructions

#### Scenario: AI report with no alerts
- **WHEN** no checker alerts or correlations exist
- **THEN** the report states that no current spec violations or possible relationships were found and still includes recurring failure context
