# Driftwatchdog

Your AI fixed this bug three times already.

Driftwatchdog remembers it.

Driftwatchdog is a local-first, language-agnostic CLI for runtime failure memory in AI-assisted coding workflows. It records failed commands, recognizes recurring bugs, consumes external spec-checker results, and produces context that humans and coding agents can act on.

Rust is Driftwatchdog's implementation language—not a restriction on the projects it monitors. Run Rust, Python, C#, JavaScript/TypeScript, Go, Java, Flutter, or any other command-line tool through the same interface.

## Why

AI coding agents often rediscover the same failure because the project has no durable runtime memory. Driftwatchdog connects:

    runtime command -> failure memory -> recurring bug
                                  -> spec alerts -> possible relationship

It is not an APM platform, test framework, spec parser, CI server, code fixer, or built-in LLM.

## Quick start

    # Pick an installation channel (see "Installation" below)
    cargo install --path .
    curl -fsSL https://github.com/lileililiwen/driftwatchdog/releases/latest/download/install.sh | sh
    npm i -g driftwatchdog

    # Initialize local state
    driftwatchdog init

    # Wrap any project command
    driftwatchdog run cargo test
    driftwatchdog run pytest
    driftwatchdog run npm test

    # Inspect runtime memory
    driftwatchdog list
    driftwatchdog top
    driftwatchdog report
    driftwatchdog report --ai > drift.md  # AI-oriented context report

State is stored locally in .driftwatch/state.db. By default Driftwatchdog does not upload code, logs, or specs, does not use telemetry, and does not call an LLM API.

## Installation

Driftwatchdog ships as a single native executable. The release distribution supports three installation channels; pick whichever fits your environment. All three install the same Rust binary; only the acquisition step differs. Network access is required only at install time, not at runtime.

**Binary naming rule:** installs from Cargo, the shell installer, and direct downloads expose the binary as `driftwatchdog`; the npm launcher exposes the same binary as `driftwatch`. Examples below use `driftwatch`; replace with `driftwatchdog` when that is the binary on your `PATH`.

Supported platforms:

| Operating system | Architecture | Suffix        |
| ---------------- | ------------ | ------------- |
| Linux (glibc)    | x86_64       | `linux-x86_64` |
| Linux (glibc)    | arm64 / aarch64 | `linux-arm64` |
| macOS            | x86_64 (Intel) | `darwin-x86_64` |

macOS arm64, Windows, and musl-based Linux are intentionally not part of the supported set in this release.

### Shell installer (Linux, macOS)

Pipes a small POSIX script from this repository, detects the host, downloads the matching archive and SHA-256 manifest from GitHub Releases, verifies the archive, and installs the `driftwatchdog` binary to a user-writable directory. No `sudo` is required by default; the destination defaults to `${XDG_BIN_HOME:-$HOME/.local/bin}`.

    # Latest release, default destination
    curl -fsSL https://github.com/lileililiwen/driftwatchdog/releases/latest/download/install.sh | sh

    # Pin a specific version
    curl -fsSL https://github.com/lileililiwen/driftwatchdog/releases/latest/download/install.sh \
      | sh -s -- --version v0.1.0

    # Install to an explicit directory
    curl -fsSL https://github.com/lileililiwen/driftwatchdog/releases/latest/download/install.sh \
      | sh -s -- --dest /opt/driftwatchdog --allow-root

    # Preview without downloading
    curl -fsSL https://github.com/lileililiwen/driftwatchdog/releases/latest/download/install.sh \
      | sh -s -- --dry-run

The installer refuses to run as root unless you pass `--allow-root`, refuses to install over an existing binary if checksum verification fails, and exits non-zero with the detected OS/architecture on unsupported hosts. Verification is mandatory and cannot be skipped. On unsupported hosts (macOS arm64, Windows, musl Linux) it prints the supported target list and suggests the source-build fallback: `cargo install --locked driftwatchdog`.

### npm (Linux, macOS)

