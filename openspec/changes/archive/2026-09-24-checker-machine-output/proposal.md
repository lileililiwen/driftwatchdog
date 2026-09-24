# Proposal: Machine-readable output for driftwatch check

## Why

`driftwatch gate` already prints a machine-readable status document, but
`driftwatch check` — the compatibility entry point for checker-only
projects — only writes human text. Control-plane consumers (notably
Forge's policy adapter, which must drive checker-only projects that have
no `gate.toml`) need one stable JSON document describing per-checker
outcomes and the alerts each produced. Without it, every consumer must
re-parse human text or reach into `.driftwatch/` storage, which the core
keeps private.

## What Changes

- Add `--format human|json` to `driftwatch check` (human stays the
  default and the byte-for-byte existing behavior).
- Emit a versioned checker-report envelope on stdout: contract id
  `driftwatch-checker/0.1.0`, tool name/version, per-checker rows
  (`name`, `status`, `alerts[]` in the existing alerts vocabulary, bounded
  `error` note) and a summary of counts.
- Compose with `--dry-run`: `check --dry-run --format json` prints the
  same document while persisting nothing.
- Keep exit-status semantics unchanged for the flag value `json` versus
  human (same success/failure mapping), so adding the flag cannot
  silently change CI gate results.
- Document the envelope in `README.md` (External checkers section) and the
  ROADMAP planning queue.

## BFS Impact Map

- **Capabilities:** `config-checker-protocol` (output contract);
  `gate-cli-and-memory-integration` untouched (its `gate --format json`
  already exists).
- **Users and flows:** CI template unchanged; agents and control planes
  consume structured check results; existing human users see no diff.
- **Contracts/data/persistence:** no schema change; JSON is a projection
  of the same results that persistence records today; `--dry-run`
  persistence rules unchanged.
- **Integrations/configuration:** no new config keys; checker execution,
  isolation of broken checkers and timeouts behave identically.
- **Callers:** CLI dispatch, report rendering layer; storage untouched.
- **Failure/boundary behavior:** a checker that fails stays isolated and
  appears with its error note; stdout carries only the document; malformed
  checker output is still a protocol-error row, not a crash.
- **Tests:** snapshot tests per format, dry-run+json persistence-absence
  test, unknown-field tolerance note, exit-code parity test.
- **Dependencies:** none; enables the Forge `driftwatch-cli-alignment`
  consumption.
- **Compatibility/security/privacy:** default behavior preserved; no
  network; no secrets beyond what alerts already contain.

## Capabilities

- `config-checker-protocol`: check results gain a stable, versioned,
  machine-readable projection without changing execution semantics.

## Non-goals

- Changing checker config, working-dir jailing, alert storage or
  correlation behavior.
- Adding JSON output to other subcommands (`export` already emits JSON).
- A schema registry beyond the embedded `contract` version field.

Source: consumer need from the Forge workspace (requirement.md §24, §32);
Driftwatchdog brief boundaries unchanged.
