# Proposal: GitHub Actions templates

## Why
Teams that adopt driftwatch in CI today must assemble the workflow from scattered README sections: install the binary, run checkers, keep the AI report, and avoid leaking logs. A wrong assembly either runs checkers without the installer matrix, uploads raw logs as artifacts, or fails closed on `check` exit codes nobody documented. One tested template removes the guesswork and encodes the project's own conventions (pinned installer version, `check` failure isolation, `report --ai` as the shareable artifact).

## What Changes
- `templates/github-actions/driftwatch-check.yml` — reusable workflow: install pinned `driftwatchdog` via `scripts/install.sh`, run `driftwatch check`, always render `driftwatch report --ai > drift.md`, upload `drift.md` as an artifact, and append the top-bugs summary to `$GITHUB_STEP_SUMMARY`. Documents required inputs (`version`), permissions (`contents: read`), and the `check`-exit-code contract.
- `templates/github-actions/README.md` — copy/use instructions (`uses:` vs copy), input table, artifact retention note, and the explicit warning never to upload raw `.driftwatch/state.db` or full logs.
- Shape test (`tests/packaging/test_gha_templates.sh` style): required anchors present (`on:`, `jobs:`, installer URL matching the release base, `report --ai`, artifact upload, step-summary), no `state.db` upload step, embedded shell passes `shellcheck -S error` when available (soft-skip otherwise, matching the repo's no-new-hard-deps stance).
- README section linking the templates.

## Capabilities
### New Capabilities
- `github-actions-templates`: tested reusable CI template running checkers and publishing the AI report.
### Modified Capabilities
- None.

## Impact
Affects: `templates/` (new), `tests/packaging/test_gha_templates.sh` (new), `README.md`, `ROADMAP.md`, `HANDOFF.md`, `openspec/specs/github-actions-templates/spec.md` (new via archive).

## Dependencies
- No new-code dependency; implementable any time after `mcp-read-tools` archives (sequenced third to keep one change in flight). Uses only the stable CLI contract (`install.sh`, `check`, `report --ai`).

## Non-goals
- No publishing back to PRs/issues, no auto-fix, no cache of `.driftwatch/` between runs (state is per-run; history lives in the repo's own checked-in workflow runs only via the uploaded report).
- No new product features or CLI changes.