The `driftwatchdog` npm package is a dependency-free launcher: it detects the host, downloads and verifies the matching native archive into a versioned cache, and forwards all arguments and the exit status to the native binary. After install the command is `driftwatch` (not `driftwatchdog`).

    npm i -g driftwatchdog
    driftwatch init      # the launcher exposes the binary as `driftwatch`

    # or, after install, run the same commands:
    driftwatch list
    driftwatch report --ai

The launcher reports unsupported hosts (including macOS arm64) with the supported target list and points at the shell installer, direct downloads, and `cargo install driftwatchdog` as alternatives. It does not execute an unverified binary and does not leave a failed download in the cache.

### Direct download

Every release on GitHub publishes three archives and a `checksums.txt` manifest. Download both files, verify the archive with `sha256sum -c`, and extract the `driftwatchdog` executable anywhere on your `PATH`.

    VERSION=v0.1.0
    OS=linux-x86_64
    curl -fsSL -O "https://github.com/lileililiwen/driftwatchdog/releases/download/${VERSION}/driftwatchdog-${VERSION}-${OS}.tar.gz"
    curl -fsSL -O "https://github.com/lileililiwen/driftwatchdog/releases/download/${VERSION}/driftwatchdog-${VERSION}-checksums.txt"
    sha256sum -c "driftwatchdog-${VERSION}-checksums.txt" --ignore-missing
    tar -xzf "driftwatchdog-${VERSION}-${OS}.tar.gz"
    install -m 0755 "driftwatchdog-${VERSION}-${OS}/driftwatchdog" "${HOME}/.local/bin/"

The manifest signs only the published archives; use `--ignore-missing` (or extract the matching line first) when verifying a single platform.

### `cargo install`

For Rust projects that already have a toolchain available, building from source remains an option:

    cargo install driftwatchdog

The local source checkout works the same way:

    git clone https://github.com/lileililiwen/driftwatchdog
    cd driftwatchdog
    cargo install --path .



## Example

    $ driftwatchdog run cargo test

    FAIL

    Recurring bug detected:
    #12 database connection timeout
    Seen 6 times before.

After checker integration:

    driftwatchdog check
    driftwatchdog link bug:abcdef12 spec:42 --note "see issue #108"
    driftwatchdog report --ai > drift.md

The AI report contains recurring failures, current spec violations, possible heuristic relationships and manual links, recent commits, and instructions to investigate recurrence and add regression coverage. It deliberately does not claim that a similarity score proves root cause.

## Commands

    driftwatchdog init
    driftwatchdog run <command> [args...]
    driftwatchdog list [--limit N] [--failed] [--tag TAG]
    driftwatchdog top [--limit N] [--days N] [--tag TAG]
    driftwatchdog show <bug-id>
    driftwatchdog report [--ai] [--limit N] [--days N] [--tag TAG]
    driftwatchdog check [--only NAMES] [--dry-run]
    driftwatchdog gate [--dry-run] [--format human|json]
    driftwatchdog link bug:<id> spec:<id> [--note "..."]
    driftwatchdog unlink <link-id>
    driftwatchdog export json|jsonl|markdown
    driftwatchdog gc [--days N]
    driftwatchdog doctor
    driftwatchdog mcp
    driftwatchdog completions <shell>
    driftwatchdog man

Run `driftwatch --help` (or `driftwatch <command> --help`) for per-command
examples. Shell completions cover bash, zsh, fish, powershell, and elvish;
`driftwatch man` prints a man page to stdout.

## MCP server (read-only)

`driftwatchdog mcp` serves a read-only Model Context Protocol surface on
stdio. The transport is newline-delimited JSON-RPC 2.0; the protocol
version is `2024-11-05`. The server opens the local database with
`SQLITE_OPEN_READ_ONLY` so a write attempt fails at the driver level.
There are no HTTP/SSE transports, no write or execute tools, and no new
runtime dependencies (`serde`/`serde_json` already cover the protocol).

Four tools are advertised:

