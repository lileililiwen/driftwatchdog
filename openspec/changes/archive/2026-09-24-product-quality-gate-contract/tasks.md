# Tasks: Product-quality Gate contract

## 1. BFS — Baseline and impact coverage

- [x] 1.1 Inventory manifest parsing, built-in/domain profiles, command bindings,
  adapter result normalization, aggregation, Gate JSON, persistence, and
  doctor readiness. Existing profiles (`backend`/`frontend`/`full`/`minimal`)
  use opaque concern-id strings; built-in profile selection is data-only.
- [x] 1.2 Map selected/unbound/optional/foreign-runtime paths to existing
  Gate status vocabulary and blocking policy. Required + missing command
  already aggregates to `REVIEW_REQUIRED`; optional + missing is
  `NOT_APPLICABLE`. New product-quality concerns inherit this behaviour.
- [x] 1.3 Define the product-quality JSON envelope (versioned, status claim
  + findings + bounded diagnostic, etc.) and the exit-code authority
  rule: `PASS`→0, `FAIL`→1, `REVIEW_REQUIRED`→2, `NOT_APPLICABLE`→0;
  a mismatch returns `REVIEW_REQUIRED` and the exit code is
  authoritative. Malformed output also returns `REVIEW_REQUIRED`.
- [x] 1.4 Define the new built-in profile names (`product`,
  `rust-product`) and the concern-id vocabulary
  (`product-code-boundary`, `placeholder-threshold`) without embedding
  any provider SDK or Workspace-Governance types in Driftwatchdog.

## 2. DFS — Requirement-by-requirement implementation

- [x] 2.1 Add `src/gate/concerns.rs` with the stable concern IDs and
  `is_product_quality_concern(id)` helper. Re-export from
  `src/gate/mod.rs`.
- [x] 2.2 Extend `src/gate/manifest.rs` with the `product` and
  `rust-product` built-in profiles, register them in
  `SUPPORTED_PROFILES`, and add unit tests.
- [x] 2.3 Add the product-quality JSON envelope parser and adapter
  function (`run_product_quality_adapter`) to
  `src/gate/adapters.rs`. Envelope shape: `{ "version": 1, "status":
  "PASS|FAIL|REVIEW_REQUIRED|NOT_APPLICABLE", "severity": ..., "findings":
  [...], "evidence": [...], "missing_evidence": [...], "diagnostic": ...,
  "remediation": ... }`. Exit-code authority rule and malformed
  fallback implemented there. Unit tests for the full mapping matrix.
- [x] 2.4 Route product-quality concerns in `src/commands/gate.rs`
  `execute_plan` to `run_product_quality_adapter`; existing
  `project-runtime` adapter and every other concern path remain
  byte-for-byte identical (text-mode fallback unchanged).
- [x] 2.5 Add integration tests covering: passing envelope, failing
  envelope, review envelope, malformed envelope, contradictory
  envelope, missing required command, optional without command, and
  the new `product`/`rust-product` profiles. Persistence parity and
  `--format json` parity (status/severity/diagnostic).
- [x] 2.6 Update `README.md` with manifest examples for the new
  profiles and concerns, and document the envelope contract shape and
  exit-code authority rule.

## 3. BFS — Cross-surface regression and completeness

- [x] 3.1 Re-run all existing Gate and checker-only tests to prove
  generic command behaviour and `driftwatch check` are unchanged.
- [x] 3.2 Re-verify native `gate.toml`, `.driftwatch/gate.toml`, and
  `.ai-gate/gate.yaml` precedence, foreign-runtime refusal, dry-run
  no-write, and doctor history readiness.
- [x] 3.3 Re-run the packaging suite (`tests/packaging.sh`) to confirm
  the `change_workflow` shape test, agent examples, and GHA templates
  still pass.
- [x] 3.4 Update `ROADMAP.md`, `README.md`, and `HANDOFF.md` only when
  implementation is complete; this planning package must not claim
  runtime enforcement before the change is archived.

## 4. Verification

- [x] 4.1 `cargo fmt --check && cargo test && cargo clippy --all-targets
  --all-features -- -D warnings`.
- [x] 4.2 Run the applicable local Gate and record PASS/FAIL/BLOCKED
  evidence with a temporary fixture project.
- [x] 4.3 `openspec validate --changes --strict --no-interactive`.
- [x] 4.4 Confirm `tests/packaging.sh` (incl. `change_workflow`) is
  green.
