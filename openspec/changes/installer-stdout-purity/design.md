# Design: Make the shell installer's stdout a single machine-readable line

## 1. The contract being fixed

`install.sh` is a POSIX `sh` script, not a Rust binary, so it carries no
versioned output document. Its stdout contract is the older and stricter one:
**on success, stdout is one line and that line is the installed path**; on
failure, stdout is empty and every diagnostic goes to stderr. `--dry-run`
inherits the same rule (`install.sh:255` prints the path it *would* install, and
the informational lines above it all carry `>&2`).

Every progress line in the script already carries an explicit `>&2`. The
installer is otherwise careful about this; the leak is that the *cleanup* path
was written as if the external command it calls were guaranteed silent.

## 2. Why a chatty external command is a portability bug, not a host quirk

`rm` is not specified to be silent. POSIX leaves a delete's diagnostic output
undefined, and a growing set of hosts and environments replace `/bin/rm` with a
wrapper that reports what it did — recoverable-delete shims, audit wrappers,
sandboxed filesystems, and `trash`-style front-ends. A shell script that puts
machine-readable data on stdout cannot assume a third-party helper will say
nothing, because the helper's behaviour is not part of the script's contract and
not under its control.

The redirection costs nothing when `rm` *is* quiet — the normal case is
unchanged byte-for-byte — and restores a guarantee the script does not currently
make. That asymmetry is the whole argument: free on a conforming host, correct
everywhere else.

## 3. The two call sites

Both are in `scripts/install.sh`, and the sweep of the install path found no
others.

| Site | Command | Why it is exposed |
| --- | --- | --- |
| `:274` | `trap 'rm -rf "$work_dir"' EXIT INT TERM` | Fires **after** the final `printf` on the happy path, and after every `die` on a failure path. This is the observed failure. |
| `:345` | `rm -f "$dest_tmp" 2>/dev/null || true` | Redirects stderr only. Reached when the final `mv` into place fails, immediately before `die`. |

Both become `rm ... >/dev/null 2>&1 || true`.

**Why also `|| true` on the trap.** The existing trap has no `|| true`, and with
coreutils `rm -rf` on an existing directory the status is always 0, so the
script's exit status is unaffected today. But an `EXIT` trap's last command can
become the script's exit status in some `sh` implementations, and a delete
wrapper is permitted to fail. A wrapper that failed to remove `$work_dir` would
then turn a successful install into a non-zero exit — a real regression, in
exchange for protecting against a leak that costs nothing. `|| true` keeps
cleanup failures from becoming install failures, which is the correct behaviour
for a best-effort cleanup.

**Why not silence stderr as well.** The trap deliberately has no `2>/dev/null`
today, so a genuine "could not remove the work directory" is visible. Routing
both streams to `/dev/null` discards that. The alternative — redirect stdout
only (`>/dev/null`) — preserves the diagnostic and still fixes the leak, because
the observed noise was on stdout.

**Rejected: reordering the trap so it runs before the final `printf`.** Traps
run at exit, not at a chosen point; faking it with an explicit cleanup call
before the `printf` duplicates the work, and any later `die` still needs the
trap anyway. The redirect is one token at two sites.

**Rejected: dropping the trap and cleaning up inline.** Loses cleanup on the
`die` paths, which is where a partially-extracted archive is most likely to be
left behind. Not a trade worth making for a stdout problem.

## 4. Scope of the sweep

The install path is `scripts/install.sh` plus the helpers it sources,
`scripts/lib/platform.sh` and `scripts/lib/release.sh`. A sweep of those three
files for delete invocations found exactly the two above; the helpers contain
none. `scripts/package.sh` (`:46`, `:68`) and `scripts/smoke.sh` (`:34`, `:41`,
`:69`) also delete, and are deliberately left alone: neither is the installer,
neither publishes a machine-readable stdout, and `smoke.sh:73` discards the
installer's stdout to `/dev/null`. Recording them here so a later reader knows
they were looked at and the omission is a decision.

## 5. Making the regression test host-independent

The naive regression test — "assert stdout is one line" — only fails on a host
that happens to have a noisy `rm`. On a stock host it is green before the fix and
green after, which proves nothing.

The test therefore controls the environment instead of hoping for it. It builds a
scratch directory containing two executables and prepends it to the installer's
`PATH`:

- `rm` — prints one line naming its arguments to **stdout** and exits 0. It
  deletes nothing. This is precisely the exposure, reproduced deterministically.
- `mv` — counts its invocations in a file, delegates to the real `mv` found on
  the host's `PATH` before the shim directory was prepended, and fails from the
  second call onward. `install.sh` calls `mv` exactly twice (`:340` stages into
  `$dest_tmp`, `:344` moves into place), so the second call is the final move and
  failing it is what reaches the `:345` cleanup.

The shim directory is prepended to `PATH` only for the child `install.sh`
process, so nothing in the harness's own environment changes.

The counter file and the shim directory are the only state the fixtures create.
Neither is cleaned by a delete: the shim `rm` is deliberately a non-deleting
printer, and the test itself never invokes a delete. The scratch directories are
left for the host to reap. That is a deliberate trade — a test that shells out
to a delete on a host where that delete is a recoverable-delete shim can fail
under `set -e` and leave paths behind, which is worse than a few `/tmp`
directories.

## 6. The assertions

Two scenarios, because there are two call sites and they are reached by
different paths.

| Scenario | `rm` in install path | `mv` in install path | Expected stdout |
| --- | --- | --- | --- |
| happy path | chatty shim, trap fires at exit | delegates both calls | exactly one line, and it is `$dest/driftwatchdog` |
| final move fails | chatty shim, trap fires at exit, `:345` fires first | fails on call 2 | empty |

The happy-path assertion is deliberately the *stronger* property rather than a
re-run of the old equality: line count first, then the value. An equality-only
test cannot tell a clean one-line stdout from a correct-looking first line with
trailing garbage, which is precisely the bug.

Line counting uses `printf '%s\n' "$x" | wc -l`, which is safe here because
command substitution has already stripped trailing newlines: the count reflects
the *interior* line count, and any appended output raises it.

## 7. Blast radius

- `install.sh` stdout on the happy path: unchanged for a caller on a
  conforming host (`$dest/driftwatchdog`, one line).
- `install.sh` stderr: unchanged. The trap keeps its stderr channel, so a
  cleanup failure is still reported.
- Exit status: unchanged, and made more robust — a failing delete wrapper can no
  longer turn a successful install into a failure (§3).
- `--dry-run`: unchanged; it exits before the trap is installed.
- Packaging suite: grows from 10 to 11 sub-suites. The pre-existing
  `test_installer.sh` keeps its assertions and gains the one-line check on its
  own happy path; nothing is weakened or removed.
- No Rust source, no CLI surface, no spec other than this delta.