| Tool | Input | Output |
| --- | --- | --- |
| `top_bugs` | `{limit?, days?, tag?}` | JSON array of `top` rows (`hash`, `summary`, `count`, `first_seen_at`, `last_seen_at`). |
| `show_bug` | `{id: string}` | Same text the `driftwatch show` command prints; unknown ids surface a `hint:` remediation. |
| `ai_report` | `{limit?, days?, tag?}` | Bytes identical to `driftwatch report --ai`. |
| `doctor_status` | `{}` | Same text the `driftwatch doctor` command prints. |

Every tool input is a closed JSON object (`additionalProperties: false`),
so a misnamed key is rejected with `-32602` at the boundary. Failure
shapes follow JSON-RPC 2.0 (`-32700` parse error, `-32601` method/tool
not found, `-32602` invalid params, tool-level failures surface as a
result with `isError: true` and the `hint:` line).

Client-agnostic registration: point the MCP-aware client's stdio
command at `driftwatch mcp` (or `driftwatchdog mcp` when installed via
Cargo / the shell installer). The server is offline and never spawns
child processes, so it is safe to enable in unattended contexts.

## Integrations

Tested, copy-paste MCP client configs and investigation workflows
live under `examples/`. Each directory is a self-contained drop-in
for one agent harness.

| Directory                       | Harness       | Notes |
| ------------------------------- | ------------- | ----- |
| `examples/claude-code/`         | Claude Code   | stdio MCP server + investigation loop. |
| `examples/opencode/`            | OpenCode      | stdio MCP server + investigation loop. |
| `examples/aider/`               | Aider         | stdio MCP server (optional) and the `report --ai > drift.md` fallback recommended for every Aider version. |

The investigation loops restate the same instructions the `report
--ai` command emits, so behaviour stays consistent with the
documented contract regardless of which harness the agent runs
inside. The human drives every command that mutates state; the MCP
surface is read-only and never executes Driftwatch commands on the
agent's behalf. See `examples/README.md` for the full copy map and
caveats, and `tests/packaging/test_agent_examples.sh` for the
consistency test that guards every JSON fragment and every
referenced subcommand.

## Continuous integration

A reusable GitHub Actions workflow lives at
`templates/github-actions/driftwatch-check.yml`. It installs a
pinned `driftwatchdog` binary, runs the configured external
checkers, always renders `driftwatch report --ai > drift.md`, and
uploads `drift.md` as a single `drift-report` artifact. `.driftwatch/state.db`
and command logs are deliberately never published.

The template exposes two triggers (`workflow_call` and
`workflow_dispatch`) and three inputs:

| Input            | Type    | Default   | Purpose |
| ---------------- | ------- | --------- | ------- |
| `version`        | string  | `latest`  | Driftwatch version to install. |
| `fail_on_drift`  | boolean | `false`   | Fail the job when `driftwatch check` exits non-zero. The AI report still uploads. |
| `upload_report`  | boolean | `true`    | Upload `drift.md` as the `drift-report` artifact. |

Reuse it from another workflow with `uses:`:

```yaml
jobs:
  driftwatch:
    uses: lileililiwen/driftwatchdog/.github/workflows/driftwatch-check.yml@v0.6.0
    with:
      version: v0.6.0
      fail_on_drift: false
    permissions:
      contents: read
```

Or copy the file into `.github/workflows/` of your own repository
when a project-specific step is needed before or after the
Driftwatch block. See `templates/github-actions/README.md` for the
full input table, exit-code contract, and copy-vs-`uses:` decision
guide. `tests/packaging/test_gha_templates.sh` is the shape test
that guards the install base URL, the upload path, the
`fail_on_drift` input, and the `contents: read` permission.

## External checkers

Driftwatchdog does not define a spec format. Copy `driftwatch.toml.example`
to `driftwatch.toml` and run `driftwatch check` — the bundled stub checker
succeeds without edits:

    cp driftwatch.toml.example driftwatch.toml
    driftwatch check --dry-run
    driftwatch check
    driftwatch check --format json   # versioned checker-report document on stdout

