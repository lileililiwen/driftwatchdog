# Driftwatch

Your AI fixed this bug three times already.

Driftwatch remembers it.

Driftwatch is a local-first, language-agnostic CLI for runtime failure memory in AI-assisted coding workflows. It records failed commands, recognizes recurring bugs, consumes external spec-checker results, and produces context that humans and coding agents can act on.

Rust is Driftwatch's implementation language—not a restriction on the projects it monitors. Run Rust, Python, C#, JavaScript/TypeScript, Go, Java, Flutter, or any other command-line tool through the same interface.

## Why

AI coding agents often rediscover the same failure because the project has no durable runtime memory. Driftwatch connects:

    runtime command -> failure memory -> recurring bug
                                  -> spec alerts -> possible relationship

It is not an APM platform, test framework, spec parser, CI server, code fixer, or built-in LLM.

## Quick start

    # Build/install once the Rust implementation is available
    cargo install --path .

    # Initialize local state
    driftwatch init

    # Wrap any project command
    driftwatch run cargo test
    driftwatch run pytest
    driftwatch run npm test

    # Inspect runtime memory
    driftwatch list
    driftwatch top
    driftwatch report
    driftwatch report --ai > drift.md  # AI-oriented context report

State is stored locally in .driftwatch/state.db. By default Driftwatch does not upload code, logs, or specs, does not use telemetry, and does not call an LLM API.

## Example

    $ driftwatch run cargo test

    FAIL

    Recurring bug detected:
    #12 database connection timeout
    Seen 6 times before.

After checker integration:

    driftwatch check
    driftwatch link bug:abcdef12 spec:42 --note "see issue #108"
    driftwatch report --ai > drift.md

The AI report contains recurring failures, current spec violations, possible heuristic relationships and manual links, recent commits, and instructions to investigate recurrence and add regression coverage. It deliberately does not claim that a similarity score proves root cause.

## Commands

    driftwatch init
    driftwatch run <command> [args...]
    driftwatch list [--limit N] [--failed] [--tag TAG]
    driftwatch top [--limit N] [--days N] [--tag TAG]
    driftwatch show <bug-id>
    driftwatch report [--ai] [--limit N] [--days N] [--tag TAG]
    driftwatch check [--only NAMES] [--dry-run]
    driftwatch link bug:<id> spec:<alert-id> [--note "..."]
    driftwatch unlink <link-id>
    driftwatch export json|jsonl|markdown
    driftwatch gc [--days N]
    driftwatch doctor

## External checkers

Driftwatch does not define a spec format. Configure existing local tools in driftwatch.toml:

    [[checkers]]
    name = "architecture"
    command = "my-spec-checker --json"

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

License selection is not yet finalized.
