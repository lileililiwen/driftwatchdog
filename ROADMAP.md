# Driftwatch Roadmap

Driftwatch is a local-first, language-agnostic CLI for runtime failure memory in AI-assisted coding workflows. It is implemented in Rust, but monitored projects may use Rust, C#, Python, JavaScript/TypeScript, Go, Java, Flutter, or any other toolchain that can be launched as a child process.

## Product boundaries

- Core workflow: record command runs, normalize failures, aggregate recurring bugs, correlate them with external spec-checker alerts, and produce human- and AI-readable reports.
- Local by default: no account, telemetry, network upload, or LLM API call; state lives under `.driftwatch/`.
- CLI first: a future viewer is read-only and cannot become a dependency of the core.
- Integrate existing tools rather than implementing AST parsing, test frameworks, spec standards, CI, code repair, or an embedded LLM.
- Heuristic correlations are leads, never root-cause claims.

## Delivery sequence

| Release | Change packages | Outcome |
| --- | --- | --- |
| v0.1 Remember | `project-foundation`, `runtime-memory`, `fingerprinting-and-retention` | Initialize a project, wrap any command, remember recurring failures, report them, and retain compact history. |
| v0.2 Observe | `export-and-doctor` | Inspect, export, and diagnose the local installation. |
| v0.3 Connect | `checker-and-drift-alerts` | Run configured external checkers through a stable JSON protocol and retain drift snapshots. |
| v0.4 Correlate | `correlation-and-ai-context` | Relate recurring failures to alerts, support manual links, and generate AI context. |
| v0.5 Distribute | `linux-macos-distribution` | Ship native Linux + Intel macOS binaries through a shell installer, an npm launcher, direct downloads, and reproducible CI. |
| v0.6 Integrate | `mcp-read-tools`, `agent-examples`, `github-actions-templates` | Serve read-only MCP tools over stdio, ship tested Claude Code/OpenCode/Aider examples, and publish a reusable GitHub Actions check template. |
| v1.0 Stable | Future hardening changes | Stabilize schema, config, checker protocol, CLI, cross-platform behavior, documentation, and real-project validation. |
| v1.1 Engineering Gates | Nine planned packages (all nine archived) | Add generic Gate contracts, project policy, evidence, tool lifecycle, adapters, optional context providers, AI evaluation, and local Gate/history integration. |
| v1.2 Product-quality Gate | `product-quality-gate-contract` | Add `product`/`rust-product` built-in profiles, the `product-code-boundary` and `placeholder-threshold` concern IDs, and a versioned JSON-envelope adapter with exit-code authority so a project-owned checker can report product-quality status without Driftwatchdog embedding a language scanner. |
| v1.3 Release-evidence and capability-conformance Gate | `release-evidence-and-capability-gate` | Add `release` built-in profile plus the `release-evidence` and `capability-conformance` concern IDs with a versioned JSON-envelope adapter (exit-code authority, required-evidence guards, stale-revision and out-of-scope-verified downgrade to REVIEW_REQUIRED) so project-owned publisher- or scanner-free commands can report release evidence and capability verification without Driftwatchdog becoming a release publisher, signer, SBOM generator, or deployment executor. |
| v1.4 Governance evidence export | `gate-evidence-export` | Add a read-only `driftwatch gate evidence-export` subcommand that maps a completed Gate run into the workspace-governance `release_evidence` vocabulary (`revision`, `version`, `toolchain`, `artifacts`, `digests`, `sbom`, `provenance`, `checks`, `publication`) with one entry per field carrying `state` (`verified` / `unverified` / `blocked`) and an `evidence_ref` into the bounded-evidence store. A field is `verified` only when a scheduled check actually ran and passed on the current revision; a stale run, a failed check, a not-scheduled field, or a could-not-execute check yields `unverified` or `blocked`; an unknown governance field is a construction error. Refuses when no run exists. The export is a pure read; Driftwatchdog does not become a publisher, signer, SBOM generator, or deployment executor. |
| v1.5 Post-MVP README readiness | `post-mvp-readiness` | Documentation-only readiness pass: a checked-in redacted terminal capture under `docs/assets/` (synthetic `driftwatch run` failure + recurring-bug report, plus a `driftwatch gate --dry-run` + `driftwatch gate` + `driftwatch gate evidence-export` capture, no external image host, no real repository data), a consolidated built-in Gate profile and concern catalog tying each built-in profile (`minimal`, `backend`, `frontend`, `full`, `product` / `rust-product`, `release`) to its stable concern IDs, default `required = true`, exit-code authority rule, and text-mode fallback behaviour (sourced from `src/gate/concerns.rs` and `src/gate/manifest.rs`), and an ordered integration recipe from no manifest to a first `driftwatch gate` run covering `gate.toml`, `.ai-gate/gate.yaml`, blocking policy, and optional context providers. The catalog and the recipe explicitly note that 75 workspace `.ai-gate/gate.yaml` files name `runtime: driftwatchdog`, so Driftwatchdog owns the central workspace Gate. No `src/**`, concern ID, envelope, wire version, CLI surface, profile, rule, or canonical spec was changed. |

