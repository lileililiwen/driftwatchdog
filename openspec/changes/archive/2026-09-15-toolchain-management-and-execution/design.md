## Context

The current runner executes commands from `PATH`. This is sufficient for
configured checkers but not for a shared Gate Runtime used on new machines.

## Goals / Non-Goals

**Goals:**

- Resolve a pinned tool version and execution mode before running an adapter.
- Verify downloaded bytes and container identity.
- Keep project-runtime commands distinct from Gate-tool dependencies.
- Provide actionable doctor/bootstrap output.

**Non-Goals:**

- Installing SDKs required by the monitored project.
- Docker-in-Docker abstraction or a universal sandbox.

## Decisions

- Manifest entries contain tool id, version, platform, source, digest/signature,
  and allowed execution modes.
- Managed binaries use a user cache with atomic download, verification, and
  per-version locks; failed verification never executes.
- Containers use an explicit image digest and bounded mounts/environment.
- Native execution is opt-in or an explicitly configured fallback.
- Project-runtime execution uses the project's declared argv without shell
  splitting; the runtime must be present for the project to build/test.
- Bootstrap is explicit; normal gate execution is offline unless policy permits
  provisioning.

## Risks / Trade-offs

- [Risk] Downloads create supply-chain exposure → [Mitigation] pinned manifests,
  checksums/signatures, HTTPS, cache isolation, and audit output.
- [Risk] Containers are unavailable → [Mitigation] doctor reports capability and
  the plan selects an allowed fallback or fails clearly.
- [Risk] Platform matrix grows → [Mitigation] manifest resolution rejects
  unsupported targets before execution.
