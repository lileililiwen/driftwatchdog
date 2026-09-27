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

A real capture from a synthetic project (no network, no real repository
data) is checked in at [`docs/assets/run-failure.txt`](docs/assets/run-failure.txt):

    $ driftwatchdog init
    $ driftwatchdog run sh -c 'echo connection timeout >&2; exit 1'   # 3 times
    $ driftwatchdog top
    COUNT    FIRST SEEN            LAST SEEN             HASH        SUMMARY
    3        2026-09-27T03:08:35Z  2026-09-27T03:08:35Z  be3c6a0f    connection timeout
    $ driftwatchdog show be3c6a0f
    Bug  : be3c6a0fb772
    Summary      : connection timeout
    Occurrences  : 3
    ...
    --- run #3 @ 2026-09-27T03:08:35Z (commit -)
        connection timeout
    --- run #2 @ 2026-09-27T03:08:35Z (commit -)
        connection timeout
    --- run #1 @ 2026-09-27T03:08:35Z (commit -)
        connection timeout

The full synthetic capture, including the `driftwatchdog report`
summary and a `driftwatchdog gate` dry-run, is in
[`docs/assets/run-failure.txt`](docs/assets/run-failure.txt) and
[`docs/assets/gate-run.txt`](docs/assets/gate-run.txt). Captures are
redacted, drawn from synthetic projects, and committed to the
repository (no external image host).

After checker integration:

    driftwatchdog check
    driftwatchdog link bug:abcdef12 spec:42 --note "see issue #108"
    driftwatchdog report --ai > drift.md

The AI report contains recurring failures, current spec violations, possible heuristic relationships and manual links, recent commits, and instructions to investigate recurrence and add regression coverage. It deliberately does not claim that a similarity score proves root cause.

## Built-in Gate profile and concern catalog

Driftwatchdog owns the central workspace Gate: 75 workspace
`.ai-gate/gate.yaml` files name `runtime: driftwatchdog`. The catalog
below ties each built-in profile to its stable concern IDs, the
default `required = true` flag, the exit-code authority rule, and
whether a text-mode fallback applies. Concern IDs and profile names
match `src/gate/concerns.rs`; the code is authoritative — if a name
here ever disagrees with the code, the code wins and the doc is
corrected.

| Built-in profile | Stable concern IDs scheduled | Default | Exit-code authority | Text-mode fallback |
| --- | --- | --- | --- | --- |
| `minimal` | (none) | — | `PASS`→0, `FAIL`→1, `REVIEW_REQUIRED`→2, `NOT_APPLICABLE`→0 (mismatches and malformed envelopes downgrade to `REVIEW_REQUIRED`) | n/a (no envelope-backed checks) |
| `backend` | `a11y`, `api-contract`, `migration`, `responsive`, `secret-scan` | required | as above (project-runtime text adapter) | yes (project-runtime `exit 0 → PASS`, `nonzero → FAIL`) |
| `frontend` | `a11y`, `responsive`, `secret-scan` | required | as above | yes |
| `full` | `a11y`, `api-contract`, `migration`, `responsive`, `secret-scan` | required | as above | yes |
| `product` (alias `rust-product`) | `placeholder-threshold`, `product-code-boundary` | required | as above (envelope wire version `1`) | yes (legacy commands keep working) |
| `release` | `capability-conformance`, `release-evidence` | required | as above (envelope wire version `1`, shared with the product-quality shape) | **no** (missing coverage must never silently pass) |

A profile that is not a built-in name selects exactly the concerns
declared in the manifest (`[profiles.<name>]` in `gate.toml` or
`checks:` in `.ai-gate/gate.yaml`); a built-in profile keeps its
default set, adjustable per concern with `required = false` (or the
`checks:` map `false` / `"optional"` in `.ai-gate/gate.yaml`). When
the manifest binds a profile default without a `command`, the
existing required + missing-command aggregate path records
`REVIEW_REQUIRED`; the catalog does not paper over that case.

## Integration recipe: from no manifest to a first `driftwatch gate` run

