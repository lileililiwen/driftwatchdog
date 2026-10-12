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

Driftwatchdog owns the central workspace Gate: 76 workspace
`.ai-gate/gate.yaml` files name `runtime: driftwatchdog`, and 70 of
them declare no `commands:` block at all. The catalog
below ties each built-in profile to its stable concern IDs, the
default `required = true` flag, the exit-code authority rule, and
whether a text-mode fallback applies. Concern IDs and profile names
match `src/gate/concerns.rs`; the code is authoritative — if a name
here ever disagrees with the code, the code wins and the doc is
corrected.

| Built-in or workspace profile | Stable concern IDs scheduled | Default | Exit-code authority | Text-mode fallback |
| --- | --- | --- | --- | --- |
| `minimal` | `governance-quality` | required | code-scoped governance report | **no** (a report it cannot read must not pass silently) |
| `backend` | `a11y`, `api-contract`, `governance-quality`, `migration`, `responsive`, `secret-scan` | required | governance report for `governance-quality`; as above for project-runtime checks | yes for project-runtime checks; no for governance quality |
| `frontend` | `a11y`, `governance-quality`, `responsive`, `secret-scan` | required | governance report for `governance-quality`; as above for project-runtime checks | yes for project-runtime checks; no for governance quality |
| `full` | `a11y`, `api-contract`, `governance-quality`, `migration`, `responsive`, `secret-scan` | required | governance report for `governance-quality`; as above for project-runtime checks | yes for project-runtime checks; no for governance quality |
| `product` (alias `rust-product`) | `governance-quality`, `placeholder-threshold`, `product-code-boundary`, `source-file-size` (built-in, no command) | required | envelope wire version `1` for the two project-owned concerns; in-process normalized result for the built-in scanner; **code-scoped report** for `governance-quality`, whose status comes from the matching findings' levels and not from the checker's exit code | yes for the two project-owned concerns; **no** for `governance-quality` (a report it cannot read must not pass silently); n/a for the built-in scanner |
| `release` | `capability-conformance`, `governance-quality`, `release-evidence` | required | governance report for `governance-quality`; as above for release envelopes | no for governance quality; no for release envelopes |
| `deployable` | `ci-contract`, `compose-contract`, `governance-quality` | required | governance report for `governance-quality`; project-runtime exit status for the two project checks | no for governance quality; yes for project checks |
| Workspace profiles `typescript-product`, `python-product`, `dotnet-product`, `dotnet-library`, `typescript-monorepo`, `rust-platform`, `tooling`, `deployment`, `inventory`, `governance` | `governance-quality` | required | code-scoped governance report | **no** (a report it cannot read must not pass silently) |

Every built-in and supported workspace profile schedules `governance-quality`
as a required check. Project-defined profiles keep their declared concerns
and also receive that required check. Unknown profile names remain errors.
Other built-in concerns can be adjusted per concern with `required = false`
(or the `checks:` map `false` / `"optional"` in `.ai-gate/gate.yaml`).

When a required check binds no `command`, nothing ran, so the result
is `UNBOUND` with `Error` severity and the gate **fails** — a
required absence is never `NOT_APPLICABLE`, and it blocks regardless
of the manifest's `blocking:` list. Two manifest-level declarations
make that absence explicit instead:

- `not_applicable.<id> = "<reason>"` — a written exemption for a
  check the project's stack genuinely has no implementation for. The
  check stays scheduled and is reported as an acknowledged exemption;
  it does not block. The reason is length-bounded and credential-
  redacted. Exemptions are **budgeted**: one project may declare at most
  `min(floor(0.25 × required_checks), 5)` of them, computed from its own
  manifest, and every declaration spends one — including one on an
  optional check. At the budget the run records a warning that the
  headroom is exhausted; over it the run **fails**. `build`, `tests`,
  `security`, `repository`, `governance-quality`, and
  `declared-verification` are never exempable at any budget, so an
  exemption naming one of them fails the run
  outright. The spine is matched
  by concern, not by spelling: this workspace also spells
  repository-integrity as `repository_integrity`, and that id is protected
  exactly as `repository` is. A project that genuinely needs more must bind
  a command or be reviewed.
