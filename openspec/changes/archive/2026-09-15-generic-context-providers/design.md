## Context

The current product mentions external spec checkers but must remain usable by
projects with no OpenSpec. Context is input to planning and evaluation, not the
owner of requirements.

## Goals / Non-Goals

**Goals:**

- Define a generic provider returning bounded context documents and metadata.
- Include Git diff, changed files, and project-local documents.
- Add OpenSpec integration behind the same provider boundary.

**Non-Goals:**

- Making OpenSpec types part of `src/gate`.
- Editing, archiving, or validating a specification system from the provider.

## Decisions

- Providers implement discovery and read-only loading into generic
  `ContextDocument` values.
- Each document records kind, path, digest, size, and optional change id.
- Provider failures are explicit and can yield `REVIEW_REQUIRED` when required
  context is absent.
- OpenSpec parsing is optional and activated by configuration or CLI selection.
- Context collection is bounded, path-confined, and secret-safe.

## Risks / Trade-offs

- [Risk] Providers leak format-specific concepts → [Mitigation] keep them behind
  provider modules and generic document metadata.
- [Risk] Large specs consume context → [Mitigation] bounded selection and digest
  references with explicit truncation.
- [Risk] Missing context is mistaken for no requirements → [Mitigation] status it
  as unavailable, not empty success.
