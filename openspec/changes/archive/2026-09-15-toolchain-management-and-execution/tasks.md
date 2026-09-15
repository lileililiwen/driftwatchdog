## 1. BFS: Impact and Structure

- [x] 1.1 Map current runner, timeout/process-group behavior, installer, npm launcher, doctor, and platform matrix.
- [x] 1.2 Define manifest, cache, lock, trust, container, native, and project-runtime boundaries.
- [x] 1.3 Define network policy, artifact provenance, platform errors, and bootstrap UX.

## 2. DFS: Requirement Implementation

- [x] 2.1 Implement manifest parsing, platform resolution, cache layout, and atomic locks.
- [x] 2.2 Implement checksum/signature verification and offline cache reuse.
- [x] 2.3 Implement managed, container, native, and project-runtime execution backends.
- [x] 2.4 Add `bootstrap` and extend `doctor` with truthful readiness checks.
- [x] 2.5 Add tests for cache races, mismatch, unsupported platforms, timeouts, and missing runtimes.

## 3. BFS: Regression and Completion

- [x] 3.1 Re-check existing installer, npm launcher, release archives, and supported targets.
- [x] 3.2 Re-check process isolation, environment leakage, mounts, symlinks, and cancellation.
- [x] 3.3 Verify ordinary checks do not download tools without explicit policy.
- [x] 3.4 Run packaging, security, full Rust gates, strict OpenSpec validation, and local verification.
