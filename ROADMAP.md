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

## Explicit non-goals for v1

Cloud sync, accounts, team permissions, issue tracking, a Sentry replacement, a
CI server, automatic code modification, a built-in LLM, a vector database, a
full AST engine, an IDE, and a complex SPA remain out of scope. AI evaluation is
an optional provider boundary, not a built-in LLM dependency.

## Release gates

Each change must include unit/integration coverage for its public behavior, preserve the language-agnostic CLI contract, avoid network access by default, and pass the repository's OpenSpec validation before implementation is considered ready.