Configure existing local tools in driftwatch.toml
(see `driftwatch.toml.example` for a copy-paste starter):

    [[checkers]]
    name = "architecture"
    command = "my-spec-checker"
    args = ["--json"]
    # working_dir = "tools/spec"  # relative to the project root, jailed inside it
    # timeout_ms = 30000

Checker config is strict: duplicate names, empty commands, `timeout_ms = 0`,
unknown fields (with `did you mean` hints), and `working_dir` escapes fail
fast with an actionable error.

The checker emits a stable JSON document:

    {
      "alerts": [
        {
          "severity": "warning",
          "message": "Connection timeout does not follow spec",
          "source": "specs/database.md",
          "symbol": "DbPool"
        }
      ]
    }

Checker failures are isolated: one broken or malformed checker result must not prevent remaining checkers from running.

`driftwatch check --format json` emits a versioned
`driftwatch-checker/0.1.0` document to stdout with the per-checker
`status` mapped to one of `ok` / `alerting` / `failed` / `timeout` /
`protocol-error`, the parsed `alerts[]` array in the same wire shape
the checker wrote, and a `summary` of counts. The dry-run banner and
any correlation warning move to stderr so the document is the only
thing on stdout. Execution, isolation, persistence, and exit-status
semantics are byte-for-byte identical to human mode, so a CI gate
that reads JSON cannot drift from a developer running plain
`driftwatch check`. Use this for control-plane consumers (e.g. the
Forge policy adapter) that drive checker-only projects; projects with
a `gate.toml` already have `driftwatch gate --format json` for the
manifest-driven path.

## Engineering Gate (local-first)

Projects with a `gate.toml` manifest can run the broader Gate locally
before every relevant change is archived or completed:

    driftwatch gate --dry-run   # show the resolved plan; no execution, no persistence
    driftwatch gate             # execute applicable checks, persist the result, exit nonzero when blocked
    driftwatch gate --format json  # machine-readable status document

`driftwatch gate` resolves the plan (profile, explicit checks, blocking
policy, project commands, rule-pack identity), executes each planned
check through the bounded project-runtime adapter (a check without a
declared `command` records `NOT_APPLICABLE` so missing coverage is
explicit), optionally runs one `ai-review` evaluation when `[ai]
enabled = true`, aggregates with the manifest blocking policy, and
persists one `gate_runs` row with change/revision identity and
manifest digest. `driftwatch check` remains the compatibility entry
point for legacy checker-only projects.

### Business-project policy at `.ai-gate/gate.yaml`

Bootstrapped business repositories record Gate policy in
`.ai-gate/gate.yaml` and name `driftwatchdog` as the shared runtime. The
Gate resolves `gate.toml`, then `.driftwatch/gate.toml`, then
`.ai-gate/gate.yaml` (native `gate.toml` wins when both exist) and runs
the same pipeline. A manifest whose `runtime` names another tool is
reported and skipped — nothing is executed or persisted.

```yaml
version: 1
runtime: driftwatchdog
profile: browser-extension        # built-in or project-defined
rule_pack: browser-extension@0.1.0
checks:                           # true | false | "optional"
  build: true
  tests: true
  accessibility: optional
commands:                         # bind a selected concern to a command
  build: "npm run build"
  tests: "npm test"
blocking: [FAIL, REVIEW_REQUIRED]
project_commands:
  lint: "npm run lint"
contexts: [git, openspec]
```

A `profile` that is not a built-in name selects exactly its declared
`checks`; a built-in profile keeps its default set, adjustable through
`checks` (`false` removes, `"optional"` schedules non-blocking). The
same domain profiles are available in `gate.toml` via a
`[profiles.<name>]` table listing the concerns the profile selects.

Local verification comes first: a change that has not passed its local
Gate must not be represented as complete merely because CI is
configured. CI templates repeat the same command as a second layer.

### Product-quality Gate contract

Two built-in profiles, `product` and `rust-product`, schedule the
`product-code-boundary` and `placeholder-threshold` concerns. The
profiles are aliases: `rust-product` exists for projects that want a
Rust-flavored name without changing the concern set. Both concerns are
required by default; relax an individual concern to `required = false`
when a project cannot bind a command yet.