## Closed change inventory

The following change packages have been implemented and archived:

- `project-foundation` — Rust CLI, config, local directory, SQLite schema, Git metadata.
- `runtime-memory` — Wrap any command, persist and query runs, top-level empty state.
- `fingerprinting-and-retention` — Normalize failures, aggregate bugs, report, GC.
- `export-and-doctor` — Portable export (json/jsonl/markdown) and local doctor diagnostics.
- `checker-and-drift-alerts` — External checker protocol, adapters, snapshots, alerts, `driftwatch check`.
- `correlation-and-ai-context` — Heuristic correlation, manual `link`/`unlink`, and `driftwatch report --ai`.
- `linux-macos-distribution` — Shell installer, npm launcher, GitHub Actions release workflow, and SHA-256-verified archives for Linux x86_64, Linux arm64, and macOS x86_64.
- `ci-test-gates` — CI gates: fmt check, clippy with `-D warnings`, and the full test suite on every push/PR.
- `crash-hardening` — char-boundary truncation, UTF-8-once capture, signal-aware status, opt-in `run --timeout-ms` with process-group kill, and capture diagnostics.
- `quality-cicd-docs-ux` — `[lints.clippy] all = "deny"`, MSRV `1.74`, `deny.toml`, proptest seeds, CI breadth (`--all-features` clippy, MSRV, cargo-deny, tarpaulin, `macos-14`, shellcheck, npm audit, smoke), release integrity (provenance, SPDX SBOM, tag==Cargo assertion, Dependabot, auto-tag hardening, `bump.sh`), and UX (`completions`, `man`, per-command examples, `hint:` lines, `driftwatch.toml.example`, repo-hygiene packaging test).
- `mcp-read-tools` — stdio JSON-RPC 2.0 MCP server with four read-only tools (`top_bugs`, `show_bug`, `ai_report`, `doctor_status`), closed input schemas, `SQLITE_OPEN_READ_ONLY` open, and the `driftwatch mcp` subcommand.
- `agent-examples` — tested copy-paste MCP + workflow examples for Claude Code, OpenCode, and Aider; Aider includes the `report --ai > drift.md` fallback; consistency test in `tests/packaging/test_agent_examples.sh`.
- `github-actions-templates` — reusable `templates/github-actions/driftwatch-check.yml` (`workflow_call` + `workflow_dispatch`) that pins a `driftwatchdog` install, always renders the AI report, uploads only `drift.md`, summarises `top` into the step summary, with `contents: read` and the `fail_on_drift`/`upload_report` inputs; shape test in `tests/packaging/test_gha_templates.sh`.
- `checker-machine-output` — `driftwatch check --format human|json` with a versioned `driftwatch-checker/0.1.0` document on stdout (declaration order, `ok`/`alerting`/`failed`/`timeout`/`protocol-error` statuses, parsed `alerts[]` in the existing wire shape, bounded `error` note, summary counts) while human output, dry-run persistence and exit-status semantics are unchanged. Enables the Forge `driftwatch-cli-alignment` consumption path.
- `product-quality-gate-contract` — `product` and `rust-product` built-in profiles, stable concern IDs `product-code-boundary` and `placeholder-threshold`, versioned JSON-envelope result normalization (wire version `1`, exit-code authority rule: `PASS`→0, `FAIL`→1, `REVIEW_REQUIRED`→2, `NOT_APPLICABLE`→0, mismatches and malformed envelopes downgrade to `REVIEW_REQUIRED`), text-mode fallback for legacy commands, and end-to-end coverage including persistence, `--format json`, dry-run, and optional-concern relaxation. Pure data + bounded redaction; no provider SDKs or embedded scanners.
- `release-evidence-and-capability-gate` — `release` built-in profile + stable concern IDs `release-evidence` and `capability-conformance`; shared wire shape with the product-quality envelope (wire version `1`, exit-code authority rule, bounded fields, secret redaction); release-evidence required-evidence guard (PASS requires `revision`, `product_version`, at least one `artifacts` entry, and `provenance`; stale-revision vs current git rev downgrades to `REVIEW_REQUIRED`); capability-conformance required-evidence guard (PASS requires non-empty `verified` whose ids are all in the resolved plan; out-of-scope verified ids downgrade to `REVIEW_REQUIRED`); malformed/missing/wrong-version envelopes downgrade to `REVIEW_REQUIRED`; text-mode fallback is deliberately not provided for these concerns so missing coverage can never be silently treated as a pass. Driftwatchdog remains an executor/aggregator: no release publisher, signer, SBOM generator, or deployment executor is added.
- `gate-evidence-export` — read-only `driftwatch gate evidence-export` subcommand (also `--format json` / `--dry-run`) that maps a completed `gate_runs` row into the workspace-governance `release_evidence` vocabulary (closed field set `revision` / `version` / `toolchain` / `artifacts` / `digests` / `sbom` / `provenance` / `checks` / `publication`, closed state set `verified` / `unverified` / `blocked`); per-field `state` priority `verified > blocked > unverified` with `verified` reserved for scheduled checks that ran and passed on the current revision; stale-revision rule emits every field `unverified` with a `stale` diagnostic naming both revisions; unknown governance field is an `ExportError::UnknownField` construction error; refuses with a non-zero exit when no run exists; pure read of the existing `gate_runs` row and the bounded-evidence store, no new schema, no new evidence store, no publisher / signer / SBOM generator / deployment executor, `driftwatch check` and the existing concern IDs and envelopes are unchanged.
- `post-mvp-readiness` — documentation-only README readiness pass: a checked-in redacted terminal capture under `docs/assets/run-failure.txt` + `docs/assets/gate-run.txt` (synthetic project, no real repository data, no external image host), a consolidated built-in Gate profile and concern catalog in `README.md` (profile → stable concern IDs → default `required = true` → exit-code authority rule → text-mode fallback behaviour) sourced from `src/gate/concerns.rs` and `src/gate/manifest.rs`, and an ordered integration recipe from no manifest to a first `driftwatch gate` run covering `gate.toml`, `.ai-gate/gate.yaml`, blocking policy, and optional context providers. The catalog and recipe explicitly note that 75 workspace `.ai-gate/gate.yaml` files name `runtime: driftwatchdog`, so Driftwatchdog owns the central workspace Gate. No `src/**`, concern ID, envelope, wire version, CLI surface, profile, rule, or canonical spec was changed; Driftwatchdog remains an executor/aggregator.