- `binding_policy = "report" | "reject"` — `"report"` (the default)
  records the result as `UNBOUND`; `"reject"` refuses to resolve at
  all, naming every offending check in one refusal before any check
  executes and before any run is persisted.

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

For a deployable project, copy the repository-relative starter files under
[`templates/deployable/`](templates/deployable/). The `deployable` profile
requires both a project-owned Compose validation command and a repeatable
local CI command. The supplied workflow repeats the local command and
`driftwatch gate`; remote branch protection must separately require that CI
check. Template presence is bootstrap scaffolding, not runtime or deployment
evidence.
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
    driftwatchdog gate [--dry-run] [--format human|json|platform]
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
    driftwatch gate --format platform  # platform.gate-result 1.0.0 document; refuses rather than coerces
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

### Emitting a `platform.gate-result` document

`driftwatch gate --format platform` prints the same run in the
workspace's shared `platform.gate-result` contract at **major 1.0.0**,
so a portfolio consumer can read "may this deploy proceed?" without
scraping the human table or hard-coding driftwatchdog's vocabulary. The
document declares the version it claims in a root `contract` field
(`platform.gate-result/1.0.0`), the same shape `driftwatch check
--format json` uses for `driftwatch-checker/0.1.0`.

The two vocabularies now have the same six values, and every status maps
onto one of them:

| `GateStatus` | platform | claim |
|---|---|---|
| `PASS` | `passed` | ran and passed |
| `FAIL` | `failed` | ran and reported a failure |
| `NOT_APPLICABLE` | `skipped` | a proven exemption |
| `REVIEW_REQUIRED` | `review_required` | a human decision is still open |
| `UNBOUND` | `unbound` | asserted, no command bound, nothing ran |
| — | `errored` | never emitted; every execution failure is already a `REVIEW_REQUIRED` |

`review_required` and `unbound` are not approximations. The contract
defines them with exactly the claims this Gate makes and forbids merging
them: *"A consumer MUST NOT read `skipped` and `unbound` alike, and MUST
NOT read `review_required` as a decision already taken."* Optionality
still changes whether a status blocks and never changes what it claims,
so an optional unbound check is emitted as `unbound` under a `passed`
root result rather than refused.

Identifiers and lengths are where refusal still lives. A manifest check
id that does not match the platform's `^[a-z][a-z0-9_.-]{0,63}$` is
refused, not slugged — no reversible slug fits inside 64 characters, and
lowering `L1-configuration` would merge it with `l1-configuration`.
Conforming ids pass through unchanged and are repeated verbatim in the
check's `driftwatch_gate_id`. An over-length diagnostic or evidence
reference is refused rather than truncated, and the profile is preserved
verbatim in `driftwatch_profile`. On refusal the emitter writes
**nothing** to stdout, prints one diagnostic naming the exact value and
the constraint, and exits `2`; the gate run itself is still executed and
still persisted.

    driftwatch gate --format platform > gate-result.json   # exits 2 and prints nothing when unrepresentable

**Known gap.** This Gate bounds a diagnostic at 2,048 bytes plus an
ellipsis (2,049 characters) and the contract bounds `detail` at 512, so a
run whose build or test output is longer refuses on `detail` alone. That
is measured, not hypothetical: it is the reason `--format platform`
currently refuses on this repository and on `forge`. Resolving it needs a
contract decision (carry the long text in an additional property, or
raise `detail.maxLength`) and is deliberately not decided here.

Validation against the contract belongs to the consumer, which already
vendors the schema; driftwatchdog asserts the constraints it depends on
and refuses rather than validating its own output.

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
timeouts:                         # optional; per-check deadline in ms
  tests: 600000                   # a suite that needs longer than 60s
blocking: [FAIL, REVIEW_REQUIRED]
project_commands:
  lint: "npm run lint"
