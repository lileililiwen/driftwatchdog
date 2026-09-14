## Approach
- Single reusable workflow file so there is exactly one blessed shape: `templates/github-actions/driftwatch-check.yml` using `workflow_call` plus `workflow_dispatch` inputs (`version`, default `latest`; `upload_report`, default true). Jobs run on `ubuntu-latest` with `permissions: {contents: read}`.
- Steps: (1) install via the pinned installer URL (`https://github.com/lileililiwen/driftwatchdog/releases/latest/download/install.sh` with `--version ${{ inputs.version }}`); (2) `driftwatch check` (allowed to report drift without failing the job — encode the exit-code contract explicitly: drift findings are informational unless `fail_on_drift: true`); (3) `if: always()` render `driftwatch report --ai > drift.md`; (4) `actions/upload-artifact` for `drift.md` only; (5) append `driftwatch top --limit 5` output to `$GITHUB_STEP_SUMMARY`.
- The `fail_on_drift` input (default false) decides whether a nonzero `check` fails the job; document that checker failures are isolated per checker by the CLI itself, so one broken checker never blocks the report.
- Shape test asserts anchors, not full YAML semantics: `on:` includes `workflow_call`, installer base URL equals `scripts/lib/release.sh` base, `report --ai` + `upload-artifact` + `GITHUB_STEP_SUMMARY` present, no `state.db`/`.driftwatch` upload, `fail_on_drift` documented in both files. Embedded `run:` blocks are extracted and shellchecked at severity error when the tool exists.

## Non-goals
- No matrix across driftwatch versions; no PR comments; no state caching.