## Change dependency graph

```text
project-foundation
        ↓
runtime-memory → fingerprinting-and-retention → export-and-doctor
        ↓                                      ↓
checker-and-drift-alerts ───────────────→ correlation-and-ai-context
                                                ↓
                                        linux-macos-distribution
                                                ↓
                                          mcp-read-tools
                                          ↓            ↓
                          github-actions-templates  agent-examples
```

The first three packages form the smallest useful runtime-memory release. Checker integration remains optional until that foundation is stable. Distribution packaging (v0.5) layers on top of the stable CLI without altering its contract.

## v1.1 Engineering Gate planning queue

All nine packages are implemented and archived
(`bfs-dfs-bfs-change-workflow`, `generic-gate-contract`,
`gate-project-configuration`, `evidence-and-artifacts`,
`toolchain-management-and-execution`, `gate-adapter-evaluation`,
`generic-context-providers`, `gate-ai-evaluation`,
`gate-cli-and-memory-integration`). No planning packages remain.
The archived dependency order was:

```text
bfs-dfs-bfs-change-workflow
        ↓
generic-gate-contract
        ↓
gate-project-configuration ───────┐
        ↓                          │
evidence-and-artifacts             │
        ↓                          │
toolchain-management-and-execution│
        ↓                          │
gate-adapter-evaluation            │
        ↓                          │
generic-context-providers          │
        ↓                          │
gate-ai-evaluation                 │
        ↓                          │
gate-cli-and-memory-integration ←─┘
```

OpenSpec is an optional context provider in this queue. The generic Gate
contract remains usable by projects using OpenAPI, Gherkin, Markdown, Jira, or
no specification framework.

## Proposed planning queue (not selected)

*(empty — every authored package has been promoted.)*

The previously proposed `release-evidence-and-capability-gate` planning
package is implemented and archived as v1.3 of the roadmap.

## Explicit non-goals for v1

Cloud sync, accounts, team permissions, issue tracking, a Sentry replacement, a
CI server, automatic code modification, a built-in LLM, a vector database, a
full AST engine, an IDE, and a complex SPA remain out of scope. AI evaluation is
an optional provider boundary, not a built-in LLM dependency.

## Release gates

Each change must include unit/integration coverage for its public behavior, preserve the language-agnostic CLI contract, avoid network access by default, and pass the repository's OpenSpec validation before implementation is considered ready.
