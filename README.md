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

## External checkers

Driftwatchdog does not define a spec format. Copy `driftwatch.toml.example`
to `driftwatch.toml` and run `driftwatch check` — the bundled stub checker
succeeds without edits:

    cp driftwatch.toml.example driftwatch.toml
    driftwatch check --dry-run
    driftwatch check

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

## Project status

The repository currently contains the implementation-ready OpenSpec roadmap and change packages. Read ROADMAP.md for the delivery sequence and HANDOFF.md for the current implementation handoff.

## Development

The intended architecture is a single Rust binary with focused modules for CLI dispatch, process execution, generic fingerprint normalization, SQLite storage, reports, checker adapters, and heuristic correlation. Keep the core language-agnostic and local-first.

OpenSpec artifacts are validated with:

    openspec validate --changes --strict --no-interactive

See AGENTS.md for repository working rules.

## License

Licensed under the [MIT License](LICENSE). Copyright © 2026 lileililiwen.
