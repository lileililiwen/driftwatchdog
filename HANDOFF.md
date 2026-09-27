# Driftwatchdog Handoff

## Change completion workflow

After implementing a change and ticking every box in its `tasks.md`, follow the closing sequence in `AGENTS.md` ("Change completion workflow"):

1. Verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
2. Confirm `openspec/changes/<change>/tasks.md` has every box ticked.
3. `openspec archive <change> -y`; capability specs are promoted when the
   change contains `spec.md`.
4. `git add -A && git commit -m "Implement <change>"`.
5. Update this file (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after <change>"`.
6. Re-run `openspec validate --changes --strict --no-interactive`.

## Current state

The `gate-evidence-export` change is **implemented and archived**
(2026-09-27): a new `driftwatch gate evidence-export` subcommand
(`--format human|json`, `--dry-run` for documentation) reads the
latest completed `gate_runs` row and prints a versioned
`EvidenceExport` document in the workspace-governance
`release_evidence` vocabulary. The export carries a fixed top-level
shape (`schema_version: 1`, `project_id` from the project directory
basename, `revision` from the gate run, `toolchain` as
`driftwatchdog@<cargo_version>`, the manifest digest, rule-pack
version, and `gate_run_id`) plus one entry per governance field
(`revision`, `version`, `toolchain`, `artifacts`, `digests`, `sbom`,
`provenance`, `checks`, `publication`) carrying `state` in the
closed set `verified` / `unverified` / `blocked`, an optional
`evidence_ref` into the bounded-evidence store, and an optional
`source_gate` naming the check that produced the verdict. The
per-field state priority is `verified > blocked > unverified`,
`verified` is reserved for scheduled checks that actually ran and
passed and the run revision matches the project revision, and a
stale run emits every field `unverified` with a `stale` diagnostic
naming both revisions. Unknown governance field names are an
`ExportError::UnknownField` construction error. The export refuses
with a non-zero exit when no run exists for the project. The new
module lives in `src/gate/evidence_export.rs` (closed field and
state constants, `Field` constructor with unknown-name refusal,
`FieldExport` / `EvidenceExport` / `StaleDiagnostic` structs, the
`build` reducer, human and JSON renderers, and 23 unit tests
covering the field/state closed sets, the priority reduction, the
stale-revision rule, the unknown-field construction error, and
bounded redaction); the CLI surface grows a `GateSubcommand` enum
on `GateArgs` in `src/cli.rs` (the existing `driftwatch gate` flat
flags are preserved when no subcommand is given) and a new
`src/commands/evidence_export.rs` orchestrator (8 unit tests
covering the no-run refusal, human and JSON outputs, malformed-
`results_json` recovery, idempotent read-only behaviour, and
project-id resolution). A new `tests/gate_evidence_export.rs`
integration suite (11 tests, Unix-only) drives the binary through
the five design scenarios — ran/PASS, ran/FAIL, not-scheduled,
could-not-execute, and stale revision — plus the field-set, schema,
and read-only invariants. The bounded-evidence store, the release-
evidence / capability-conformance adapters, the release-gate
envelope wire version, the Gate `gate_runs` schema, the
`driftwatch check` path, and the `gate --format json` path are
byte-for-byte unchanged. Driftwatchdog remains an executor/
aggregator: no release publisher, signer, SBOM generator, or
deployment executor is added. The full test suite is green (533
lib + 196 integration); `cargo fmt --check`, `cargo clippy
--all-targets --all-features -- -D warnings`, `openspec validate
--changes --strict --no-interactive` (the remaining unselected
change is `post-mvp-readiness`), and `sh tests/packaging.sh` (8/8)
are green. The ROADMAP now lists v1.4 Governance evidence export,
the README documents the new subcommand plus the `EvidenceExport`
schema, and the next action is the governance-side consumption
(belongs to `workspace-governance`, not to this repository). One
planning change remains authored but unselected: `post-mvp-readiness`.

The `release-evidence-and-capability-gate` change is **implemented and
archived** (2026-09-24): `driftwatch gate` gains a third built-in
profile, `release`, that selects two new stable concern IDs —
`release-evidence` and `capability-conformance`. Both are routed in
`src/commands/gate.rs` `execute_plan` to dedicated envelope adapters
in `src/gate/adapters.rs` (`run_release_evidence_adapter`,
`run_capability_conformance_adapter`) that share the same wire shape
with the existing product-quality adapter
(`RELEASE_GATE_ENVELOPE_VERSION = 1`, exit-code authority rule
`PASS`→0, `FAIL`→1, `REVIEW_REQUIRED`→2, `NOT_APPLICABLE`→0). The
release-evidence adapter applies a required-evidence guard (a `PASS`
claim must include `revision`, `product_version`, at least one
`artifacts` entry, and `provenance`; missing fields downgrade to
`REVIEW_REQUIRED` naming the missing keys) and a stale-revision guard
(when the project exposes a current revision via `git` and the
envelope's `revision` disagrees, a `PASS` claim is downgraded to
`REVIEW_REQUIRED` with both revisions named in the diagnostic). The
capability-conformance adapter applies a required-evidence guard (a
`PASS` claim must include a non-empty `capabilities.verified` list
whose every id appears in the resolved plan) and an out-of-scope
guard (verified ids outside the plan downgrade to `REVIEW_REQUIRED`).
Both adapters also reject malformed/missing/wrong-version envelopes
as `REVIEW_REQUIRED` and deliberately do **not** provide a text-mode
fallback so missing coverage can never be silently treated as a pass.
`src/gate/concerns.rs` extends the concern-ID vocabulary
(`RELEASE_EVIDENCE`, `CAPABILITY_CONFORMANCE`, `RELEASE_GATE_CONCERNS`,
`is_release_gate_concern` / `is_release_evidence_concern` /
`is_capability_conformance_concern`); `src/gate/manifest.rs` adds the
`release` profile to `profile_defaults` and `SUPPORTED_PROFILES` and
records the two concerns in the `not_scheduled` explanation set for
unrelated plans. 6 new unit tests cover the vocabulary; 5 new
`src/gate/manifest.rs` tests cover the `release` profile, optional
relaxation, explicit `enabled = false`, and `not_scheduled`
behaviour; 18 new `src/gate/adapters.rs` tests cover the two adapters
(passing/failing/review envelopes, contradictory exits, stale
revision, missing required fields, out-of-scope verified, empty
verified, malformed/empty/wrong-version/unknown-status, secret
redaction, provenance string-or-object); 19 new
`tests/gate_release_evidence.rs` integration tests cover the
end-to-end behaviour (passing/failing, contradiction, missing
required, malformed, empty, missing-command, optional-without-
command, dry-run rendering, dry-run no-persist, `--format json`,
verified subset, out-of-scope, empty-verified, stale revision in a
git repo, wrong envelope version, NOT_APPLICABLE optional vs required
block, history persistence). `driftwatch check` and every other Gate
concern path are byte-for-byte unchanged. Driftwatchdog remains an
executor/aggregator: no release publisher, signer, SBOM generator, or
deployment executor is added. The full test suite is 679 green;
`cargo fmt --check`, `cargo clippy --all-targets --all-features
-- -D warnings`, `openspec validate --changes --strict
--no-interactive`, and `sh tests/packaging.sh` (8/8) are green. The
ROADMAP now lists v1.3 Release-evidence and capability-conformance
Gate, the planning queue is empty, and the README documents the
`release` profile plus the envelope wire shape and the two new
guards. No further change remains authored; per the standing
auto-mode authorization the next change requires explicit selection.

The `product-quality-gate-contract` change is **implemented and archived**
(2026-09-24): `driftwatch gate` gains two built-in profiles, `product` and
`rust-product` (alias), that select the stable concern IDs
`product-code-boundary` and `placeholder-threshold`. Concerns are routed in
`src/commands/gate.rs` `execute_plan` to a new
`run_product_quality_adapter` in `src/gate/adapters.rs` that parses a
versioned JSON envelope (`PRODUCT_QUALITY_ENVELOPE_VERSION = 1`,
`status` + bounded `findings`/`evidence`/`missing_evidence`/`diagnostic`/
`remediation`) and applies the exit-code authority rule
(`PASS`→0, `FAIL`→1, `REVIEW_REQUIRED`→2, `NOT_APPLICABLE`→0). Status/exit
mismatches, missing envelopes, unparsable envelopes, and unknown versions
all downgrade to `REVIEW_REQUIRED` so missing coverage is never silently
treated as a pass. When a command exits cleanly without emitting an
envelope, the existing text-mode mapping (exit 0 → `PASS`, nonzero →
`FAIL`) is preserved so legacy commands keep working. New
`src/gate/concerns.rs` owns the data-level concern vocabulary
(`PRODUCT_CODE_BOUNDARY`, `PLACEHOLDER_THRESHOLD`,
`PRODUCT_QUALITY_CONCERNS`, `is_product_quality_concern`); `src/gate/manifest.rs`
adds the two built-in profile names, extends `SUPPORTED_PROFILES`, and
records the new concerns in the `not_scheduled` explanation set for
unrelated plans. 17 new unit tests cover the envelope parser, the full
exit-code/status mapping matrix, contradiction detection, malformed
fallback, and infrastructure failure mapping; 15 new
`tests/gate_product_quality.rs` integration tests cover the passing /
failing / review / contradictory / malformed / missing-command /
optional-without-command / dry-run / `--format json` / `rust-product`
alias / persistence scenarios. `driftwatch check` and every other Gate
concern path are byte-for-byte unchanged. The full test suite is 631
green; `cargo fmt --check`, `cargo clippy --all-targets --all-features
-- -D warnings`, `openspec validate --changes --strict --no-interactive`,
and `sh tests/packaging.sh` (8/8 incl. `change_workflow`) are green. The
ROADMAP now lists v1.2 Product-quality Gate and the README documents
both profile names plus the envelope wire shape and exit-code authority
rule. One planning change remains authored but unselected:
`release-evidence-and-capability-gate`.

The `checker-machine-output` change is **implemented and archived**
(2026-09-24): `driftwatch check` gains `--format human|json`; the JSON
mode emits a single versioned `driftwatch-checker/0.1.0` document on
stdout (per-checker rows in declaration order with `name`, `status`
mapped to one of `ok` / `alerting` / `failed` / `timeout` /
`protocol-error`, the parsed `alerts[]` in the existing wire shape,
bounded `error` for the failure modes, and `summary` counts) so
control-plane consumers (notably Forge's policy adapter) can drive
checker-only projects without parsing human text. The dry-run banner
and correlation-skip warning move to stderr in JSON mode; human output
text, dry-run persistence, and exit-status semantics are byte-for-byte
identical across formats. New `src/checker/json.rs` (8 unit tests on
the renderer: empty outcome, mixed status mapping, success-with-zero-
alerts vs alerting, `start_failed`/`unknown` collapsing, alerts wire
shape + `extra` map, always-present empty `alerts:[]`, project root
included/omitted, error field reserved for failure modes) and 8 new
`tests/check.rs` integration tests (versioned document, mixed
declaration order, malformed isolation, dry-run no-write, no-checkers
still valid, exit-code parity, human text unchanged, `--help` shows
the flag). `cargo fmt --check`, `cargo test` (445 lib + 32 check
integration), `cargo clippy --all-targets --all-features -- -D
warnings`, and `openspec validate --changes --strict --no-interactive`
are green. The checker protocol, snapshot persistence, MCP surface, and
`driftwatch gate --format json` are untouched. Unblocks the Forge
`driftwatch-cli-alignment` consumption path. One additional change
remained authored but unselected: `product-quality-gate-contract`, which
proceeded next under the standing auto-mode authorization in
`AGENTS.md`.

The `ai-gate-manifest-consumption` change is **implemented and archived**
(2026-09-20): `driftwatch gate` now resolves a business project's
`.ai-gate/gate.yaml` as a third Gate manifest source (precedence
`gate.toml` → `.driftwatch/gate.toml` → `.ai-gate/gate.yaml`) and converts
the declared policy (`version`, optional `runtime`, `profile`,
`rule_pack`, `checks` map of `true|false|"optional"`, optional `commands`,
`blocking` status list, `project_commands`, `contexts`) into the native
`GateManifest`, so the existing resolve/execute/aggregate/persist pipeline
runs it unchanged. New `src/gate/aigate.rs` (strict `yaml-rust2` mapping,
actionable unknown-field hints, command-must-reference-a-selected-check,
empty-command and unknown-blocking rejection before execution);
`manifest::LoadOutcome` + `load_for_runtime` (a manifest naming another
runtime is reported, executed nothing, persisted nothing, exit 0); project
domain profiles via a new `[profiles.<name>]` `GateManifest` table (empty
maps are `skip_serializing_if` so existing `gate.toml` digests stay stable;
built-in shadowing rejected); the YAML profile string and `rule_pack` are
carried as plan identity. 13 unit + 3 integration tests (valid YAML, TOML
precedence, foreign runtime, unknown field, blocking list, command binding,
domain vs built-in profile, dry-run identity). README Gate section and
doctor/context remediation wording updated. `cargo fmt --check`, `cargo
test` (571 pass), `cargo clippy --all-targets --all-features -- -D
warnings`, `cargo deny check` (advisories/licenses/bans/sources ok), and
strict OpenSpec validation are green. No network access, no rule-pack
evaluator, and `driftwatch check` behavior are unchanged.

The `gate-cli-and-memory-integration` change is **implemented and
archived** (2026-09-15): new `driftwatch gate` command
(`--dry-run` plan rendering with no execution/persistence, human and
`--format json` machine-readable output, nonzero exit when blocked),
`src/repo/gates.rs` over migration 0006 (`gate_runs` identity rows:
change/revision, manifest digest, rule-pack version, aggregate
status, blocking flag, canonical results JSON; repeated gating stays
distinguishable), execution through the shared project-runtime
adapter (`sh -c` over declared commands with bounded capture/timeout/
process-group kill; command-less concerns record explicit
`NOT_APPLICABLE`; one failure never aborts the rest; optional
`ai-review` evaluation appended only when `[ai] enabled = true`),
transactional persistence, `gate.history` doctor check (silent
without `gate.toml`, `Info` when never run, `Pass`/`Warn` on the
latest run; therefore visible in read-only `doctor_status` MCP with
no tool-surface change), export schema v4 (`gate_runs` array,
`#[serde(default)]` so v3 reads stay valid; jsonl `gate_run`
records; markdown `Gate runs` section), `report --ai` Gate status
section distinguishing current gate failures from historical
recurrence, README Gate section, and a CI-template local-first
comment. 9 new tests cover pass/fail/review/dry-run/no-manifest
persistence, history checks, and distinguishable revisions.
`driftwatch check` behavior is untouched. No Gate planning packages
remain; the v1.1 Engineering Gate queue is complete.

The `gate-ai-evaluation` change is **implemented and archived**
(2026-09-15): new `src/gate/ai.rs` (provider-neutral typed
request/response contract: `AiRule` + evidence previews + context
previews into a bounded redacted `AiEvalRequest` with prompt/rule
digests; tolerant `AiProviderOutput` validation where `PASS`
requires valid schema, non-empty rules-checked, and every evidence
key resolving to an available artifact; malformed output or unknown
status is `REVIEW_REQUIRED`, never `PASS`; violations force `FAIL`;
opt-in `AiProviderConfig` with explicit missing-provider policy;
bounded external-command invocation reusing the checker runner's
capture/timeout/process-group kill; disabled AI yields
`NOT_APPLICABLE` with no provider call; spawn/timeout/signal
failures are distinct `REVIEW_REQUIRED` diagnostics; no LLM is
embedded, no API keys stored, raw artifacts never submitted) plus
`doctor` integration (`gate.ai.*` checks: absence silent, disabled
is `Info`, enabled-without-provider is `Warn`, missing executable
is `Warn`). 14 new tests cover valid, invalid, missing-evidence,
missing-provider (both policies), timeout, and secret cases. No
OpenSpec types enter generic gate modules. One Gate planning
package remained; the next in dependency order was
`gate-cli-and-memory-integration` and proceeded automatically
under the standing auto-mode authorization in `AGENTS.md`.

The `generic-context-providers` change is **implemented and archived**
(2026-09-15): new `src/gate/context.rs` (generic read-only
`ProviderRegistry` returning bounded, hashed `ContextDocument` values,
`git` provider for diff/status, `project-files` provider for
project-local documents, optional `openspec` provider emitting generic
documents only, path confinement to the project root, size/truncation
bounds, secret redaction, unavailable-not-empty semantics) plus `doctor`
integration (`gate.context.*` checks: absence stays silent, unknown
providers are `Warn`, a selected-but-absent `openspec/` dir is honest
`Info`). 20 new tests cover Git-only, OpenSpec, missing, escape,
truncation, and mutation cases. No OpenSpec types enter generic
gate/core modules. Two Gate planning packages remain; the next in
dependency order is `gate-ai-evaluation` and proceeds automatically
under the standing auto-mode authorization in `AGENTS.md`.

The `gate-adapter-evaluation` change is **implemented and archived**
(2026-09-15): new `src/gate/adapters.rs`
(`AdapterRegistry` with validated capability declarations,
`AdapterInput` executed through the existing checker runner for
bounded capture/timeout/process-group kill, tolerant
checker-JSON/SARIF/Gitleaks/OSV/Semgrep normalizers with secret
redaction and finding caps, exit-code/findings/evidence normalized
separately so scanner nonzero-with-findings still yields `FAIL`,
infra failures mapped to `REVIEW_REQUIRED` with `<tool>:output` /
`<tool>:timeout` missing evidence, `project-runtime` text adapter by
exit code, `run_all` isolation so one failure never aborts the rest,
pure deterministic `evaluate` over threshold rules plus
rule-required available-evidence guard). 28 new tests cover fixtures,
malformed/timeout/nonzero/signal/spawn isolation, caps, argv
passthrough, evaluator evidence rules, and no-SDK-coupling.
`driftwatch check` persistence and existing checker protocol are
untouched. Two Gate planning packages remain; the next in
dependency order is `gate-ai-evaluation` and proceeds
automatically under the standing auto-mode authorization in
`AGENTS.md`.

The `toolchain-management-and-execution` change is **implemented and
archived** (2026-09-15): new `src/gate/toolchain.rs`
(strict `gate-tools.toml` parsing with `did-you-mean` diagnostics, pinned
`id`/`version`/`digest`/`modes` entries, platform resolution against the
release matrix `linux-x86_64`/`linux-arm64`/`darwin-x86_64` with
fail-before-execution rejection, per-tool/per-version/per-platform user
cache with `sha256:` verification before atomic temp+rename,
per-executable `create_new` bootstrap locks with contention-as-error,
offline-first `resolve_execution` across `managed`/`container`/`native`/
`project_runtime` backends with digest-pinned container images,
project-root-confined mounts, explicit env allowlist, opt-in native
fallback, verbatim project argv with no SDK install, timeout carried
into the runner, explicit `bootstrap_plan`/`render_bootstrap_plan` with
no network I/O in core) plus `doctor` integration
(`gate.toolchain.*` checks: missing manifest stays silent, malformed is
`Warn`, missing tools are `Warn` with the exact bootstrap remediation,
unsupported platforms are `Fail`). 28 new tests cover parsing,
digests, atomic store, mismatch cleanup, lock contention/mutual
exclusion, offline reuse, container pinning/mount escapes/
unavailability, native opt-in, project-runtime passthrough/missing,
timeouts, bootstrap plans, and doctor output. Ordinary checks never
download tools. Two Gate planning packages remain; the next in
dependency order is `gate-ai-evaluation` and proceeds automatically
under the standing auto-mode authorization in `AGENTS.md`.

The `evidence-and-artifacts` change is **implemented and archived**
(2026-09-14): new `src/gate/evidence.rs` (typed artifact classes,
configured-secret + pattern redaction with bounded UTF-8-safe previews,
adapter-path escape rejection, `ArtifactRecord::unavailable` markers, and
the `evidence_backed_pass` guard so missing/unavailable evidence never
claims evidence-backed `PASS`), new `src/repo/evidence.rs`
(`gate_artifacts` identity rows, idempotent insert-or-get on
`UNIQUE(key)`, retention prune that deletes files but keeps rows),
migration 0005 (`gate_artifacts` + index; local schema version now 5),
export schema v3 (`gate_artifacts` metadata in json/jsonl/markdown, v2
documents stay readable via `#[serde(default)]`), and `driftwatch gc`
pruning artifact files with metadata retained. Report and MCP read paths
are verified unchanged. 30+ new tests cover secrets, escapes, caps,
cleanup, and unavailable artifacts.
`driftwatch.toml` checker execution is untouched. Five Gate planning
packages remain; the next in dependency order is
`toolchain-management-and-execution` and proceeds automatically under
the standing auto-mode authorization in `AGENTS.md`.

The `gate-project-configuration` change is **implemented and archived**
(2026-09-14): new `src/gate/manifest.rs` (strict `gate.toml` parsing with
`did-you-mean` unknown-field diagnostics, `backend`/`frontend`/`full`/
`minimal` profile resolution, explicit-check enable/disable overrides,
project commands kept separate from tool commands, generic context
declarations, substring changed-surface triggers, `sha256:` manifest
digest + rule-pack identity, deterministic `GatePlan` resolution, pure
`render_plan` dry-run output) plus 15 tests covering valid, unknown-field,
unknown-profile, version-mismatch, empty-command, backend-excludes-
responsive, UI-trigger, determinism, and missing-manifest cases.
`driftwatch.toml` checker execution is untouched. Six Gate planning
packages remain; the next in dependency order is
`evidence-and-artifacts` and requires explicit authorization before
implementation.

The `generic-gate-contract` change is **implemented and archived**
(2026-09-14): new `src/gate/` module (domain types, versioned JSON DTOs
with `GATE_CONTRACT_VERSION = 1`, deterministic blocking-policy
aggregation, bounded evidence references with secret redaction, and the
checker-outcome compatibility adapter) plus 24 unit/contract tests
covering PASS/FAIL/REVIEW_REQUIRED/NOT_APPLICABLE, blocking and
non-blocking review, required-inapplicable, checker mapping, and
malformed/oversized/secret/unknown-version input. Existing checker
protocol, snapshots, exports, reports, and MCP paths are untouched.
Seven Gate planning packages remain; the next in dependency order is
`gate-project-configuration` and requires explicit authorization before
implementation.

The `bfs-dfs-bfs-change-workflow` change is **implemented and archived**
(2026-09-14): three-phase workflow wording in `AGENTS.md` (BFS impact map,
DFS implementation, final BFS regression; proposal/design/spec/task
responsibilities; local-before-CI verification; no-archive-before-final-BFS
gate) plus the `change_workflow` packaging test
(`tests/packaging/test_change_workflow.sh`) that enforces phase order,
per-phase checkboxes, final-BFS verification, spec Scenarios, and a negative
missing-DFS fixture across every active change. Eight Gate planning packages
remain; the next in dependency order is `generic-gate-contract` and requires
explicit authorization before implementation.

All v0.x change packages through v0.5 are **implemented and archived**: `project-foundation`, `runtime-memory`, `fingerprinting-and-retention`, `export-and-doctor`, `checker-and-drift-alerts`, `correlation-and-ai-context`, and `linux-macos-distribution`. The `crash-hardening` change is also **implemented and archived** (2026-09-14): char-boundary truncation helper, UTF-8-once capture, signal-aware statuses (`RunStatus::Signalled`/`Timeout`, checker `Status::Unknown`), opt-in `run --timeout-ms` with process-group kill, and capture diagnostics. The `quality-cicd-docs-ux` change is also **implemented and archived** (2026-09-14): `[lints.clippy] all = "deny"` + `rust-version = "1.74"` MSRV (with `is_none_or`/`repeat_n` lowered to 1.74-compatible APIs), `deny.toml` advisory/license policy, proptest seeds for the normalizer, CI breadth (`--all-features` clippy, MSRV check, cargo-deny, tarpaulin coverage, `macos-14` matrix, shellcheck + script-mode enforcement, `npm audit`, smoke), release integrity (npm provenance via `id-token: write`, `macos-14` runner, tag==Cargo version assertion, SPDX SBOM, CHANGELOG.md, cold-start `bump.sh` at 0.1.0, PAT-missing skip + no-tag-spam auto-tag, Dependabot), and docs/UX (binary-naming rule, `driftwatch completions`/`man`, per-command examples, error `hint:` lines, honest doctor INFO, mandatory-verify installer with early `--dry-run` + `cargo install` fallback, npm darwin-arm64 preinstall guard, `driftwatch.toml.example`, repo-hygiene packaging test). The `mcp-read-tools` change is also **implemented and archived** (2026-09-14): stdio JSON-RPC 2.0 MCP server with four read-only tools (`top_bugs`/`show_bug`/`ai_report`/`doctor_status`), closed input schemas, `SQLITE_OPEN_READ_ONLY` DB open, and `driftwatch mcp` subcommand. The `agent-examples` change is also **implemented and archived** (2026-09-14): tested copy-paste MCP + workflow examples for Claude Code, OpenCode, and Aider, the `report --ai > drift.md` fallback for Aider, `examples/README.md` with the harness→destination map, and a packaging consistency test that asserts every `mcp.json` parses, every `driftwatch <sub>` token in the workflows exists in `--help`, and no `run`/`check` is ever presented as an agent step. The `github-actions-templates` change is also **implemented and archived** (2026-09-14): reusable GitHub Actions workflow at `templates/github-actions/driftwatch-check.yml` (workflow_call + workflow_dispatch) that pins a `driftwatchdog` install, always renders `report --ai > drift.md`, uploads only `drift.md` (never `state.db` or command logs), summaries `driftwatch top` into `$GITHUB_STEP_SUMMARY`, and ships with `contents: read` plus `fail_on_drift`/`upload_report` inputs. The `driftwatchdog` Rust binary builds and tests cleanly (396 tests pass). The full CLI surface is functional: `init`, `run`, `list`, `top`, `show`, `report` (with `--ai`), `gc`, `export json|jsonl|markdown`, `doctor`, `check`, `link`, `unlink`, `completions`, `man`, `mcp`. The MCP server speaks JSON-RPC 2.0 on stdio with four read-only tools (`top_bugs`, `show_bug`, `ai_report`, `doctor_status`) and opens the database read-only. The SQLite schema (version 3) covers runs, fingerprints, occurrences, check_snapshots (with `git_commit`/`git_branch`), drift_alerts, correlations (with per-component scores and `algorithm_version`), and manual_links. Nine capability specs are live under `openspec/specs/` (including `quality-cicd-docs-ux`, `agent-examples`, and `github-actions-templates`; `ci-test-gates` and `release-distribution` Purposes filled). Native release archives plus an SHA-256 manifest are produced for Linux x86_64, Linux arm64, and macOS x86_64 by `.github/workflows/release.yml`; the shell installer (`scripts/install.sh`), the npm launcher (`npm/driftwatchdog/`), direct downloads, and `cargo install` are documented in README.md. v0.6 (Integrate) is complete. v1.0 (Stable) is the next planning milestone; the first v1.0 hardening change is not yet proposed in `openspec/changes/`.

## Start here

Read these in order:

1. README.md — product positioning, user-facing command surface, and installation channels.
2. ROADMAP.md — release sequence, closed change inventory, and dependency graph.
3. `openspec/specs/` — the capability specifications the implementation satisfies.
4. `openspec/changes/` held the v1.1 planning queue (now fully archived; see `openspec/changes/archive/`). Future changes follow the dependency
   order in `ROADMAP.md`; implementation requires explicit authorization and one
   change at a time.

## Change inventory

| Change | Status | Purpose | Depends on |
| --- | --- | --- | --- |
| project-foundation | archived 2026-09-04 | Rust CLI, config, local directory, SQLite schema, Git metadata | none |
| runtime-memory | archived 2026-09-04 | Run arbitrary commands, persist/query runs, top-level empty state | foundation |
| fingerprinting-and-retention | archived 2026-09-04 | Normalize failures, aggregate bugs, report, GC | runtime memory |
| export-and-doctor | archived 2026-09-04 | Portable export (json/jsonl/markdown) and local doctor diagnostics | foundation |
| checker-and-drift-alerts | archived 2026-09-04 | External checker protocol, adapters, snapshots, alerts, `driftwatch check` | foundation |
| correlation-and-ai-context | archived 2026-09-04 | Heuristic correlations, manual `link`/`unlink`, `driftwatch report --ai` | fingerprinting; checker alerts |
| linux-macos-distribution | archived 2026-09-04 | Shell installer, npm launcher, release workflow, SHA-256-verified native archives for Linux x86_64, Linux arm64, and macOS x86_64 | any prior archive |
| crash-hardening | archived 2026-09-14 | No-panic truncation, UTF-8-safe capture, signal-aware status, bounded run/checker execution | runtime-memory; checker-and-drift-alerts |
| mcp-read-tools | archived 2026-09-14 | stdio MCP server with read-only bug tools (`top_bugs`, `show_bug`, `ai_report`, `doctor_status`) | stable CLI surface |
| agent-examples | archived 2026-09-14 | Tested copy-paste MCP + workflow examples for Claude Code, OpenCode, and Aider; Aider includes the `report --ai > drift.md` fallback; consistency test in `tests/packaging/test_agent_examples.sh` | mcp-read-tools |
| github-actions-templates | archived 2026-09-14 | Reusable `templates/github-actions/driftwatch-check.yml` (`workflow_call` + `workflow_dispatch`) that pins a `driftwatchdog` install, always renders the AI report, uploads only `drift.md`, summarises `top` into the step summary, with `contents: read` and the `fail_on_drift`/`upload_report` inputs; shape test in `tests/packaging/test_gha_templates.sh` | mcp-read-tools (stable CLI only) |
| generic-gate-contract | archived 2026-09-14 | Generic gate domain types, versioned JSON DTOs, deterministic blocking aggregation, bounded evidence refs with secret redaction, checker-outcome adapter; `src/gate/` + 24 tests | bfs-dfs-bfs-change-workflow |
| gate-project-configuration | archived 2026-09-14 | Project Gate manifest (`gate.toml`): profiles, explicit checks, project commands, triggers, rule-pack identity, dry-run plan; `src/gate/manifest.rs` + 15 tests; `driftwatch.toml` execution untouched | generic-gate-contract |
| evidence-and-artifacts | archived 2026-09-14 | Bounded Gate evidence (`src/gate/evidence.rs` + `src/repo/evidence.rs`, migration 0005, export v3, gc prune); 30+ tests; `driftwatch.toml` execution untouched | gate-project-configuration |
| toolchain-management-and-execution | archived 2026-09-15 | Pinned tool manifests, verified user cache, atomic locks, managed/container/native/project-runtime backends (offline default, explicit bootstrap), `gate.toolchain.*` doctor checks; `src/gate/toolchain.rs` + 28 tests; `driftwatch.toml`/`gate.toml` execution untouched | evidence-and-artifacts |
| gate-adapter-evaluation | archived 2026-09-15 | Adapter registry, checker-JSON/SARIF/Gitleaks/OSV/Semgrep normalizers, project-runtime text adapter, isolated execution, deterministic evaluator; `src/gate/adapters.rs` + 28 tests; checker protocol/persistence untouched | toolchain-management-and-execution |
| generic-context-providers | archived 2026-09-15 | Generic read-only provider registry, bounded hashed documents, Git + project-file + optional OpenSpec providers, `gate.context.*` doctor checks; `src/gate/context.rs` + 20 tests; no OpenSpec types in gate core | gate-adapter-evaluation |
| gate-ai-evaluation | archived 2026-09-15 | Provider-neutral typed AI evaluation (opt-in redacted bounded fail-closed contract, external-command boundary, no embedded LLM); `src/gate/ai.rs` + 14 tests; `gate.ai.*` doctor checks | generic-context-providers |
| gate-cli-and-memory-integration | archived 2026-09-15 | Local `driftwatch gate` CLI (dry-run, human/json, nonzero-when-blocked), `gate_runs` history (migration 0006), `gate.history` doctor, export v4, AI-report Gate section; `src/commands/gate.rs` + `src/repo/gates.rs` + 9 tests; `check` untouched | gate-ai-evaluation |
| checker-machine-output | archived 2026-09-24 | `driftwatch check --format human|json` with a versioned `driftwatch-checker/0.1.0` document on stdout (declaration order, `ok`/`alerting`/`failed`/`timeout`/`protocol-error` statuses, parsed `alerts[]` in the existing wire shape, bounded `error` note, summary counts); dry-run banner and correlation warning on stderr; human output, persistence, and exit-status semantics unchanged. New `src/checker/json.rs` (8 unit tests) + 8 new `tests/check.rs` integration tests. `gate --format json` and the existing checker protocol are untouched. | stable CLI surface |
| product-quality-gate-contract | archived 2026-09-24 | `product` and `rust-product` built-in profiles + stable concern IDs `product-code-boundary` and `placeholder-threshold`; versioned JSON-envelope result normalization (`PRODUCT_QUALITY_ENVELOPE_VERSION = 1`, exit-code authority rule PASS→0 / FAIL→1 / REVIEW_REQUIRED→2 / NOT_APPLICABLE→0, mismatches and malformed envelopes downgrade to REVIEW_REQUIRED); text-mode fallback for legacy commands; end-to-end coverage including persistence, `--format json`, dry-run, and optional-concern relaxation. New `src/gate/concerns.rs` (4 unit tests) + 17 new `src/gate/adapters.rs` unit tests + 15 new `tests/gate_product_quality.rs` integration tests + 5 new `src/gate/manifest.rs` unit tests. `driftwatch check` and every other Gate concern path are byte-for-byte unchanged. | gate-cli-and-memory-integration |
| release-evidence-and-capability-gate | archived 2026-09-24 | `release` built-in profile + stable concern IDs `release-evidence` and `capability-conformance`; shared wire shape with the product-quality envelope (`RELEASE_GATE_ENVELOPE_VERSION = 1`, exit-code authority rule, bounded fields, secret redaction); release-evidence required-evidence guard (PASS requires `revision`, `product_version`, at least one `artifacts` entry, and `provenance`; stale revision vs current git rev downgrades to REVIEW_REQUIRED); capability-conformance required-evidence guard (PASS requires non-empty `verified` whose ids are all in the resolved plan; out-of-scope verified ids downgrade to REVIEW_REQUIRED); malformed/missing/wrong-version envelopes downgrade to REVIEW_REQUIRED; no text-mode fallback so missing coverage can never be silently treated as a pass. New `src/gate/concerns.rs` (6 unit tests) + 5 new `src/gate/manifest.rs` unit tests + 18 new `src/gate/adapters.rs` unit tests + 19 new `tests/gate_release_evidence.rs` integration tests. `driftwatch check` and every other Gate concern path are byte-for-byte unchanged; Driftwatchdog remains an executor/aggregator (no release publisher, signer, SBOM generator, or deployment executor is added). | product-quality-gate-contract |
| gate-evidence-export | archived 2026-09-27 | Read-only `driftwatch gate evidence-export` subcommand (`--format human|json`, `--dry-run` for documentation) that maps a completed `gate_runs` row into the workspace-governance `release_evidence` vocabulary (closed field set `revision` / `version` / `toolchain` / `artifacts` / `digests` / `sbom` / `provenance` / `checks` / `publication`, closed state set `verified` / `unverified` / `blocked`); per-field state priority `verified > blocked > unverified` with `verified` reserved for scheduled checks that ran and passed on the current revision; stale-revision rule emits every field `unverified` with a `stale` diagnostic naming both revisions; unknown governance field is an `ExportError::UnknownField` construction error; refuses with a non-zero exit when no run exists. New `src/gate/evidence_export.rs` (23 unit tests) + new `src/commands/evidence_export.rs` (8 unit tests) + new `tests/gate_evidence_export.rs` (11 integration tests covering ran/PASS, ran/FAIL, not-scheduled, could-not-execute, stale revision, schema, and read-only invariants). The bounded-evidence store, the release-evidence / capability-conformance adapters, the release-gate envelope wire version, the `gate_runs` schema, the `driftwatch check` path, and the `gate --format json` path are byte-for-byte unchanged; Driftwatchdog remains an executor/aggregator. | release-evidence-and-capability-gate |

## Implementation constraints

- Rust is the implementation language; monitored projects are language-agnostic.
- The default normalizer must accept arbitrary text. Rust/.NET/Python/Node-specific profiles are optional enhancements only.
- Use a single local binary and SQLite. Do not introduce services, a plugin runtime, an embedded vector database, or an LLM dependency.
- Default behavior is offline and non-telemetric.
- Preserve child command arguments and exit codes.
- Checker failures must be isolated from other checkers.
- Correlation wording must say "possible relationship", "possible match", or "heuristic correlation"; never present a score as root-cause proof.
- Keep full logs disposable while preserving long-term bug identity and statistics.

## Verification gates (current)

Last run on this change:

    cargo fmt --check
    cargo test             # lib + integration (incl. 11 new gate_evidence_export integration, 8 new evidence_export command unit, 23 new evidence_export core unit)
    cargo clippy --all-targets --all-features -- -D warnings
    openspec validate --changes --strict --no-interactive   # one unselected change remains: post-mvp-readiness
    sh tests/packaging.sh   # 8/8 pass: target_mapping, artifact_naming, checksum_manifest, installer, repo_hygiene, agent_examples, gha_templates, change_workflow
    ./target/debug/driftwatch run sh -c 'echo boom >&2; exit 1'   # bug attached
    ./target/debug/driftwatch show <hash8>                  # render fingerprint
    ./target/debug/driftwatch report                        # markdown report
    ./target/debug/driftwatch report --ai                   # AI-context report
    ./target/debug/driftwatch gc                            # pruned 0 runs (recent)
    ./target/debug/driftwatch export json                   # valid JSON document
    ./target/debug/driftwatch export jsonl                  # one record per line
    ./target/debug/driftwatch export markdown               # human-readable
    ./target/debug/driftwatch doctor                        # checks pass
    ./target/debug/driftwatch check                         # runs configured checkers
    ./target/debug/driftwatch link bug:<hash8> spec:<id>    # persists manual link
    ./target/debug/driftwatch unlink <id>                   # removes targeted link
    ./target/debug/driftwatch gate                          # release profile + complete envelopes -> PASS
    ./target/debug/driftwatch gate --dry-run                # release profile: capability-conformance + release-evidence required
    ./target/debug/driftwatch gate --format json            # release-gate rows in machine form
    ./target/debug/driftwatch gate evidence-export         # 9 governance fields, every field unverified for a non-release run
    ./target/debug/driftwatch gate evidence-export --format json   # versioned EvidenceExport document
    sh scripts/smoke.sh                                     # release packaging end-to-end

## Module map

- `src/main.rs` — binary entrypoint, `anyhow` boundary, returns `ExitCode`; dispatches all 13 top-level subcommands (the new `driftwatch gate evidence-export` flows through the existing `gate` dispatcher as a `GateSubcommand`).
- `src/cli.rs` — `clap` derive types (`Cli`, `Command::{Init,Run,List,Top,Show,Report,Gc,Export,Doctor,Check,Gate,Link,Unlink,Completions,Man,Mcp}`) and arg structs; `RunArgs` carries opt-in `--timeout-ms`; `GateArgs` carries `--dry-run` and `--format human|json` plus a new `GateSubcommand::EvidenceExport(GateEvidenceExportArgs)` (with its own `--format` / `--dry-run`) so the existing flat `driftwatch gate` flags are preserved when no subcommand is given; `CheckArgs` carries `--format human|json`.
- `src/error.rs` — `thiserror` `Error` enum used by library code; includes `LinkTarget` and `ManualLinkNotFound` variants.
- `src/fingerprint/mod.rs` — module entry, re-exports `Rules`, `Canonical`, `fingerprint`.
- `src/fingerprint/normalizer.rs` — generic normalizer (13 ordered rules).
- `src/fingerprint/hash.rs` — `fingerprint(&str) -> String` (SHA-256).
- `src/project/root.rs` — `ProjectRoot::discover` with bounded walk-up.
- `src/project/config.rs` — `Config` + `CheckerEntry` with optional `working_dir`, `env`, `timeout_ms`, `max_output_bytes`.
- `src/project/git.rs` — `GitContext`, `capture(cwd)`, non-fatal failures.
- `src/project/init.rs` — `init`/`init_at` orchestration, idempotent.
- `src/storage/mod.rs` — `open(path)` with PRAGMAs (WAL, NORMAL, foreign_keys=ON).
- `src/storage/migrations.rs` — versioned migration runner (applies 1 + 2 + 3 + 4 + 5 + 6).
- `src/storage/schema.rs` — `MIGRATION_0001_BASELINE`, `MIGRATION_0002_SNAPSHOT_GIT`, `MIGRATION_0003_CORRELATION_DETAIL`, `MIGRATION_0006_GATE_RUNS` (`gate_runs` history).
- `src/repo/mod.rs` — `Db` wrapper, `open`, `open_in_memory`, `conn`, `conn_mut`.
- `src/repo/gates.rs` — `GateRunRow`/`NewGateRun`, `Gates::{insert,count,latest,list}` over `gate_runs` (newest-first, missing-table reads as empty for old/read-only DBs).
- `src/repo/evidence.rs` — `Artifacts::{count,insert_or_get,get_by_key,list_all,list_all_with_ids,mark_unavailable,prune_before}` over `gate_artifacts`; prune deletes files under the state dir and flips rows to unavailable, never deleting identity.
- `src/repo/runs.rs` — `RunRecord`, `RunStatus` (`Running`/`Success`/`Failed`/`StartFailed`/`Signalled`/`Timeout`), `RunCompletion`, `ListFilter`; `Runs::{reserve,find,insert_full,list,all}`.
- `src/repo/bugs.rs` — `Fingerprint`, `TopRow`, `Occurrence`, `RecentCommit`, `Report`; `Bugs::{...}` plus `current_fingerprints` and `tags_for`.
- `src/repo/alerts.rs` — `Snapshot`, `Alert`, `NewSnapshot`, `NewAlert`; `Alerts::{snapshot_count,alert_count,list_snapshots,list_alerts,latest_snapshot_for,insert_snapshot,insert_alerts,record_run}` plus `current_alerts`.
- `src/repo/correlations.rs` — `Correlation` with per-component score fields and `algorithm_version`; `Correlations::{list_all,count,upsert}` plus `replace_for_fingerprint` (static, takes `&mut Db`).
- `src/repo/links.rs` — `ManualLink`; `Links::{list_all,count,find_by_id,create,delete,list_for_fingerprint,list_for_alert}`.
- `src/similarity/{mod,tokenize,score,candidates}.rs` — heuristic engine: tokenization, Jaccard, weighted `score_pair`, N×M candidate generation with 5,000-pair cap.
- `src/correlate.rs` — `run_after_check` orchestration: loads fingerprints + alerts, runs the candidate generator, persists passing pairs.
- `src/export/{dto,build,json,jsonl,markdown,mod}.rs` — versioned export DTOs (`SCHEMA_VERSION = 4`, incl. `GateArtifactExport` + `gate_artifacts` array and `GateRunExport` + `gate_runs` array, both with `#[serde(default)]` for older reads) and three serializers (jsonl `gate_artifact`/`gate_run` records, markdown `## Gate evidence` and `## Gate runs` sections).
- `src/doctor/{check,mod}.rs` — `Check`, `Status`, and the `Report` aggregator. Includes a `checker.last_run` warn when the most recent check snapshot for a configured checker was a failure, plus `gate.toolchain.*`, `gate.context.*`, `gate.ai.*`, and `gate.history` readiness/history checks (absent manifests stay silent; unknown providers are `Warn`; AI stays optional).
- `src/checker/mod.rs` — public module: `protocol`, `runner`, `report`, `json` re-exports.
- `src/checker/protocol.rs` — `DriftAlert`, `AlertsDocument`, `ProtocolError`, `parse_alerts_document`.
- `src/checker/runner.rs` — `CheckerSpec`, `run_checker`, `CheckerRun` (incl. `signalled` + `capture_error`) with bounded capture and per-checker timeout plus group kill.
- `src/checker/report.rs` — `Status`, `Severity`, `CheckerOutcome`, `label_for_status`.
- `src/checker/json.rs` — `CheckerReportDocument` for `driftwatch check --format json` (contract id `driftwatch-checker/0.1.0`, declaration order, `ok`/`alerting`/`failed`/`timeout`/`protocol-error` per row, parsed `alerts[]` in the existing wire shape, bounded `error` for failure modes, summary counts, optional `project` field). Pure projection of the per-checker `CheckerOutcome` values; no execution, isolation, persistence, or exit-status impact.
- `src/gate/concerns.rs` — stable concern-ID vocabulary owned by the Gate contract (current set: `product-code-boundary`, `placeholder-threshold`, `release-evidence`, `capability-conformance` plus the `PRODUCT_QUALITY_CONCERNS` and `RELEASE_GATE_CONCERNS` sorted slices and the `is_product_quality_concern` / `is_release_gate_concern` / `is_release_evidence_concern` / `is_capability_conformance_concern` helpers); pure data with no provider SDK or language scanner; the routing decision in `src/commands/gate.rs` uses it to dispatch product-quality concerns to `run_product_quality_adapter` and the two release-gate concerns to `run_release_evidence_adapter` / `run_capability_conformance_adapter` while every other concern keeps the existing project-runtime text adapter.
- `src/gate/{mod,types,dto,aggregate,redact,adapt}.rs` — generic gate contract (`GateStatus`/`GateSeverity`/`Finding`/`EvidenceRef`/`GateResult`, `GATE_CONTRACT_VERSION = 1` JSON boundary with size caps, deterministic `aggregate` with `BlockingPolicy`, secret-redacting bounded diagnostics, `adapt_checker_outcome` mapping Empty→PASS / Success→FAIL / infra-failure→REVIEW_REQUIRED); no OpenSpec dependency, no storage migration, no CLI surface yet.
- `src/gate/manifest.rs` — project Gate manifest (`GateManifest`/`ManifestCheck`/`Trigger`/`ResolvedGatePlan`/`LoadOutcome`, `parse`/`resolve`/`render_plan`/`load`/`load_for_runtime`/`manifest_path`); precedence `gate.toml` → `.driftwatch/gate.toml` → `.ai-gate/gate.yaml`, missing manifest is `Ok(None)`; project domain profiles via `profiles` table (`[profiles.<name>]`, `skip_serializing_if` empty so digests stay stable, built-in shadowing rejected) with `profile_defaults_for`/`is_builtin_profile`; built-in profiles `backend`/`frontend`/`full`/`minimal` plus the two product-quality profiles `product` and `rust-product` (alias) and the new `release` profile registered in `SUPPORTED_PROFILES` and `profile_defaults`; the product-quality and release-gate concern ids are also recorded in the `not_scheduled` explanation set for plans that select a different profile; `driftwatch.toml` execution untouched; no tool install, no network, no OpenSpec types.
- `src/gate/aigate.rs` — business `.ai-gate/gate.yaml` → `GateManifest` conversion (`RUNTIME_NAME = "driftwatchdog"`, `AiGateDoc { runtime, manifest }`, strict `yaml-rust2` mapping with `did-you-mean` unknown-field hints, `checks` map of `true|false|"optional"`, `commands` must reference a selected check, empty-command/unknown-blocking rejection before execution, `blocking` list → review-required policy, `rule_pack` → identity, non-built-in profile selects exactly its declared checks); reuses the shared pipeline, no second Gate, no network.
- `src/gate/evidence.rs` — bounded evidence domain (`ArtifactKind`/`ArtifactRecord`/`NewArtifact`/`EvidenceError`, `MAX_ARTIFACT_BYTES = 1 MiB`, `MAX_PREVIEW_BYTES = 1024`, `build_record`/`store_bytes` via temp+rename, `confine_adapter_path`/`confined_path` escape rejection, `redact_secrets_with_extra` previews, `evidence_backed_pass` guard, `UNAVAILABLE_PREVIEW` marker); no OpenSpec types, no tool install, no network.
- `src/gate/evidence_export.rs` — workspace-governance evidence export (closed `GOVERNANCE_FIELDS` set `revision` / `version` / `toolchain` / `artifacts` / `digests` / `sbom` / `provenance` / `checks` / `publication`, closed `GOVERNANCE_STATES` set `verified` / `unverified` / `blocked`, `EVIDENCE_EXPORT_SCHEMA_VERSION = 1`, `Field` constructor refusing unknown names, `EvidenceState` parse/serialise, `FieldExport` / `EvidenceExport` / `StaleDiagnostic` structs, `fields_for_concern` mapping `release-evidence` → the seven release-evidence fields and `capability-conformance` → `checks`, `build` reducer with `verified > blocked > unverified` priority, stale-revision rule with a `stale` diagnostic naming both revisions, `ExportError::{UnknownField, NoRun}`, `render_human` and `render_json` with secret-redaction re-use); pure read, no execution, no mutation, no second evidence store, no OpenSpec types.
- `src/gate/adapters.rs` — adapter contracts (`AdapterRegistry`/`AdapterCapability`/`OutputFormat`/`AdapterInput`, duplicate/empty validation before execution), five built-ins (`checker`, `gitleaks`, `osv`, `semgrep`, `project-runtime`) via CLI boundaries only, tolerant normalizers (`parse_checker_json`/`parse_sarif`/`parse_gitleaks` without secret values/`parse_osv`/`parse_semgrep`) with finding/evidence caps and redaction, exit-findings-evidence separated (nonzero-with-findings → `FAIL`), infra mapping (`SpawnFailed`/`NonZeroExit`/`Timeout`+timeout evidence/`Signalled`/`MalformedOutput` → `REVIEW_REQUIRED` + `<tool>:output`), `run_text_adapter` exit-code mapping, `run_all` failure isolation, pure deterministic `evaluate` (threshold rules + rule-required available-evidence guard); also owns the **product-quality envelope adapter** (`PRODUCT_QUALITY_ENVELOPE_VERSION = 1` parser, version + status check, bounded findings/evidence/missing_evidence/diagnostic/remediation and secret redaction, `expected_exit_for_status` / `default_severity_for_status` mapping, `contradiction_result` and `envelope_result` helpers, `run_product_quality_adapter` entry point used by `src/commands/gate.rs` `execute_plan` for the two product-quality concern ids; spawn/timeout/signal map to `infra_result`; the text-mode fallback preserves the existing `project-runtime` semantics for legacy commands) and the **release-gate envelope adapters** (`RELEASE_GATE_ENVELOPE_VERSION = 1` shared wire version, `parse_release_envelope` / `parse_capability_envelope` parsers with `capabilities` sub-object handling for the capability envelope, `envelope_review_result` for malformed/missing cases, `envelope_to_result` helper with secret redaction, `run_release_evidence_adapter` with the required-evidence guard for `revision` / `product_version` / `artifacts` / `provenance` plus the stale-revision guard using a caller-supplied current revision from `git::capture`, `run_capability_conformance_adapter` with the empty-`verified`-list and out-of-scope-verified guards using the resolved plan ids; both release-gate adapters deliberately do not provide a text-mode fallback so missing coverage is never silently treated as a pass); no SDKs, no scanner reimplementation, no network, no LLM.
- `src/gate/context.rs` — generic read-only context providers (`ProviderRegistry`, bounded hashed `ContextDocument` with kind/path/digest/size/change-id, `git` diff/status + `project-files` + optional `openspec` providers emitting generic documents only, project-root confinement, truncation bounds, secret-safe previews, unavailable-not-empty semantics, `collect_context`/`selection_from_manifest`/`context_checks`); no OpenSpec types in gate core, no mutation, no network.
- `src/gate/ai.rs` — provider-neutral AI evaluation (`AiProviderConfig` opt-in with explicit missing-provider policy, `AiRule`, bounded redacted `AiEvalRequest` with prompt/rule digests, tolerant `AiProviderOutput` with fail-closed `validate_output`, `run_ai_evaluation` via the bounded checker runner, `AiEvaluationRecord` persistence wrapper, `load_ai_config`/`ai_checks`); no embedded LLM, no API keys, no network, no OpenSpec types.
- `src/gate/toolchain.rs` — pinned tool manifests (`ToolchainManifest`/`ToolEntry`/`ExecutionMode`, strict `gate-tools.toml` parse with `did-you-mean` hints, `version = 1` contract check), platform matrix (`SUPPORTED_PLATFORMS`, `current_platform`, `resolve_platform` fail-before-execution), verified user cache (`cache_base_from`/`user_cache_base`, `cache_dir`/`executable_path`, `digest_bytes`/`verify_bytes`/`cached_verified`, `store_verified_bytes` verify-then-temp+rename with staged re-verify, `MAX_TOOL_BYTES = 128 MiB`), per-executable `CacheLock` (`acquire_lock` via `create_new`, contention-as-error, drop-removes), policy-gated `resolve_execution` (`ProvisionPolicy::offline`/`bootstrap`, managed cache reuse, digest-pinned container images, project-root-confined mounts, explicit env allowlist, opt-in native fallback, verbatim project argv with no SDK install, `timeout_ms` passthrough), explicit `bootstrap_plan`/`render_bootstrap_plan`, optional manifest discovery (`gate-tools.toml` at root then `.driftwatch/`, missing is silent), truthful `toolchain_checks` (`gate.toolchain.*`: cache re-read, `PATH`/container probes, never config-alone); no network, no implicit install, no OpenSpec types.
- `src/runtime/runner.rs` — `CommandSpec` (incl. opt-in `timeout_ms`), `CapturedStream`, `RunOutcome` (incl. `timed_out` + `diagnostic`), `run`; byte-accumulating UTF-8-once drain, signal-aware status, process-group kill on timeout.
- `src/util.rs` — `truncate_char_boundary` shared helper (byte limit, char-boundary cut, ellipsis).
- `src/checker/runner.rs` — `CheckerSpec`, `run_checker`, `CheckerRun` (incl. `signalled` + `capture_error`) with bounded capture and per-checker timeout plus group kill.
- `src/commands/{run,list,top,show,report,gc,export,doctor,check,gate,link,unlink,report_ai,evidence_export}.rs` — per-subcommand orchestration returning process exit code. `top.rs` and `show.rs` expose `render` functions that return the same bytes the CLI prints; `report_ai.rs` exposes `render_ai_for_mcp` for the read-only MCP path and renders a `Gate status` section from the latest `gate_runs` row (tolerant of old DBs). `gate.rs` exposes `gate` plus `gate_history_checks` for `doctor`; its `execute_plan` captures `git::capture` once per run for the release-evidence staleness check and dispatches product-quality / release-evidence / capability-conformance concerns to the dedicated envelope adapters while every other concern keeps the project-runtime text adapter; its top-level `gate` function also routes the new `GateSubcommand::EvidenceExport` to `evidence_export::evidence_export`. `evidence_export.rs` is the read-only `driftwatch gate evidence-export` orchestrator: it opens the DB read-only, reads the latest `gate_runs` row, parses the persisted `results_json` (bare array; legacy versioned document accepted as a fallback), captures the current project revision via `git::capture`, calls `evidence_export::build`, prints the human or JSON document, and refuses with a non-zero exit when no run exists.
- `src/commands/meta.rs` — `completions` (all five shells via `clap_complete`) and `man` (via `clap_mangen`) generators.
- `src/mcp/{mod,server,tools}.rs` — Model Context Protocol server over stdio. `server.rs` is the JSON-RPC 2.0 dispatch loop (newline-delimited, `PROTOCOL_VERSION = "2024-11-05"`, `-32700`/`-32601`/`-32602` error codes, EOF exits 0). `tools.rs` defines four read-only tools (`top_bugs`, `show_bug`, `ai_report`, `doctor_status`) that reuse the existing CLI builders; every input schema is closed (`additionalProperties: false`). The DB is opened with `SQLITE_OPEN_READ_ONLY` via the existing `Db::open_read_only` so a write attempt fails at the driver level.
- `src/cli.rs` — `clap` derive types plus `Completions`/`Man` subcommands, `long_about` with examples, per-command `after_help` examples, and the binary-naming rule doc comment.
- `src/error.rs` — `thiserror` `Error` enum plus `hint()` single-wrap remediation for every user-facing variant.
- `src/doctor/check.rs` — `Status::{Pass,Info,Warn,Fail}` with `Check::info` constructor (unconfigured-but-ok states).
- `deny.toml` — cargo-deny policy (advisories, licenses, bans, sources) enforced in CI.
- `CHANGELOG.md` — Keep-a-Changelog history (Unreleased section started).
- `scripts/lib/{config,version,platform,release}.sh` — shared packaging helpers: target matrix, version source (Cargo.toml), host detection, and URL construction.
- `scripts/package.sh` — reproducible per-target release builder (`cargo build --release --locked`, then archive).
- `scripts/checksum.sh` — deterministic SHA-256 manifest generator from the final archives.
- `scripts/install.sh` — POSIX shell installer: strict mode, `--version`/`--dest`/`--allow-root`/`--dry-run` (early exit, no network), mandatory SHA-256 verification, HTTPS download, `cargo install` fallback on unsupported hosts, atomic install into a user-writable default.
- `scripts/bump.sh` — conventional-commit bump heuristic with cold-start at `0.1.0`.
- `npm/driftwatchdog/scripts/install-guard.js` — preinstall preflight failing fast on unsupported hosts (incl. darwin-arm64).
- `scripts/smoke.sh` — opt-in local release smoke test that drives the installer against a hand-built fixture.
- `npm/driftwatchdog/package.json` — npm package metadata (`bin` exposes `driftwatch`).
- `npm/driftwatchdog/bin/driftwatch.js` — launcher: host detection, versioned cache, manifest + archive fetch, verification, exec with forwarded args and exit status.
- `npm/driftwatchdog/lib/platform.js` — npm-side `targetFor` / `detectTarget`, normalized to the shell matrix.
- `npm/driftwatchdog/lib/release.js` — npm-side `releaseUrls`, mirrors `scripts/lib/release.sh`.
- `npm/driftwatchdog/lib/verify.js` — dependency-free HTTP, SHA-256, manifest parser, and tar.gz extractor (with path-traversal safety).
- `npm/driftwatchdog/test/launcher.test.js` — `node:test` suite covering supported/unsupported hosts, verification, round-trip exec, cache reuse, and traversal rejection.
- `tests/packaging.sh` + `tests/packaging/{test_*.sh,fixture.sh}` — bash packaging tests (target mapping, artifact naming, checksum manifest, installer, repo hygiene, agent examples) and shared fixture.
- `tests/packaging/test_change_workflow.sh` — `bfs-dfs-bfs-change-workflow` authoring check: phase order, per-phase checkboxes, final-BFS verification, spec Scenarios, negative missing-DFS fixture; auto-picked by `tests/packaging.sh`.- `tests/packaging/run_all.sh` + `tests/packaging.rs` — combined bash+node runner and a Rust integration test that invokes it from `cargo test`.
- `tests/packaging/test_agent_examples.sh` — `agent-examples` consistency test: every `examples/*/mcp.json` parses, declares a stdio server with `driftwatch` + `mcp`, and every `driftwatch <sub>` token in `workflow.md` exists in `driftwatchdog --help`; no `run`/`check` is allowed as an agent step. Auto-picked by `tests/packaging.sh`.
- `tests/packaging/test_gha_templates.sh` — `github-actions-templates` shape test: triggers (`workflow_call` + `workflow_dispatch`), installer base URL matches the release repository in `scripts/lib/release.sh`, `driftwatch check` + `report --ai` + step summary + upload-artifact anchors present, upload `path:` is `drift.md` (never `state.db` or anything under `.driftwatch/`), `fail_on_drift` documented in both files, `contents: read` set; embedded `run:` blocks pass `shellcheck -S error` when the tool is available (soft-skip otherwise). Auto-picked by `tests/packaging.sh`.
- `examples/{claude-code,opencode,aider}/` — per-harness MCP client-config fragment (`mcp.json`) and investigation workflow (`workflow.md`); Aider also documents the `report --ai > drift.md` fallback.
- `examples/README.md` — harness→destination map, stdio-only + read-only caveat, fallback pointer.
- `templates/github-actions/driftwatch-check.yml` — reusable workflow (`workflow_call` + `workflow_dispatch`); pinned `driftwatchdog` install via the same release base `scripts/lib/release.sh` ships, `driftwatch check`, always-render `report --ai > drift.md`, upload `drift.md` only, `driftwatch top --limit 5` into `$GITHUB_STEP_SUMMARY`, `contents: read`, `fail_on_drift` and `upload_report` inputs; header documents the local-first rule (`driftwatch gate` locally before archive; CI repeats it).
- `templates/github-actions/README.md` — input table, copy-vs-`uses:` decision guide, exit-code contract, and the never-upload-`state.db` warning.
- `.github/workflows/ci.yml` — fmt, clippy (`--all-targets --all-features`, `-D warnings`), workspace tests, MSRV (1.74) check, cargo-deny, tarpaulin coverage, `macos-14` tests, shellcheck + script-mode enforcement, npm tests + audit, smoke.
- `.github/workflows/release.yml` — tag-triggered matrix build (linux-x86_64, linux-arm64, darwin-x86_64 on `macos-14`), tag==Cargo version assertion, checksum manifest + SPDX SBOM generation, and `softprops/action-gh-release` upload; the publish job depends on every matrix build, so partial matrices fail before any asset ships. npm-publish carries `id-token: write` for `--provenance`.
- `.github/workflows/auto-tag.yml` — PAT-missing skip with notice, `[no-release]` skip, remote tag-spam guard, cold-start bump.
- `.github/dependabot.yml` — weekly cargo/npm/actions updates.

## Known environment note

OpenSpec Codex skill generation initially hit a read-only sandbox directory. The integration was regenerated successfully in the writable host checkout with `openspec init --tools codex --force` and `openspec update --force`. Restart the IDE if slash commands are not visible.

## Next action

**v0.6 (Integrate) is complete.** All three v0.6 change packages
(`mcp-read-tools`, `agent-examples`, `github-actions-templates`) are
archived as of 2026-09-14, the packaging suite runs 8/8 green (incl.
`change_workflow` and `gha_templates`), and
the Rust test suite passes (631 tests, incl. 24 gate-contract + 15
gate-manifest + 30 evidence/artifact + 28 toolchain + 28 adapters +
20 context + 14 ai-evaluation + 9 gate-cli + 4 gate-concerns + 17
product-quality adapter + 15 gate-product-quality integration). All
nine v1.1 Engineering Gate packages, **`bfs-dfs-bfs-change-workflow`,
`generic-gate-contract`, `gate-project-configuration`,
`evidence-and-artifacts`, `toolchain-management-and-execution`,
`gate-adapter-evaluation`, `generic-context-providers`,
`gate-ai-evaluation`, and `gate-cli-and-memory-integration`, are
implemented and archived** as of 2026-09-15. Local schema version is
6, export schema is v4.

The `ai-gate-manifest-consumption` change is **implemented and archived**
(2026-09-20): `driftwatch gate` consumes a business project's
`.ai-gate/gate.yaml` (native `gate.toml` still wins precedence), honors the
`runtime` field, maps the `blocking` status list, and accepts project
domain profiles via `[profiles.<name>]`. Rust test suite is now 571 green;
`cargo deny check` (advisories/licenses/bans/sources) is green. This
unblocks the BYOK Translator `product-foundation-and-governance` Gate, whose
`.ai-gate/gate.yaml` declares `runtime: driftwatchdog` and profile
`browser-extension`.

The `checker-machine-output` change is **implemented and archived**
(2026-09-24): `driftwatch check --format human|json` emits a versioned
`driftwatch-checker/0.1.0` document on stdout in JSON mode; human text,
dry-run persistence, and exit-status semantics are unchanged. Rust test
suite is now 579 green (445 lib + 32 check integration + 6 export
integration + 24 report + … + 8 new `src/checker/json.rs` unit + 8 new
`tests/check.rs` integration).

The `product-quality-gate-contract` change is **implemented and archived**
(2026-09-24): `product` and `rust-product` built-in profiles select the
`product-code-boundary` and `placeholder-threshold` concern IDs;
`run_product_quality_adapter` parses a versioned JSON envelope and
applies the exit-code authority rule (mismatches and malformed envelopes
downgrade to `REVIEW_REQUIRED`). Rust test suite is now 631 green.
`driftwatch check` and every other Gate concern path are unchanged. One
planning change remains authored but unselected:
`release-evidence-and-capability-gate`. Per the standing auto-mode
authorization, the next in dependency order is the one that has no
remaining dependencies on the v1.1 Engineering Gate queue — pick the
candidate and proceed under the "Change completion workflow" at the
top of this file.

The `release-evidence-and-capability-gate` change is **implemented and
archived** (2026-09-24): `release` built-in profile selects
`release-evidence` and `capability-conformance` concern IDs; both
dedicated envelope adapters share the product-quality wire version and
exit-code authority rule and add a release-evidence required-evidence
guard + stale-revision guard and a capability-conformance
required-evidence guard + out-of-scope-verified guard. Both
release-gate adapters deliberately do not provide a text-mode fallback
so missing coverage can never be silently treated as a pass. Rust
test suite is now 679 green (502 lib + integration + packaging). No
further change remains authored; per the standing auto-mode
authorization, the next change requires explicit selection.

Follow the "Change completion workflow" at the top of this file
whenever the next change is ready to archive.
