# Tasks: Enforce common repository source-file size boundaries

## 1. BFS — Baseline and impact coverage

- [x] 1.1 Map `source-file-size` through concern vocabulary, `product` and
  `rust-product` profile resolution, manifest digesting, Gate dispatch,
  aggregation, human/JSON output, and existing `gate_runs` persistence.
- [x] 1.2 Confirm the source-boundary contract: Git-listed paths plus
  `.gitignore`/Git exclusions, default dependency/build/vendor exclusions,
  conventional source-root discovery, explicit repository-relative include /
  exclude overrides, and symlink/binary handling.
- [x] 1.3 Add test fixtures covering exactly 999, 1,000, and 1,001 raw
  newline-delimited physical lines, including a final unterminated line.
- [x] 1.4 Confirm the implementation-handoff boundary: Rust 2021/MSRV 1.74,
  `src/gate/source_size.rs` owns scanning, `src/commands/gate.rs` only routes
  the built-in concern, and no project command or sibling repository changes
  are required.

## 2. DFS — Requirement-by-requirement implementation

- [x] 2.1 Add `SOURCE_FILE_SIZE`, product-quality recognition, profile defaults,
  and focused vocabulary/profile tests.
- [x] 2.2 Add strict `[source_size]` manifest parsing and validation for
  `max_lines`, repository-relative `include`, and `exclude`, including digest
  and invalid-pattern tests.
- [x] 2.3 Implement Git-aware source candidate discovery with
  `.gitignore`/Git exclusion handling, conventional source roots, default
  dependency/build/vendor exclusions, configured glob overrides, path
  normalization, symlink refusal, and deterministic ordering.
- [x] 2.4 Implement raw newline counting and normalized `GateResult` creation:
  PASS at or below the limit, FAIL with bounded findings above it, and
  REVIEW_REQUIRED for boundary/read failures without aborting other reads.
- [x] 2.5 Route the built-in concern through the in-process scanner and retain
  the existing project-runtime adapter for every other concern.
- [x] 2.6 Add unit and integration tests for successful, failing, ignored,
  configured-boundary, review, dry-run, JSON, persistence, and opt-out cases.
- [x] 2.7 Update README and relevant Gate documentation with the common
  `source-file-size` concern, default 1,000-line policy, raw `wc -l` semantics,
  Git boundary, and configuration examples.

## 3. BFS — Cross-surface regression and completeness

- [x] 3.1 Verify existing product-quality command concerns still use their
  project-owned command bindings and their envelope/exit-code behavior is
  unchanged.
- [x] 3.2 Verify existing profiles, `.driftwatch/gate.toml` precedence,
  `.ai-gate/gate.yaml` foreign-runtime refusal, checker-only `driftwatch check`,
  and no-manifest no-op behavior remain unchanged.
- [x] 3.3 Verify required FAIL and REVIEW_REQUIRED results block according to
  existing aggregation policy, while explicit optional/disabled policy remains
  visible and non-blocking.
- [x] 3.4 Review source paths, findings, diagnostics, config patterns, and
  serialized output for traversal, symlink, unbounded-output, machine-path,
  and source-content leakage risks.
- [x] 3.5 Confirm all current-change scenarios map to the capability spec and
  remove any implementation placeholder or unowned decision.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo test`, and
  `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] 4.2 Run the focused source-size integration suite and the repository
  packaging suite, recording exit codes and assertions for PASS, FAIL, and
  REVIEW_REQUIRED boundaries.
- [x] 4.3 Run `openspec validate --changes --strict --no-interactive`.
- [x] 4.4 Run `git diff --check` and record the implementation handoff:
  this package is complete only when the common scanner, Gate wiring, tests,
  and documentation are implemented and verified; strict validation alone is
  not runtime enforcement evidence.
