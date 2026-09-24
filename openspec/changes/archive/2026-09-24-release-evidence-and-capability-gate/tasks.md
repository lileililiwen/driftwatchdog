# Tasks: Release evidence and capability Gate concerns

## 1. BFS — Baseline and impact coverage

- [x] Inventory Gate concern selection, result normalization, evidence storage,
  history checks, and JSON output.
- [x] Map capability and release evidence fields to generic Gate statuses and
  project blocking policy.
- [x] Add complete, missing, stale, contradictory, malformed, and optional
  command fixtures before implementation.

## 2. DFS — Requirement-by-requirement implementation

- [x] Add the two concern IDs and profile selection guidance.
- [x] Normalize capability/release reports with bounded evidence and redaction.
- [x] Add stale/contradictory/missing-evidence blocking behavior.
- [x] Add persistence, human output, JSON output, and remediation coverage.

## 3. BFS — Cross-surface regression and completeness

- [x] Re-run all existing Gate, doctor, adapter, and checker-only tests.
- [x] Verify native manifests, business YAML precedence, foreign-runtime
  refusal, dry-run, and history behavior.
- [x] Verify Driftwatchdog remains an executor/aggregator and does not become a
  release publisher or package manager.

## 4. Verification

- [x] `cargo fmt --check && cargo test && cargo clippy --all-targets
  --all-features -- -D warnings`.
- [x] Run the applicable local Gate and record its actual status.
- [x] `openspec validate --changes --strict --no-interactive`.
