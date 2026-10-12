# Proposal: Make the shell installer's stdout a single machine-readable line

## Why

`scripts/install.sh` writes exactly one thing to stdout on the happy path: the
absolute path of the installed executable. That is a contract. Callers capture
it with `$( )` and compare it, and `tests/packaging/test_installer.sh:21` does
exactly that. Nothing is supposed to follow it.

Two cleanup `rm` invocations can append to that stdout:

- `scripts/install.sh:274` — `trap 'rm -rf "$work_dir"' EXIT INT TERM`. The EXIT
  trap runs *after* the final `printf '%s\n' "$binary_path"`, so anything the
  delete writes lands after the path, in the same stream.
- `scripts/install.sh:345` — `rm -f "$dest_tmp" 2>/dev/null || true`, which
  redirects stderr but not stdout.

A coreutils `rm` is silent, so on a normal host the exposure is invisible. It is
not invisible everywhere: on a host where `rm` is a recoverable-delete wrapper
that reports what it moved on **stdout**, the captured value becomes two lines
and the equality fails. Measured on such a host:

    $ installed=$(... sh scripts/install.sh --version 9.9.9 --dest "$dest")
    $ printf '%s\n' "$installed" | od -c
    .../dest/driftwatchdog\nmavis-trash: moved to trash: '/tmp/tmp.hZSODzODpO'\n
    $ printf '%s\n' "$installed" | wc -l
    2

`cargo test` reports exactly one failure from this: `packaging_bash_and_node_
suites_pass` → sub-suite `installer`, "unexpected installed path".

The defect is in the installer, not in the test. A CLI whose stdout is a
machine-readable value must guarantee that nothing else ever follows it, on
every host, regardless of what the external commands it shells out to happen to
print. Right now it makes no such guarantee: it relies on `rm` happening to be
quiet. A failure path that leaks a delete notice into stdout is the same bug in
a different place.

## What Changes

- Redirect **both** streams for every cleanup delete in the install path, at
  both call sites.
- Assert the stronger property in `tests/packaging/`: the installer's stdout is
  exactly one line and that line is exactly the installed path — on the happy
  path — and is **empty** on the failure path that reaches the `:345` cleanup.
- Make the regression test host-independent by putting a deliberately chatty
  `rm` (and, for the failure path, a second-call-failing `mv`) on the installer's
  `PATH`. Without the fix the test fails on every host, not only on hosts that
  happen to ship a noisy delete.

## Package Boundary

This change touches only this repository. It modifies `scripts/install.sh` and
the packaging test suite. It does not change the installer's CLI surface, exit
codes, checksum verification, atomicity, or the release artifacts.

## Non-Goals

- Not changing what the installer prints to **stderr**. The progress lines
  (`install: target=…`, `install: fetching …`, `install: installed …`) stay
  exactly as they are.
- Not changing the `--dry-run` stdout, which is also exactly one line and is
  already trap-free (the trap is installed after the dry-run `exit`).
- Not silencing deletes in `scripts/package.sh` or `scripts/smoke.sh`. Those are
  not the install path and neither publishes a machine-readable stdout;
  `smoke.sh:73` discards the installer's stdout entirely. Recorded here as swept
  and deliberately out of scope, not as missed.
- Not adding a `PATH` shim or a `rm` replacement to `scripts/` itself. The shim
  is a test fixture and lives only in the test's scratch directory.
