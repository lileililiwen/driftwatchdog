# Tasks: installer-stdout-purity

## 1. BFS: Impact

- [x] 1.1 Reproduce: capture the value of `installed` in the failing installer
      sub-suite and record the exact bytes, showing two lines rather than one.
- [x] 1.2 Record the baseline `cargo test` state, including the exact
      `packaging_bash_and_node_suites_pass` result and per-suite counts.
- [x] 1.3 Map every `rm` in the install path — `scripts/install.sh` plus the
      helpers it sources (`scripts/lib/platform.sh`, `scripts/lib/release.sh`) —
      and classify each as stdout-exposed or not. List `scripts/package.sh` and
      `scripts/smoke.sh` as swept-and-out-of-scope with the reason.
- [x] 1.4 Confirm which stdout writes in `install.sh` already carry `>&2` and
      which reach stdout, so the fix does not touch diagnostics.
- [x] 1.5 Confirm the failure is the installer's and not the test's: the same
      property holds when the captured value is taken from a chatty `rm`, so a
      stock host's green run is not evidence of correctness.

## 2. DFS: Implementation

- [ ] 2.1 Fix the exit-trap cleanup at `scripts/install.sh:274` so it cannot
      write to stdout, and make a non-zero cleanup status unable to replace the
      install's own exit status.
- [ ] 2.2 Fix the staged-binary cleanup at `scripts/install.sh:345` so it
      redirects stdout as well as stderr.
- [ ] 2.3 Re-sweep the install path and confirm no other delete can reach
      stdout; record the result in the handoff rather than asserting it silently.
- [ ] 2.4 Add `tests/packaging/test_installer_stdout.sh` with two scenarios that
      put a chatty `rm` (and a second-call-failing `mv` for the failure path) on
      the installer's `PATH`, so the defect is reproduced on every host.
- [ ] 2.5 Assert the stronger property in both scenarios: happy path is exactly
      one line equal to the installed path; failure path is empty stdout. Do not
      weaken the existing `test_installer.sh`.
- [ ] 2.6 Confirm the new test fails when the fix is reverted and passes with
      it, so it is a regression test rather than a restatement.

## 3. BFS: Regression and verification

- [ ] 3.1 `cargo test` — the full suite, no worse than the 1.2 baseline, with
      `packaging_bash_and_node_suites_pass` green and the exact counts recorded.
- [ ] 3.2 `cargo fmt --check` and
      `cargo clippy --all-targets --all-features -- -D warnings` clean.
- [ ] 3.3 `openspec validate installer-stdout-purity --strict` and
      `openspec validate --changes --strict` pass; `openspec list --changes
      --strict` resolves the change.
- [ ] 3.4 `sh tests/packaging.sh` green, including the new sub-suite, with the
      pre-existing ten still green.
- [ ] 3.5 `git diff --check` clean, and only the intended files are staged.
- [ ] 3.6 Re-check the untouched surfaces named as non-goals: installer stderr
      text, `--dry-run` output, exit codes, checksum verification, and atomic
      install behaviour.
