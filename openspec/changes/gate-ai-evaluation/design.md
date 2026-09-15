## Context

AI review must be a typed evaluator over rules and evidence, not an unrestricted
repository chat. Provider availability and evidence quality must remain visible
in the result.

## Goals / Non-Goals

**Goals:**

- Define provider-neutral request/response DTOs.
- Require `PASS`, `FAIL`, `REVIEW_REQUIRED`, or `NOT_APPLICABLE`.
- Require evidence references and missing-evidence reporting.
- Keep provider calls opt-in, bounded, redacted, and auditable.

**Non-Goals:**

- LLM selection, prompt marketplace, or autonomous code modification.

## Decisions

- AI receives rule text, selected context, evidence references/previews, and a
  required JSON schema.
- `PASS` requires explicit evidence and valid schema; invalid output is
  `REVIEW_REQUIRED`.
- Secrets are redacted before provider submission; raw artifacts are not sent by
  default.
- Provider failure is distinguishable from rule failure and follows configured
  blocking policy.
- AI outputs are stored as evidence-backed evaluations with model/provider id
  and prompt/rule digest.

## Risks / Trade-offs

- [Risk] Model variance harms reproducibility → [Mitigation] pin prompt/rule
  digests, store provider metadata, and prefer deterministic checks.
- [Risk] Sensitive code leaves the machine → [Mitigation] opt-in provider,
  redaction, previews, and doctor disclosure.
- [Risk] AI overclaims → [Mitigation] schema validation and no-evidence-is-no-pass.
