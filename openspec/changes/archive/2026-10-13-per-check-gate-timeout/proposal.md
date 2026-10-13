# Proposal: Give every gate check its own declared timeout, and show it

## Why

`driftwatch gate` hardcodes one timeout for every check it runs:

- `src/commands/gate.rs:56` — `const CHECK_TIMEOUT_MS: u64 = 60_000;`
- `src/commands/gate.rs:196` — its only use, passed as `AdapterInput::timeout_ms`
  for the `sh -c <command>` the manifest bound.

No manifest can change it. A repository whose real test suite takes 143s gets
killed at 60s, and the gate records that kill as `REVIEW_REQUIRED` with
`<tool>:timeout` missing evidence. That is a *false claim about the repository*,
not a result: the check never got to finish.

### Measured against the live fleet

Standalone warm timings collected before this change (2026-10-04):

| repository | warm wall time | outcome under the 60s constant |
| --- | --- | --- |
| `cvunify` | 55s | passes, ~8% headroom (flaky) |
| `labrys` | 58.5s | passes, ~2% headroom (flaky) |
| `fotofy` UnitTests | 69s | **TIMEOUT** |
| `openaccounting` | 143s | **TIMEOUT** |
| `rust-ecommerce` | ~10 min | **TIMEOUT** |
| `alltools-platform` | >15 min | **TIMEOUT** |

Two different defects hide behind one constant:

1. **Four repositories cannot pass** no matter what they do. Their commands are
   killed mid-run and reported as unproven.
2. **Two repositories are one slow machine away from a false red.** `cvunify`
   (8% headroom) and `labrys` (2% headroom) pass today only because the
   machine was warm. Neither has margin.

A single larger global constant fixes neither well. Raising 60s to 900s makes
defect 1 go away and makes defect 2 *worse* in relative terms: every check now
waits up to 15 minutes, and the two near-miss repositories still have no
guaranteed margin, just a wider one they do not need. A timeout that is wrong
for every check is not a default, it is a missing field.

## What

1. **A per-check `timeout_ms` in the manifest** (`ManifestCheck.timeout_ms`,
   `Option<u64>`). Absent → the existing 60s default, so all 76 manifests that
   carry a `.ai-gate/gate.yaml` (and every native `gate.toml`) keep their
   current behaviour with **zero edits**.
2. **The same semantics the checker config already has**, not a new invention:
   `src/project/config.rs:81` declares `pub timeout_ms: Option<u64>`,
   `src/checker/runner.rs:53` resolves it as
   `entry.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS)`, and
   `src/project/config.rs:239` already refuses `timeout_ms = 0` with an
   actionable message.
3. **A hard cap, `MAX_CHECK_TIMEOUT_MS = 3_600_000` (1h)**, enforced at
   validation with a named error, mirroring `MAX_MAX_LINES`
   (`src/gate/manifest.rs:562`). Without a bound, "raise it to 24h" becomes the
   new footgun and a hung command holds the machine for a work session.
4. **The resolved timeout is visible.** `ResolvedGatePlan` carries
   `check_timeouts`, and `--dry-run` renders it. A timeout nobody can see is
   indistinguishable from a slow test — which is why this defect stayed
   invisible across 76 manifests. Same principle as ruling D18 applied to time.
5. **Both manifest forms accept the key.** The `.ai-gate/gate.yaml` converter
   gains a per-check `timeouts:` mapping that produces the *same*
   `ManifestCheck.timeout_ms` field, so the rule runs once in
   `manifest::validate` for both forms — the single-owner pattern the YAML
   module's own header already states for `not_applicable` and
   `binding_policy`.

## Non-goals

- **Not a fleet sweep.** This change adds the field; it does not edit the 76
  manifests to use it. Which timeout a repository needs is that repository's
  call, measured on its own machine.
- **Not a change to the default.** 60s stays 60s for every check that declares
  nothing, deliberately: a default that changes silently is a default nobody
  reviewed.
- **Not a new status.** A timeout already reports distinctly from a non-zero
  exit (`REVIEW_REQUIRED` + `project-runtime:timeout` missing evidence vs
  `FAIL`); this change changes the deadline, not the mapping. See `design.md` §6.
- No archive. The change stays active until separately authorized.
