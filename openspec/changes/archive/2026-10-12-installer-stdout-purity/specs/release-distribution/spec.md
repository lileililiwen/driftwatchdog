# installer-stdout-purity Specification (delta: installer-stdout-purity)

## ADDED Requirements

### Requirement: The shell installer writes a single line to stdout on success

The shell installer SHALL write exactly one line to standard output on a
successful install, and that line SHALL be the absolute path of the installed
executable. It SHALL NOT write anything else to standard output before or after
that line. On a failing run it SHALL write nothing to standard output, and SHALL
write all diagnostics to standard error.

Progress, warning, and error output SHALL remain on standard error and SHALL be
unchanged by this requirement. The `--dry-run` form SHALL satisfy the same
one-line rule.

This requirement holds regardless of what any external command the installer
shells out to writes. The installer SHALL NOT depend on a helper command being
silent in order to keep its standard output parseable.

#### Scenario: Captured stdout is exactly the installed path

- **WHEN** a caller captures the standard output of a successful install
- **THEN** the captured value is exactly one line, and that line is the absolute
  path of the installed executable

#### Scenario: Cleanup does not append to a successful install's stdout

- **WHEN** the installer's exit trap removes its working directory after the
  installed path has been written
- **THEN** the working-directory removal writes nothing to standard output, and
  the captured value remains a single line

#### Scenario: Cleanup does not append to a failed install's stdout

- **WHEN** the install fails after staging a temporary binary, and the installer
  removes the staged binary before exiting
- **THEN** the removal writes nothing to standard output, and the run's standard
  output is empty

#### Scenario: A chatty helper on PATH does not change stdout

- **WHEN** a command the installer invokes during cleanup, or the installer's
  exit trap itself, writes to standard output
- **THEN** the installer's own standard output is unaffected, and still carries
  only the installed path on success

#### Scenario: Cleanup failure is not an install failure

- **WHEN** the working-directory removal exits non-zero
- **THEN** the installer's exit status is still the status of the install itself,
  and a successful install remains successful

#### Scenario: Dry run reports one line

- **WHEN** the installer is run with `--dry-run`
- **THEN** its standard output is exactly one line naming the path it would
  install to, and its informational lines remain on standard error
