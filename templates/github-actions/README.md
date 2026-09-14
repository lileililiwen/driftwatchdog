# Driftwatch GitHub Actions template

Reusable CI workflow that installs a pinned `driftwatchdog`, runs
the configured external checkers, and uploads the AI-context
report as a single artifact. Raw state (`.driftwatch/state.db`,
command logs) is deliberately never published.

The template lives at `templates/github-actions/driftwatch-check.yml`
and is a single, blessed shape — copy it, or reuse it via
`workflow_call`.

## Inputs

| Name             | Type    | Default     | Purpose |
| ---------------- | ------- | ----------- | ------- |
| `version`        | string  | `latest`    | Driftwatch version to install. Any `vX.Y.Z` or `X.Y.Z` is accepted by the installer. |
| `fail_on_drift`  | boolean | `false`     | When `true`, a non-zero `driftwatch check` exit code fails the job. The AI report still uploads. |
| `upload_report`  | boolean | `true`      | When `true`, `drift.md` is uploaded as the `drift-report` artifact. |

## Required permission

`contents: read`. The workflow never writes to the repository and
never uploads state.

## Two ways to use it

### 1. Reuse via `workflow_call`

Add a thin caller workflow in your repository's
`.github/workflows/` directory:

```yaml
name: driftwatch

on:
  push:
    branches: [main]
  pull_request:

jobs:
  driftwatch:
    uses: lileililiwen/driftwatchdog/.github/workflows/driftwatch-check.yml@main
    with:
      version: v0.6.0
      fail_on_drift: false
    permissions:
      contents: read
```

`lileililiwen/driftwatchdog` resolves to the GitHub repository
hosting the template; `@main` pins the caller to the current
default branch. Pin to a tag (`@v0.6.0`) when reproducibility
matters.

### 2. Copy into your repository

Copy `templates/github-actions/driftwatch-check.yml` into
`.github/workflows/driftwatch-check.yml` in your own repository
and edit the file directly. Use this path when you need to add
project-specific steps before or after the Driftwatch block.

## Exit-code contract

`driftwatch check` may exit non-zero for two reasons:

1. **A configured checker found drift** (alerts in the JSON
   document). Drift findings are informational by default; the
   AI report still uploads and the top bugs still land in the
   step summary.
2. **A checker errored** (e.g. malformed output, process exit
   code, timeout). Checker failures are isolated per checker by
   the CLI itself, so one broken checker never blocks the
   others or the report.

Set `fail_on_drift: true` to fail the job on either condition
after the report has uploaded. Keep it `false` for advisory
visibility.

## What is published

| Artifact        | Source     | When |
| --------------- | ---------- | ---- |
| `drift-report`  | `drift.md` (the bytes `driftwatch report --ai` writes) | Every run when `upload_report` is true. |
| Step summary    | `driftwatch top --limit 5` Markdown table | Every run. |

**Never published:** `.driftwatch/state.db`, captured stdout or
stderr, or any path under `.driftwatch/`. The CI suite asserts
this contract; see `tests/packaging/test_gha_templates.sh`.

## Verifying the template

The bash packaging suite ships a shape test that asserts:

1. The template declares `workflow_call` and `workflow_dispatch`
   triggers.
2. The installer base URL equals the one in
   `scripts/lib/release.sh` (the shared source of truth).
3. `driftwatch check` runs and the report is always rendered.
4. The upload step targets only `drift.md` (no `state.db` or
   `.driftwatch/` paths).
5. `fail_on_drift` is documented in both files.
6. Embedded `run:` blocks pass `shellcheck -S error` when the
   tool is available (soft-skip otherwise).

Run it locally with:

    sh tests/packaging.sh
