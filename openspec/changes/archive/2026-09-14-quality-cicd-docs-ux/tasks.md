## 1. Quality policy
- [x] 1.1 Add `[lints]` + `rust-version`/MSRV + dependency update policy (`toml`, `rusqlite`, `thiserror` review); add `deny.toml` + coverage + proptest/fuzz seeds for normalizer.
- [x] 1.2 Fix `scripts/lib/*.sh` mode inconsistency or document sourcing rule with CI enforcement.

## 2. CI breadth
- [x] 2.1 Add `--all-features` to clippy (parity with AGENTS.md/HANDOFF.md); add macOS/arm64 matrix.
- [x] 2.2 Add shellcheck, `npm audit`, smoke (`scripts/smoke.sh`) jobs.

## 3. Release integrity
- [x] 3.1 Add `id-token: write` to npm-publish; refresh `macos-13` runner; assert tag==Cargo==npm single version source.
- [x] 3.2 Harden auto-tag (missing-PAT skip, no tag-spam, cold-start bump fix); add Dependabot; attach SBOM; start CHANGELOG.md.

## 4. Docs + UX
- [x] 4.1 Fix binary-naming docs (README install line + npm section + `Cargo.toml`/`cli.rs` alignment note); fill `release-distribution` Purpose; list `ci-test-gates` in ROADMAP; sync HANDOFF 5-step vs AGENTS 6-step; add `driftwatch.toml.example` + copy-paste checker example.
- [x] 4.2 Rich `--help` (long_about + examples), completions/man generation; single-wrap errors with `hint`; honest doctor severities (`INFO` for unconfigured); installer `--dry-run` docs + `cargo install` fallback; npm darwin-arm64 preflight guard.
- [x] 4.3 Remove stray `.driftwatch/` + placeholder `driftwatch.toml` from repo root; add doctor guard or `.gitignore` test.

## 5. Change completion workflow (HANDOFF.md routine — repeat for this change)
- [x] 5.1 Run verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
- [x] 5.2 Confirm `openspec/changes/quality-cicd-docs-ux/tasks.md` has every box ticked.
- [x] 5.3 Archive with `openspec archive quality-cicd-docs-ux -y` (omit `--skip-specs`: adds `quality-cicd-docs-ux` capability + modifies `ci-test-gates`/`release-distribution`).
- [ ] 5.3 Archive with `openspec archive quality-cicd-docs-ux -y` (omit `--skip-specs`: adds `quality-cicd-docs-ux` capability + modifies `ci-test-gates`/`release-distribution`).
- [ ] 5.4 Stage and commit implementation + archive: `git add -A && git commit -m "Implement quality-cicd-docs-ux"`.
- [ ] 5.5 Update `HANDOFF.md` (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after quality-cicd-docs-ux"`.
- [ ] 5.6 Re-run `openspec validate --changes --strict --no-interactive` to confirm the change list still resolves.