1. `driftwatchdog init` — creates `.driftwatch/state.db` and
   `driftwatch.toml` (the local state directory; nothing is
   uploaded).
2. Pick the integration surface.
   * Native `gate.toml` at the project root (or under
     `.driftwatch/gate.toml`) for self-contained projects.
   * `.ai-gate/gate.yaml` for business projects that already follow
     the `.ai-gate` convention; name `runtime: driftwatchdog` so the
     shared executor is the one in this repository.
3. Declare the policy.
   * `version = 1` is required.
   * `profile = "<built-in>"` selects the catalog row above. Pick
     `product` (or `rust-product`) for product-quality, `release`
     for release-gate, one of `backend` / `frontend` / `full` /
     `minimal` otherwise.
   * Bind every selected concern to a project-owned command that
     emits the versioned JSON envelope (or, for `backend` /
     `frontend` / `full`, the existing project-runtime text
     contract).
4. Override selectively.
   * `blocking = [FAIL, REVIEW_REQUIRED]` (or any subset) tightens
     the aggregate policy; the default already blocks both.
   * `required = false` (or `checks: { <id>: optional }`) relaxes
     one concern without changing the profile.
   * `[profiles.<name>]` in `gate.toml` or the `checks:` map in
     `.ai-gate/gate.yaml` declares a project-defined profile that
     selects exactly the concerns it lists.
5. Add optional context providers.
   * `contexts = [git, project-files]` enables the built-in
     providers; `[openspec]` is opt-in and stays generic (no
     OpenSpec types enter the gate core).
6. Verify locally before every change is archived.
   * `driftwatch gate --dry-run` — shows the resolved plan, the
     `manifest: sha256:…` digest, the rule-pack identity, and the
     `not_scheduled` set. Nothing is executed and nothing is
     persisted.
   * `driftwatch gate` — executes, persists one `gate_runs` row,
     prints the per-check table, and exits nonzero when blocked.
   * `driftwatch gate --format json` — machine-readable status
     document on stdout for control-plane consumers.
   * `driftwatch gate evidence-export` — versioned governance
     evidence record (read-only; refuses when no run exists).
7. CI repeats the same `driftwatch gate` invocation as a second
   layer; the local Gate is the first completion verification for
   every relevant change, not the last.

Driftwatchdog is an executor and aggregator in every step: it never
publishes, signs, generates an SBOM, or deploys. The recipe adds
provenance and capability verification only because the project's
own command produces it as data; the recipe itself introduces no
new external dependency.

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
    driftwatch gate evidence-export                  # completed-run -> Workspace Governance vocabulary
    driftwatch gate evidence-export --format json    # versioned `EvidenceExport` document

`driftwatch gate` resolves the plan (profile, explicit checks, blocking
policy, project commands, rule-pack identity), executes each planned
check through the bounded project-runtime adapter (a check without a
declared `command` records `NOT_APPLICABLE` so missing coverage is
explicit), optionally runs one `ai-review` evaluation when `[ai]
enabled = true`, aggregates with the manifest blocking policy, and
persists one `gate_runs` row with change/revision identity and
manifest digest. `driftwatch check` remains the compatibility entry
point for legacy checker-only projects.

### Exporting a run for the Workspace Governance audit

`driftwatch gate evidence-export` reads the latest completed
`gate_runs` row and prints a versioned evidence record in the
Workspace Governance `release_evidence` vocabulary. The export is a
pure read: it never mutates the run, the project, or any registry,
and it never invokes an LLM, signer, SBOM generator, or publisher.
One entry is emitted per governance field (`revision`, `version`,
`toolchain`, `artifacts`, `digests`, `sbom`, `provenance`, `checks`,
`publication`) carrying a state of `verified`, `unverified`, or
`blocked` and an `evidence_ref` into the bounded-evidence store. A
field is `verified` only when a scheduled check actually ran and
passed and the run's revision matches the project's current
revision; a check that failed, was not scheduled, or could not
execute stays `unverified` or `blocked`. When no run exists, the
export refuses with a non-zero exit and emits no document. The
governance-side acceptance of the record belongs to
`workspace-governance`, not to this repository.

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

