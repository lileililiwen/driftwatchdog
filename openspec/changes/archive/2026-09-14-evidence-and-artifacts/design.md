## Context

Evidence must remain useful after a run without turning `.driftwatch/` into an
unbounded log store or leaking secrets. It must work for command output, files,
JSON/SARIF, screenshots, traces, and provider responses.

## Goals / Non-Goals

**Goals:**

- Store typed metadata, content hashes, bounded previews, and artifact paths.
- Capture source and tool provenance.
- Support cleanup while preserving result identity and audit summaries.

**Non-Goals:**

- Public cloud artifact storage.
- Treating a hash as proof that content is safe or correct.

## Decisions

- Use an artifact directory under `.driftwatch/` with generated run identity.
- Every artifact records kind, producer, version, timestamp, size, digest, and
  redaction status.
- Results reference artifacts; previews are capped and UTF-8 safe.
- Sensitive environment values and recognized secret patterns are redacted before
  persistence; failed redaction is safer than storing raw content.
- Retention removes bulky artifacts while preserving result summaries and hashes.

## Risks / Trade-offs

- [Risk] Evidence collection creates large files → [Mitigation] caps, retention,
  and explicit artifact classes.
- [Risk] Redaction removes useful context → [Mitigation] preserve locations and
  stable fingerprints while omitting secret values.
- [Risk] Artifact paths escape state directory → [Mitigation] canonical root
  checks and path confinement.
