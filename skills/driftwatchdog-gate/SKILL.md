---
name: driftwatchdog-gate
description: Use when configuring or diagnosing a portfolio Gate, binding project checks, exporting Gate evidence to Forge, or deciding whether a project has sufficient local verification to advance.
---

# Driftwatchdog Gate

Driftwatchdog owns Gate plan resolution, execution, blocking policy, run
history, and evidence export. Projects own their check commands and policy
declarations. Forge consumes the versioned observation and records its source
revision; it does not reinterpret a failed or missing run as PASS.

## Gate path

1. Read `README.md`'s built-in profile and concern catalog and inspect the
   project's `gate.toml` or `.ai-gate/gate.yaml`. The current code in
   `src/gate/concerns.rs` is authoritative if documentation disagrees.
2. Resolve the profile and every scheduled required concern. Bind a real
   project command or use an allowed, explicit not-applicable reason. A
   required unbound check is `UNBOUND` and blocks; a missing command is not a
   successful check.
3. Run a dry plan preview, then the local Gate and any underlying command whose
   result needs review. Preserve `FAIL`, `REVIEW_REQUIRED`, malformed,
   unavailable, stale, and revision mismatch states. Do not retire a question
   with a checker that performs no comparison.
4. Export versioned evidence for Forge only after inspecting the real run and
   its revision. Keep the project's native tests, Gate verdict, and Forge's
   persisted observation distinguishable. Template presence or a build alone
   is not release or runtime evidence.
5. For Gate implementation changes, follow `AGENTS.md`: select one OpenSpec
   change, test behavior, run the required local checks, archive, and hand off
   with separate commits. Keep generic Gate core language-agnostic.

Use current `README.md` examples and the selected canonical spec for exact
commands and envelope versions. Never put shared Gate adapters or tool
installers into individual product repositories.
