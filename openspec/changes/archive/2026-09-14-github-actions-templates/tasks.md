## 1. Template + docs
- [x] 1.1 Add `templates/github-actions/driftwatch-check.yml`: `workflow_call` + `dispatch` inputs (`version`, `fail_on_drift=false`, `upload_report=true`), pinned installer install, `check`, always-render `report --ai > drift.md`, upload `drift.md` only, `top` into `$GITHUB_STEP_SUMMARY`, `contents: read`.
- [x] 1.2 Add `templates/github-actions/README.md`: copy-vs-`uses:` instructions, input table, exit-code contract, never-upload-`state.db` warning.

## 2. Shape test
- [x] 2.1 Add packaging-style test: required anchors (`workflow_call`, installer base == `release.sh` base, `report --ai`, upload-artifact, step summary), no `state.db` upload, `fail_on_drift` documented, embedded shell passes shellcheck errors when available. Wire into `tests/packaging.sh` discovery.

## 3. Docs
- [x] 3.1 Link the templates from README (CI section); record in HANDOFF module map.

## 4. Change completion workflow (HANDOFF.md routine — repeat for this change)
- [x] 4.1 Run verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
- [x] 4.2 Confirm `openspec/changes/github-actions-templates/tasks.md` has every box ticked.
- [x] 4.3 Archive with `openspec archive github-actions-templates -y` (omit `--skip-specs`: adds `github-actions-templates` capability).
- [x] 4.4 Stage and commit implementation + archive: `git add -A && git commit -m "Implement github-actions-templates"`.
- [x] 4.5 Update `HANDOFF.md` (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after github-actions-templates"`.
- [x] 4.6 Re-run `openspec validate --changes --strict --no-interactive` to confirm the change list still resolves.