### Release-evidence and capability-conformance Gate contract

A third built-in profile, `release`, schedules the two release-gate
concerns: `release-evidence` and `capability-conformance`. Both are
project-owned: the project binds a command that emits a versioned
JSON envelope on stdout, and Driftwatchdog executes the command,
normalises the result, and aggregates. Driftwatchdog never becomes a
release publisher, signer, SBOM generator, or deployment executor; it
treats the producer as data.

```toml
version = 1
profile = "release"
[[checks]]
id = "release-evidence"
command = "tools/release-evidence emit --envelope"
[[checks]]
id = "capability-conformance"
command = "tools/capability-conformance emit --envelope"
```

Both concerns share the same wire shape on the outside — exit-code
authority, version-gated envelope parser, bounded fields, secret
redaction — but each applies its own evidence rules.

**`release-evidence`** records the source revision, the released
product version, the artifact list, and provenance. A `PASS` claim
must include `revision`, `product_version`, at least one entry in
`artifacts`, and `provenance` (object or string). When the project
provides a current source revision (typically from `git`), the
adapter compares it to the envelope's `revision`; a mismatch refuses
the `PASS` and downgrades the result to `REVIEW_REQUIRED` so stale
evidence never silently passes the gate. A missing, malformed,
wrong-version, or out-of-sync envelope all become `REVIEW_REQUIRED`
with the missing evidence key recorded. The `product_version` field
names the released product version (`1.2.3`); the envelope wire
version stays the simple `version` field that every Gate envelope
uses.

**`capability-conformance`** records the declared, configured,
verified, and unverified capabilities of the project. A `PASS` claim
must list at least one entry in `capabilities.verified`; every
verified id must also appear in the resolved gate plan (the
`scheduled_ids` for the run), so the gate refuses a claim to verify
something it cannot reproduce. An empty `verified` list, a verified id
outside the plan, or any of the malformed/missing envelope cases
all become `REVIEW_REQUIRED`.

The two envelopes reuse the same wire-version `1`, exit-code authority
table, and bounded/redacted field limits as the product-quality
contract. The two release-gate concerns inherit the existing
`[ai] enabled = true` path: when AI evaluation is on, the
`ai-review` row is appended to the same `gate_runs` snapshot. Like
the product-quality profile, a project that picks `release` without
binding `commands.<id>` lands on `REVIEW_REQUIRED` via the existing
required + missing-command aggregate path; relax an individual
concern to `required = false` to opt out of one half.

## Project status

v0.1 through v0.6 are shipped. v1.1 (Engineering Gates), v1.2
(Product-quality Gate), and v1.3 (Release-evidence and
capability-conformance Gate) are also shipped: every change listed
in `ROADMAP.md` is implemented and archived, the full CLI surface
(`init`, `run`, `list`, `top`, `show`, `report --ai`, `gc`, `export`,
`doctor`, `check`, `gate`, `link`, `unlink`, `completions`, `man`,
`mcp`) is functional, the packaging suite is green (8/8 bash tests
including `agent_examples`, `gha_templates`, and `change_workflow`),
and CI enforces fmt, clippy (`-D warnings`, `--all-features`), MSRV
1.74, cargo-deny, tarpaulin coverage, macOS (`macos-14`) parity,
shellcheck, npm audit, and the end-to-end smoke test. The next
planning milestone is v1.0 (Stable); see `ROADMAP.md` for the
delivery sequence and `HANDOFF.md` for the current implementation
handoff.

## Development

The intended architecture is a single Rust binary with focused modules for CLI dispatch, process execution, generic fingerprint normalization, SQLite storage, reports, checker adapters, and heuristic correlation. Keep the core language-agnostic and local-first.

OpenSpec artifacts are validated with:

    openspec validate --changes --strict --no-interactive

See AGENTS.md for repository working rules.

## License

Licensed under the [MIT License](LICENSE). Copyright © 2026 lileililiwen.