The concerns are data only: Driftwatchdog never embeds a language
scanner, runs a provider SDK, or installs a tool. The project binds a
command to each concern in `gate.toml` (or `.ai-gate/gate.yaml`):

```toml
version = 1
profile = "product"
[[checks]]
id = "product-code-boundary"
command = "tools/product-boundary check --envelope"
[[checks]]
id = "placeholder-threshold"
command = "tools/placeholder-threshold check --envelope"
```

When the command runs, the adapter parses the versioned JSON envelope
emitted on stdout and applies the **exit-code authority rule**:

| Envelope status      | Required exit code | Adapter result |
| -------------------- | ------------------ | -------------- |
| `PASS`               | `0`                | `PASS`         |
| `FAIL`               | `1`                | `FAIL`         |
| `REVIEW_REQUIRED`    | `2`                | `REVIEW_REQUIRED` |
| `NOT_APPLICABLE`     | `0`                | `NOT_APPLICABLE` |

A status/exit mismatch, a missing envelope, an unparsable envelope, or
an unknown `version` field all downgrade the result to
`REVIEW_REQUIRED` so missing coverage is never silently treated as a
pass. Infrastructure failures (spawn, timeout, signal) stay
`REVIEW_REQUIRED` and name the missing evidence key. When the command
exits cleanly but emits no envelope, the adapter falls back to the
existing text-mode mapping (exit `0` → `PASS`, nonzero → `FAIL`) so
commands that pre-date the envelope contract keep working.

Envelope shape (wire version `1`):

```json
{
  "version": 1,
  "status": "FAIL",
  "severity": "error",
  "findings": [
    {
      "title": "test code reached product source",
      "severity": "error",
      "location": "src/lib.rs",
      "rule": "no-tests-in-product"
    }
  ],
  "evidence": [],
  "missing_evidence": [],
  "diagnostic": "see findings",
  "remediation": "move the offending test to tests/"
}
```

`severity` defaults to `info` for `PASS`, `error` for `FAIL`,
`warning` for `REVIEW_REQUIRED`, and `info` for `NOT_APPLICABLE` when
the envelope omits it. Every string is bounded and secret-redacted
using the same limits the rest of the Gate contract applies
(`MAX_FINDINGS`, `MAX_DIAGNOSTIC_BYTES`, `MAX_REMEDIATION_BYTES`,
`MAX_EVIDENCE_REFS`, `MAX_MISSING_EVIDENCE`, etc.), so a malformed or
oversized producer cannot break the Gate's output invariants. The
`rust-product` profile is a strict alias for `product`; the two names
select the same concerns with the same default severity model.

## Project status

v0.1 through v0.6 are shipped. v1.1 (Engineering Gates) and v1.2
(Product-quality Gate) are also shipped: every change listed in
`ROADMAP.md` is implemented and archived, the full CLI surface (`init`,
`run`, `list`, `top`, `show`, `report --ai`, `gc`, `export`, `doctor`,
`check`, `gate`, `link`, `unlink`, `completions`, `man`, `mcp`) is
functional, the packaging suite is green (8/8 bash tests including
`agent_examples`, `gha_templates`, and `change_workflow`), and CI
enforces fmt, clippy (`-D warnings`, `--all-features`), MSRV 1.74,
cargo-deny, tarpaulin coverage, macOS (`macos-14`) parity, shellcheck,
npm audit, and the end-to-end smoke test. The next planning milestone
is v1.0 (Stable); see `ROADMAP.md` for the delivery sequence and
`HANDOFF.md` for the current implementation handoff.

## Development

The intended architecture is a single Rust binary with focused modules for CLI dispatch, process execution, generic fingerprint normalization, SQLite storage, reports, checker adapters, and heuristic correlation. Keep the core language-agnostic and local-first.

OpenSpec artifacts are validated with:

    openspec validate --changes --strict --no-interactive

See AGENTS.md for repository working rules.

## License

Licensed under the [MIT License](LICENSE). Copyright © 2026 lileililiwen.