contexts: [git, openspec
governance_quality:               # optional; narrows a code scope, never widens it
  codes: [RULES_DRIFT_NEW]
# A product-profile project also schedules `governance-quality`. It has
# no default command: the governance checker lives in a sibling
# repository, so each project binds its own under `commands:` --
#   commands:
#     governance-quality: "python3 ../workspace-governance/scripts/workspace_check.py --json"
# Leaving it unbound records `UNBOUND` and fails the run.
```

A `profile` that is not a built-in name selects exactly its declared
`checks`; a built-in profile keeps its default set, adjustable through
`checks` (`false` removes, `"optional"` schedules non-blocking). The
same domain profiles are available in `gate.toml` via a
`[profiles.<name>]` table listing the concerns the profile selects.

A check declares its own execution deadline with `timeout_ms` — `timeouts:`
in the YAML form, `timeout_ms = <ms>` on the `[[checks]]` entry in
`gate.toml`. A check that declares nothing runs under the 60s default,
so existing manifests keep their current behaviour untouched. The value
must be positive and at most 3600000 (1h); both bounds are refused at
parse time, before any check runs, and the error names the offending
check. `driftwatch gate --dry-run` prints the effective deadline of
every scheduled check so the bound is readable before anything runs. A
check killed by its deadline reports `REVIEW_REQUIRED` with
`project-runtime:timeout` missing evidence — distinct from the `FAIL` a
non-zero exit produces, so "ran out of time" never reads as "failed".

A check with no entry under `commands:` is `UNBOUND` and fails the run.
That includes `governance-quality`, which the `product` profile
schedules but cannot default: its checker lives in a sibling repository,
so there is no path to bind for it, and binding one is a decision each
project makes for itself. See
[Governance-quality Gate concern](#governance-quality-gate-concern).

Local verification comes first: a change that has not passed its local
Gate must not be represented as complete merely because CI is
configured. CI templates repeat the same command as a second layer.

### Product-quality Gate contract

The `product` and `rust-product` built-in profiles schedule the
product-quality concerns — `product-code-boundary`,
`placeholder-threshold`, the built-in `source-file-size`, and
`governance-quality` (documented on its own below, because its
wire shape and status mapping differ). The profiles are aliases:
`rust-product` exists for projects that want a Rust-flavored name
without changing the concern set. All four are required by default;
relax an individual concern to `required = false`, or remove it with
`enabled = false`, when a project cannot meet it yet.

`product-code-boundary` and `placeholder-threshold` are data only:
Driftwatchdog never embeds a language scanner, runs a provider SDK, or
installs a tool for them. The project binds a command to each concern in
`gate.toml` (or `.ai-gate/gate.yaml`). `source-file-size` is different:
it is executed in-process by Driftwatchdog and needs no project command
(see [Source-file-size Gate concern](#source-file-size-gate-concern)).

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

### Governance-quality Gate concern

`governance-quality` makes a **code-scoped** slice of an external
governance check reportable as a Gate concern. The governance checks
live outside this repository; the concern binds no checker path of its
own, so a project that does not bind a command for it records
`UNBOUND` and fails, exactly like any other unbound required check.

The report is project-owned data, not a Gate envelope: a top-level JSON
object with `findings` and `summary`, where each finding carries a
`code`, a `level` (`ERROR` or `WARNING`), a `message`, and a
`project`.

```yaml
# .ai-gate/gate.yaml
checks:
  governance-quality: true
commands:
  governance-quality: "python3 ../workspace-governance/scripts/workspace_check.py --json"
```

```toml
# gate.toml
[[checks]]
id = "governance-quality"
command = "python3 ../workspace-governance/scripts/workspace_check.py --json"
```

#### Envelope mapping

The status comes from the **matching** findings' levels, never from the
checker's exit code: the checker reports the whole portfolio audit, so a
run that exits `1` on a backlog this concern does not own must still
read `PASS` here. The checker's exit status is recorded in the
diagnostic, and `0`/`1` are its two expected values.

| Matching findings | Status | Exit |
| --- | --- | --- |
| none | `PASS` | 0 |
| one or more `WARNING` | `REVIEW_REQUIRED` | 2 |
| one or more `ERROR` | `FAIL` | 1 |

`ERROR` dominates a mixed run. The reported finding count is the
**filtered** count; the checker's total is quoted beside it in the
diagnostic and is never substituted for it.

#### Failure path

No case in which the checker could not produce a trustworthy answer
reports `PASS`.

| Condition | Status | Exit |
| --- | --- | --- |
| the bound command could not start (missing checker, or the shell's not-found status) | `UNBOUND` | 1 |
| an exit status other than `0`/`1` and the shell's not-found status | `REVIEW_REQUIRED` | 2 |
| unparseable or truncated report | `REVIEW_REQUIRED` | 2 |
| timeout | `REVIEW_REQUIRED` | 2 |
| a matching finding with a level the concern does not define | `REVIEW_REQUIRED` | 2 |

`UNBOUND` is reserved for "nothing ran". A checker that started and
could not finish is a review condition, and the two are never collapsed
— collapsing them would let a broken checker present as a missing one.

#### Scope versus exemption

The concern declares which finding codes it owns, and that set is the
ceiling:

| Code | Meaning |
| --- | --- |
| `QUALITY_README_CLAIM_CONTRADICTED` | a README claim is contradicted by the repository |
| `QUALITY_README_CLAIM_UNSCOPED` | a README claim is not scoped to what it covers |
| `RULES_DRIFT_NEW` | a new clause gap against the rule baseline |
| `RULES_DRIFT_BASELINE_STALE` | the rule baseline itself is stale |
| `SPEC_PURPOSE_PLACEHOLDER` | a spec's `## Purpose` section is still the placeholder |

A declared code is a code the concern **owns**, not one it has necessarily
seen. `SPEC_PURPOSE_PLACEHOLDER` is declared ahead of the producing checker
shipping it, so the concern can report the finding the moment it exists
instead of filtering it out. Until the checker emits it, it contributes no
finding and no mention in the result — the status is derived only from
findings actually observed.

A project **may narrow** the set for its run:

```toml
[governance_quality]
codes = ["RULES_DRIFT_NEW"]
```

A project **must not widen** it: a code the concern does not declare is
refused as `REVIEW_REQUIRED`, naming the code, before any command runs.
An **empty** set while the concern is enabled is also `REVIEW_REQUIRED`,
naming the empty scope — never `PASS`, because a scope reporting
nothing is indistinguishable from a silently disabled check. (The same
block works in `.ai-gate/gate.yaml` as
`governance_quality: {codes: [RULES_DRIFT_NEW]}`.)

Scope is not exemption. A scope declares *what the concern looks at*; an
exemption (`not_applicable.<id>`) *removes the check from the gate*.
`governance-quality` is never exempable at any budget, because an
exemption here would remove the report of a contradicted claim and leave
nothing behind. Narrowing a scope is a recorded decision about the codes
in force; it cannot be used to silence a code the concern owns.

#### Where this is live

The concern is **wired and live in this repository**, where it is
`UNBOUND` — this repository binds no `governance-quality` command, so that
check is `UNBOUND` and the run fails. That is the honest result, and the
concern's behaviour is proved by
`tests/gate_governance_quality.rs` and the unit tests in
`src/gate/governance/` rather than by this repository's own run. The
`declared-verification` concern, by contrast, **is** bound here and does run.

The other **70 workspace repositories** gain the concern only when
`gate-unbound-check-fails` task 5.1 lands (it makes unbound required
checks fail across the fleet). Until then the concern is live here and
inert elsewhere. The 76 repositories that select a product profile
without binding `commands.governance-quality` report `UNBOUND` in the
meantime — a real result, reported as one.

### Declared-verification Gate concern

`declared-verification` runs a repository's **own declared** verification
command. A repository records how it should be verified in its
`.project.json`, under `verification.command`; this concern is where that
record becomes a gate result rather than documentation. Ruling D18 in this
workspace: *a verifier that no gate runs does not gate.*

```yaml
checks:
  declared-verification: true
commands:
  declared-verification: 'cargo test --workspace'
```

Both halves are required. A `commands.<id>` naming a check that is not
selected is a parse error, so a binding with no selection would never run.

The concern is deliberately in **no** closed concern vocabulary, so the Gate
routes it to the project-runtime **text** adapter and the exit code is the
verdict: `0` is `PASS`, a non-zero exit is `FAIL`, and a timeout or a failed
spawn is `REVIEW_REQUIRED` with missing evidence. It is not the
`governance-quality` concern and deliberately cannot be: that concern requires
its command to emit a governance report and derives status from the *levels of
matching findings*, so binding ordinary test output there would produce
`REVIEW_REQUIRED` about the checker rather than `FAIL` about the command.

**The binding is never templated.** A profile cannot know a sibling
repository's verification command, and `profile_defaults` selects concern ids
only and never invents a command. The value is generated per repository from
that repository's own declaration by
`workspace-governance/scripts/bind_verification_command.py`, which refuses
rather than invents: no `.project.json`, an unparseable one, or no non-empty
string `command` is a named refusal, and a second run over an already-bound
repository writes nothing. The concern is **non-exemptable** — a repository
cannot write itself out of the ruling with a `not_applicable` reason — and no
built-in profile schedules it, because a profile that did would make every
un-generated repository `UNBOUND` and say nothing about its verification.

### Source-file-size Gate concern

`source-file-size` is a **built-in** concern: the `product` and
`rust-product` profiles schedule it by default, and Driftwatchdog scans
the repository in-process without a `commands.source-file-size`
binding. It counts each `0x0A` byte in every included file — exactly the
observable `wc -l` rule, so a final line without a trailing newline does
not increment the count. It does not remove blank lines, comments, or
strings, and it never parses a language.

By default a file must contain at most **1,000** physical lines. A
required Gate **fails** when any included file exceeds the maximum; the
finding names the repository-relative path, the counted lines, and the
limit:

```text
title    = "<relative path> has <actual> physical lines; maximum is <limit>"
severity = error
location = <relative path>
rule     = "source-file-size"
```

The scanner resolves a repository-owned source boundary:

* In a Git repository, candidates come from
  `git ls-files --cached --others --exclude-standard`, so `.gitignore`
  excludes untracked dependency/build content while tracked files stay
  visible for review.
* In a non-Git directory, Driftwatchdog walks the project root without
  following symlinks and applies a root `.gitignore` when present.
* Dependency, vendor, build, state, coverage, generated, and test
  directories (`node_modules`, `target`, `vendor`, `dist`, `build`,
  `coverage`, `test`, `tests`, …) are excluded by default.
* With no explicit `include`, conventional source roots (`src`, `app`,
  `lib`, `bin`, `cmd`, `internal`, `packages`, `server`, `client`) are
  discovered. Root READMEs, lockfiles, and arbitrary metadata are not
  source findings.
* Symlinks are not followed; binary or invalid-UTF-8 files (NUL bytes)
  are skipped as non-source and never fail the Gate.
* A Git-boundary, ignore-evaluation, path, or candidate-read failure
  returns `REVIEW_REQUIRED` with bounded missing-evidence fields. When a
  required concern blocks, incomplete coverage never becomes a pass.

Tighten or relax the boundary and threshold with a top-level
`[source_size]` table in `gate.toml`:

```toml
version = 1
profile = "product"

[source_size]
max_lines = 800                          # positive, <= 1_000_000
include = ["packages/service/**"]        # repository-relative globs
exclude = ["packages/service/legacy/**"] # always wins over include
```

`include` and `exclude` use `/`-separated repository-relative globs;
absolute patterns, `..` traversal, and empty patterns are rejected before
execution. A policy change (`max_lines`, `include`, or `exclude`) changes
the manifest digest. The `.ai-gate/gate.yaml` form uses the built-in
default policy; the native `[source_size]` table is the custom-boundary
surface.

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
binding `commands.<id>` has a required unbound check, which fails the
gate as `UNBOUND`; relax an individual concern to `required = false`,
declare a `not_applicable` exemption with a reason, or bind the
command.

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
