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

The installer refuses to run as root unless you pass `--allow-root`, refuses to install over an existing binary if checksum verification fails, and exits non-zero with the detected OS/architecture on unsupported hosts.

### npm (Linux, macOS)

The `driftwatchdog` npm package is a dependency-free launcher: it detects the host, downloads and verifies the matching native archive into a versioned cache, and forwards all arguments and the exit status to the native binary.

    npm i -g driftwatchdog
    driftwatch init      # the launcher exposes the binary as `driftwatch`

    # or, after install, run the same commands:
    driftwatch list
    driftwatch report --ai

The launcher reports unsupported hosts with the supported target list and points at the shell installer and direct downloads as alternatives. It does not execute an unverified binary and does not leave a failed download in the cache.

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
    driftwatchdog link bug:<id> spec:<alert-id> [--note "..."]
    driftwatchdog unlink <link-id>
    driftwatchdog export json|jsonl|markdown
    driftwatchdog gc [--days N]
    driftwatchdog doctor

## External checkers

Driftwatchdog does not define a spec format. Configure existing local tools in driftwatch.toml
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
