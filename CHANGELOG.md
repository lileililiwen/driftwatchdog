# Changelog

All notable changes to Driftwatchdog are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Added — v1.3 Release-evidence and capability-conformance Gate
- `driftwatch gate` gains a third built-in profile, `release`, that
  selects the two release-gate concern IDs `release-evidence` and
  `capability-conformance`. Both are routed through dedicated envelope
  adapters in `src/gate/adapters.rs`
  (`run_release_evidence_adapter`, `run_capability_conformance_adapter`)
  that share the product-quality wire version (`1`) and exit-code
  authority rule (`PASS`→0, `FAIL`→1, `REVIEW_REQUIRED`→2,
  `NOT_APPLICABLE`→0).
- `release-evidence` required-evidence guard: a `PASS` claim MUST
  include `revision`, `product_version`, at least one `artifacts`
  entry, and `provenance`; missing fields downgrade to
  `REVIEW_REQUIRED` naming the missing keys.
- `release-evidence` stale-revision guard: when the project exposes a
  current revision via `git` and the envelope's `revision` disagrees,
  a `PASS` claim is downgraded to `REVIEW_REQUIRED` with both
  revisions named in the diagnostic.
- `capability-conformance` required-evidence guard: a `PASS` claim
  MUST include a non-empty `capabilities.verified` list whose every
  id appears in the resolved plan.
- `capability-conformance` out-of-scope guard: verified ids outside
  the plan downgrade to `REVIEW_REQUIRED`.
- Malformed, missing, wrong-version, and contradiction envelopes all
  downgrade to `REVIEW_REQUIRED`; the release-gate adapters
  deliberately provide no text-mode fallback so missing coverage is
  never silently treated as a pass.
- New stable concern IDs `release-evidence` and `capability-conformance`
  in `src/gate/concerns.rs` plus the `RELEASE_GATE_CONCERNS` grouped
  slice and `is_release_gate_concern` /
  `is_release_evidence_concern` /
  `is_capability_conformance_concern` helpers.
- `src/gate/manifest.rs` adds the `release` profile to
  `profile_defaults` and `SUPPORTED_PROFILES` and records the two
  concerns in the `not_scheduled` explanation set for unrelated
  plans.
- Driftwatchdog remains an executor/aggregator: no release publisher,
  signer, SBOM generator, or deployment executor is added.
- 6 new `src/gate/concerns.rs` unit tests, 5 new
  `src/gate/manifest.rs` unit tests, 18 new
  `src/gate/adapters.rs` unit tests, 19 new
  `tests/gate_release_evidence.rs` integration tests.
- `driftwatch check`, `driftwatch mcp`, `gate.history`, the
  `gate_runs` schema, the export schema, the local schema version
  (still 6), and every non-release-gate concern path are
  byte-for-byte unchanged.

### Added — v1.2 Product-quality Gate
- `driftwatch gate` gains two built-in profiles, `product` and
  `rust-product` (alias), that select the stable concern IDs
  `product-code-boundary` and `placeholder-threshold`.
- `run_product_quality_adapter` parses a versioned JSON envelope
  (`PRODUCT_QUALITY_ENVELOPE_VERSION = 1`) and applies the exit-code
  authority rule (`PASS`→0, `FAIL`→1, `REVIEW_REQUIRED`→2,
  `NOT_APPLICABLE`→0). Status/exit mismatches, missing envelopes,
  unparsable envelopes, and unknown versions all downgrade to
  `REVIEW_REQUIRED` so missing coverage is never silently treated as
  a pass.
- A text-mode fallback preserves the existing `project-runtime`
  semantics for legacy commands (exit 0 → `PASS`, nonzero → `FAIL`).
- New `src/gate/concerns.rs` owns the data-level concern
  vocabulary; `src/gate/manifest.rs` adds the two profile names and
  records the concerns in `not_scheduled`.
- 4 new `src/gate/concerns.rs` unit tests, 17 new
  `src/gate/adapters.rs` unit tests, 15 new
  `tests/gate_product_quality.rs` integration tests, 5 new
  `src/gate/manifest.rs` unit tests.

### Added — v1.1 Engineering Gate
- Generic Gate contract (`src/gate/`): domain types, versioned JSON
  DTOs (`GATE_CONTRACT_VERSION = 1`), deterministic
  blocking-policy aggregation, bounded evidence references with
  secret redaction, and a checker-outcome compatibility adapter.
- Project Gate manifest (`src/gate/manifest.rs`, `gate.toml`):
  profiles, explicit checks, project commands, triggers, rule-pack
  identity, dry-run plan rendering, business `.ai-gate/gate.yaml`
  consumption (precedence `gate.toml` → `.driftwatch/gate.toml` →
  `.ai-gate/gate.yaml`).
- Bounded Gate evidence (`src/gate/evidence.rs`,
  `src/repo/evidence.rs`): `gate_artifacts` identity rows,
  idempotent insert-or-get, retention prune, `evidence_backed_pass`
  guard, export v3.
- Pinned tool manifests and offline-first execution
  (`src/gate/toolchain.rs`): `gate-tools.toml`, verified user
  cache, atomic locks, managed/container/native/project-runtime
  backends, opt-in `bootstrap` policy, `gate.toolchain.*` doctor
  checks.
