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

- [x] 2.1 Fix the exit-trap cleanup at `scripts/install.sh:274` so it cannot
      write to stdout, and make a non-zero cleanup status unable to replace the
      install's own exit status.
- [x] 2.2 Fix the staged-binary cleanup at `scripts/install.sh:345` so it
      redirects stdout as well as stderr.
- [x] 2.3 Re-sweep the install path and confirm no other delete can reach
      stdout; record the result in the handoff rather than asserting it silently.
- [x] 2.4 Add `tests/packaging/test_installer_stdout.sh` with two scenarios that
      put a chatty `rm` (and a second-call-failing `mv` for the failure path) on
      the installer's `PATH`, so the defect is reproduced on every host.
- [x] 2.5 Assert the stronger property in both scenarios: happy path is exactly
      one line equal to the installed path; failure path is empty stdout. Do not
      weaken the existing `test_installer.sh`.
- [x] 2.6 Confirm the new test fails when the fix is reverted and passes with
      it, so it is a regression test rather than a restatement.

## 3. BFS: Regression and verification

- [x] 3.1 `cargo test` — the full suite, no worse than the 1.2 baseline, with
      `packaging_bash_and_node_suites_pass` green and the exact counts recorded.
- [x] 3.2 `cargo fmt --check` and
      `cargo clippy --all-targets --all-features -- -D warnings` clean.
- [x] 3.3 `openspec validate installer-stdout-purity --strict` and
      `openspec validate --changes --strict` pass; `openspec list --changes
      --strict` resolves the change.
- [x] 3.4 `sh tests/packaging.sh` green, including the new sub-suite, with the
      pre-existing ten still green.
- [x] 3.5 `git diff --check` clean, and only the intended files are staged.
- [x] 3.6 Re-check the untouched surfaces named as non-goals: installer stderr
      text, `--dry-run` output, exit codes, checksum verification, and atomic
      install behaviour.

## 4. Recorded results

- **1.1** Reproduced. `sh tests/packaging.sh` → `FAIL installer`,
  "unexpected installed path: /tmp/…/dest/driftwatchdog". `od -c` on the
  captured value shows `<path>/driftwatchdog\nmavis-trash: moved to trash:
  '/tmp/tmp.hZSODzODpO'\n`; `wc -l` = **2**.
- **1.2** Baseline `cargo test`: `test result: FAILED. 0 passed; 1 failed` for
  `--test packaging`, and `error: test failed … --test packaging`. Every other
  suite passed (lib 623, then 32/12/6/10/7/3/13/14/11/15/19/13/15/7/2/5/6/15).
- **1.3** `scripts/install.sh` has exactly two deletes: `:274` trap (exposed —
  fires after the final `printf`) and `:345` `rm -f` (exposed — stderr-only
  redirect). `scripts/lib/platform.sh` and `scripts/lib/release.sh`, the only
  files `install.sh` sources, contain **no** delete. `scripts/package.sh:46,:68`
  and `scripts/smoke.sh:34,:41,:69` also delete; they are not the install path
  and `smoke.sh:73` discards installer stdout, so they are left alone by design
  (see design.md §4).
- **2.6** Reverted both sites to `trap 'rm -rf "$work_dir"'` and
  `rm -f "$dest_tmp" 2>/dev/null || true`: the new sub-suite fails with
  "happy path: stdout must be exactly 1 line, got 2" followed by
  `shim: rm -rf /tmp/…`. Restored only the trap fix and left `:355` reverted:
  the sub-suite fails with "failure path: stdout must be empty; got:
  shim: rm -f /tmp/…/dest2/.driftwatchdog.install.2067580". Each site is
  therefore caught on its own, on any host. With both fixes in place the
  sub-suite passes.
- **3.1** After: `cargo test` exits 0. **869 passed, 0 failed across 28
  suites**; lib 623, then 32/12/6/10/7/3/13/14/11/15/19/13/15/7/2/5/6/15, then
  `tests/packaging.rs` **1 passed**, then 3/11/7/7/8/4/0. The
  `packaging_bash_and_node_suites_pass` test itself is **1 passed, 0 failed**
  (was 0 passed, 1 failed). The baseline run was 828 passed / 1 failed across
  only 21 suites because `cargo test` aborts at the first failing test binary,
  so the 41-test difference is the newly-passing packaging test plus the seven
  suites the abort had skipped — not new tests. The bash sub-suites are 11/11
  (the pre-existing ten plus `installer_stdout`) and the Node suite is 11/11.
- **3.2** `cargo fmt --check` exit 0; `cargo clippy --all-targets --all-features
  -- -D warnings` exit 0 (`Finished dev profile`). No Rust source changed, so
  both are unaffected by this change and act as a no-regression check.
- **3.5** `git diff --check` exit 0.
- **3.3** `openspec validate installer-stdout-purity --strict` → "Change
  'installer-stdout-purity' is valid". `openspec validate --changes --strict
  --no-interactive` → "Totals: 3 passed, 0 failed (3 items)".
  `openspec list --changes --strict` is **not** a valid invocation on the pinned
  CLI (v1.6.0): it exits 1 with `error: unknown option '--strict' (Did you mean
  --sort?)`. `openspec list --changes` was used instead and resolves all three
  active changes. The strict equivalent the CLI does support is 3.3's
  `validate --changes --strict` (3 passed, 0 failed). The change resolves as
  `✓ Complete` in `openspec list --changes` because every box above is checked;
  it is **not** archived, per the standing instruction.
- **3.6** Re-verified directly: installer stderr still emits the same
  `install: target=… / fetching manifest … / fetching archive … / installed …`
  lines; `--dry-run` stdout is still one line and still writes its three
  informational lines to stderr; `--bogus` still exits 64; the checksum-mismatch
  and missing-archive cases still exit non-zero and leave no binary; the atomic
  stage-then-rename path is unchanged (`mv` to `$dest_tmp` at `:340`, `mv -f`
  to `$binary_path` at `:344`). No change to checksum verification, and no new
  dependency on the `rm` shim leaking — the shim is a test fixture only and is
  never on the installer's real `PATH`.