- Adapter registry (`src/gate/adapters.rs`): checker-JSON/SARIF/
  Gitleaks/OSV/Semgrep normalizers, project-runtime text adapter,
  isolated execution, deterministic evaluator with rule-required
  available-evidence guard.
- Generic read-only context providers (`src/gate/context.rs`):
  Git + project-files + optional OpenSpec providers, bounded
  hashed documents, `gate.context.*` doctor checks.
- Provider-neutral AI evaluation (`src/gate/ai.rs`): opt-in
  redacted bounded fail-closed contract, external-command boundary,
  no embedded LLM, `gate.ai.*` doctor checks.
- Local `driftwatch gate` CLI (`src/commands/gate.rs` +
  `src/repo/gates.rs`): `--dry-run`, `--format human|json`,
  nonzero-when-blocked, `gate_runs` history (migration 0006),
  `gate.history` doctor check, export v4, AI-report Gate section.
- BFS-DFS-BFS change workflow (`AGENTS.md`): phase order,
  per-phase checkboxes, final-BFS verification, `change_workflow`
  packaging test.

### Added — v0.6 Integrate
- `driftwatch mcp`: stdio Model Context Protocol server with four
  read-only tools (`top_bugs`, `show_bug`, `ai_report`,
  `doctor_status`). JSON-RPC 2.0 / `protocolVersion 2024-11-05`,
  closed input schemas (`additionalProperties: false`), and a
  read-only SQLite connection. No new runtime dependencies.
- Agent example collateral under `examples/{claude-code,opencode,aider}/`:
  copy-paste `mcp.json` (stdio `driftwatch` server) plus an
  investigation `workflow.md` that mirrors the `report --ai`
  instructions. Aider's directory documents the `report --ai >
  drift.md` fallback for every Aider version. `examples/README.md`
  maps harnesses to their copy destination and restates the
  stdio-only, read-only caveat.
- GitHub Actions template at `templates/github-actions/driftwatch-check.yml`:
  reusable workflow (`workflow_call` + `workflow_dispatch`) that
  pins a `driftwatchdog` install, always renders
  `report --ai > drift.md`, uploads only `drift.md` (never
  `state.db` or command logs), summarises `top` into
  `$GITHUB_STEP_SUMMARY`, and exposes `version` /
  `fail_on_drift` / `upload_report` inputs under `contents: read`.
  `templates/github-actions/README.md` documents the inputs and
  the copy-vs-`uses:` decision.
- Quality gates: `[lints.clippy] all = "deny"`, `rust-version = "1.74"` MSRV,
  `cargo-deny` advisories/licenses policy, tarpaulin coverage, proptest
  seeds for the failure normalizer.
- CI breadth: `--all-features` clippy parity, macOS (`macos-14`) matrix,
  shellcheck, `npm audit`, and `scripts/smoke.sh` jobs.
- Release integrity: npm provenance (`id-token: write`), `macos-14`
  runner, tag==Cargo version assertion, SBOM (SPDX) attached to releases.
- UX: `driftwatch completions <shell>`, `driftwatch man`, per-command
  examples in `--help`, single-wrap errors with `hint: …`, honest
  `doctor` INFO severities, installer `--dry-run` docs + `cargo install`
  fallback, `driftwatch.toml.example` copy-paste checker.

### Added — `driftwatch check --format human|json` (v0.6 follow-up)
- Versioned `driftwatch-checker/0.1.0` JSON document on stdout in
  `--format json` mode (per-checker rows in declaration order, mapped
  statuses `ok` / `alerting` / `failed` / `timeout` / `protocol-error`,
  parsed `alerts[]` in the existing wire shape, bounded `error` note
  for failure modes, `summary` counts).
- Dry-run banner and correlation-skip warning move to stderr in JSON
  mode; human text, dry-run persistence, and exit-status semantics
  are byte-for-byte identical.
- New `src/checker/json.rs` (8 unit tests) + 8 new `tests/check.rs`
  integration tests. `gate --format json` and the existing checker
  protocol are untouched.

### Packaging tests
- `tests/packaging/test_change_workflow.sh` — `bfs-dfs-bfs-change-workflow`
  authoring check: phase order, per-phase checkboxes, final-BFS
  verification, spec Scenarios, negative missing-DFS fixture.
- `tests/packaging/test_agent_examples.sh` — every `mcp.json`
  parses, declares a stdio server with `driftwatch` + `mcp`, and
  every `driftwatch <sub>` token in the workflows exists in
  `driftwatchdog --help`; no `run`/`check` is allowed as an
  agent step.
- `tests/packaging/test_gha_templates.sh` — triggers, install
  base URL, `driftwatch check` + `report --ai` + step summary +
  upload-artifact anchors, upload `path: drift.md` only, and
  `fail_on_drift` documented; embedded `run:` blocks pass
  `shellcheck -S error` when the tool is available.

### Fixed
- Installer verification is mandatory (no skip flag); unsupported hosts
  print the target list plus the source-build fallback.
- Auto-tag skips with a notice when `REPO_PAT` is absent, avoids tag spam
  when the remote tag exists, and cold-starts at `0.1.0`.
</content>
</invoke>