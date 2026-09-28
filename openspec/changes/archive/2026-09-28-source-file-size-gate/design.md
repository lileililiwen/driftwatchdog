# Design: Common repository source-file size Gate

## 1. Implementation boundary

The change is implemented in the Driftwatchdog Rust 2021 binary in the Gate
layer. The primary boundaries are:

- `src/gate/concerns.rs`: add `SOURCE_FILE_SIZE`, include it in
  `PRODUCT_QUALITY_CONCERNS`, and expose the exact concern predicate.
- `src/gate/manifest.rs`: add the `[source_size]` configuration model,
  validation, digest participation, and profile defaults.
- `src/gate/source_size.rs`: add source-boundary resolution, raw newline
  counting, deterministic findings, and scanner error classification.
- `src/commands/gate.rs`: route `source-file-size` to the in-process scanner
  and preserve the existing command adapter for all other concerns.
- `src/gate/mod.rs`: register the module.
- `tests/gate_source_file_size.rs`: end-to-end Gate fixtures and persistence
  assertions.

The implementation MUST NOT add a project command binding, invoke a language
tool, parse a language AST, alter `driftwatch check`, change the SQLite schema,
or modify the Workspace Governance repository.

## 2. Language, runtime, and local conventions

The implementation uses Rust 2021 on the repository's declared MSRV Rust 1.74.
It uses `std::fs`, `std::path`, the existing Git/project helpers, `serde`/
`toml` manifest conventions, and the existing `GateResult`/`Finding` types.
The local verification commands are:

```text
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
openspec validate --changes --strict --no-interactive
```

No network, package installation, provider SDK, or runtime daemon is needed.

## 3. Ownership and shared code

Driftwatchdog owns the scanner because it is a built-in Gate capability and
must work without a project-owned command. Workspace Governance remains an
optional policy consumer and does not become a runtime dependency. The generic
project-runtime adapter remains the owner of arbitrary external checks; the
new concern deliberately bypasses it.

The stable contract exported to consumers is the concern ID
`source-file-size`, the `[source_size]` manifest shape, and the existing Gate
result wire shape. A later shared policy project may reference those values,
but no sibling code is copied or edited in this change.

## 4. Configuration and source-boundary model

The manifest adds an optional top-level section:

```toml
[source_size]
max_lines = 1000
include = []
exclude = []
```

Rules:

1. `max_lines` defaults to `1000` and MUST be a positive integer no greater
   than `1_000_000`.
2. `include` and `exclude` are repository-relative glob patterns using `/` as
   the separator. Absolute patterns, `..` traversal, and empty patterns are
   rejected before execution.
3. When `include` is non-empty, a file is a candidate only if it matches at
   least one include pattern. When it is empty, the scanner discovers files
   from Git-listed repository paths and conventional source roots (`src`,
   `app`, `lib`, `bin`, `cmd`, `internal`, `packages`, `server`, `client`).
4. `exclude` always wins over `include` and discovery. The default exclusion
   set covers `.git`, `.driftwatch`, `node_modules`, `target`, `vendor`,
   `third_party`, `thirdparty`, `dist`, `build`, `.venv`, `venv`, coverage,
   generated, fixtures, and test directories. The default set is path-segment
   based, not language based.
5. In Git repositories, candidates come from `git ls-files --cached
   --others --exclude-standard -z`, so `.gitignore` excludes untracked
   dependency/build content while tracked files remain visible for review.
   The scanner MUST additionally apply the default and configured exclusions
   to tracked paths.
6. In a non-Git directory, the scanner recursively walks the project root
   without following symlinks, applies the same path exclusions, and treats a
   missing `.gitignore` as an empty Git exclusion set. A present `.gitignore`
   is applied using the repository's supported Git-ignore matching path or a
   compatible local matcher; inability to evaluate it is a review condition,
   not an implicit pass.
7. Only regular UTF-8 text files are counted. Files with NUL bytes or invalid
   UTF-8 are excluded as non-source binary content. This is file-type
   filtering, not language parsing.
8. Candidate paths are normalized to repository-relative `/` paths and sorted
   lexicographically before reading.

The default source-root discovery intentionally prevents README files,
lockfiles, and arbitrary repository metadata from becoming source findings
without requiring language knowledge. Explicit `include` patterns are the
escape hatch for repositories with different layouts.

## 5. Counting and result state model

The scanner counts `0x0A` bytes in each candidate file. This is exactly the
observable `wc -l` rule: a final line without a newline does not increment the
count. It does not remove blank lines, comments, strings, or generated text
inside an included file.

| Condition | Result | Evidence/finding | Aggregate |
|---|---|---|---|
| Scan completes; every candidate is at or below `max_lines` | `PASS` | Diagnostic with candidate count and threshold | Non-blocking |
| One or more candidates exceed `max_lines` | `FAIL` | One bounded finding per violating path, with counted lines, limit, and excess | Blocks when required |
| Git boundary, ignore evaluation, path, or file read fails | `REVIEW_REQUIRED` | Bounded diagnostic and missing-evidence entry naming the operation/path | Blocks when review policy blocks |
| No candidate files are discovered | `PASS` | Diagnostic records zero candidates and the active boundary | Non-blocking |

The scanner must continue reading other candidates after a single read error so
one bad file does not suppress unrelated findings. Any read/boundary error
forces `REVIEW_REQUIRED`, even if other files pass. A missing Git executable in
a non-Git directory is not an error; it selects the non-Git fallback. A Git
command that reports an actual repository error is a review condition.

The concern uses `source = "driftwatchdog"`; it never emits
`NOT_APPLICABLE` when selected and required. An explicit `enabled = false` or
`required = false` continues to use existing manifest policy. The scanner is
read-only and does not persist its own state.

Findings use the existing bounded fields:

```text
title    = "<relative path> has <actual> physical lines; maximum is <limit>"
severity = error
location = <relative path>
rule     = "source-file-size"
```

The result has no raw file contents. Diagnostics and paths use existing size
limits and redaction helpers.

## 6. Gate integration and compatibility

`product` and `rust-product` resolve `source-file-size` alongside the existing
product concerns. The plan is data-compatible, but execution dispatches this
one ID to the scanner with the project root and resolved source-size policy.
All other planned checks retain their current dispatch and command behavior.

The manifest digest includes the source-size policy because changing the
threshold or boundary changes the Gate contract. `--dry-run` renders the
selected concern and policy without scanning, writing, or persisting. Human
and JSON output reuse existing rendering and aggregation. Gate history stores
the normalized result in the existing bounded `results_json` field.

If a foreign `.ai-gate/gate.yaml` runtime is selected, no scanner runs. If no
Gate manifest exists, existing no-op behavior remains unchanged.

## 7. Failure, security, and boundary policy

- Empty `include`/`exclude` entries: configuration error before scanning.
- Absolute or traversal patterns: configuration error before scanning.
- Missing `source_size` section: use the 1,000-line default and default
  discovery policy.
- Missing `.gitignore`: no-op; use Git's own default exclusion behavior and
  built-in exclusions.
- Unreadable `.gitignore`, failed Git listing, or unreadable candidate:
  `REVIEW_REQUIRED`; continue independent candidate reads where safe.
- Symlink candidate or symlinked directory: skip without following it.
- Binary/invalid UTF-8 file: skip as non-source; do not fail the Gate.
- Oversized file: `FAIL`, not review, with a deterministic finding.
- Excessive candidate count or path/output volume: honor existing bounded
  result limits and report truncation as review evidence if complete boundary
  evaluation cannot be proven.
- No network, shell command from project configuration, source upload, or
  automatic rewrite is permitted.

## 8. Verification oracle

Unit tests in `src/gate/source_size.rs` MUST cover raw newline counting,
unterminated final lines, default roots, configured includes/excludes,
exclusion precedence, `.gitignore` behavior, dependency/build directory
exclusion, binary skipping, symlink non-following, deterministic ordering,
threshold boundaries at 999/1000/1001, and read/boundary review outcomes.

Integration tests in `tests/gate_source_file_size.rs` MUST cover:

- `product` and `rust-product` selecting the concern and passing at 1,000;
- a 1,001-line included file returning `FAIL`, blocking the process, and
  appearing in human and JSON output;
- an oversized `node_modules`/`target`/ignored file not affecting PASS;
- explicit include/exclude overrides and exclude precedence;
- missing/invalid boundary input producing `REVIEW_REQUIRED`;
- `--dry-run` not scanning or persisting;
- existing profile, foreign-runtime, persistence, and opt-out behavior.

Completion evidence requires the four local commands in section 2, a clean
`git diff --check`, and the relevant integration tests with exit codes and
serialized findings asserted. Strict OpenSpec validation proves package
structure only; it does not prove runtime enforcement until implementation.

## 9. Decision ledger

- Per-file rather than aggregate limit: selected because the Gate must identify
  and remediate the concrete oversized source boundary.
- Raw `wc -l` semantics: selected explicitly; no language-specific line
  interpretation is allowed.
- Git-aware discovery plus configured overrides: selected to avoid scanning
  dependencies and ignored build trees without forcing every project to write a
  checker.
- Default threshold: fixed at 1,000 physical lines; projects may tighten or
  relax it through the manifest policy, subject to the positive/range guard.
- Scanner location: Driftwatchdog Gate, not Workspace Governance or project
  scripts, because this is common execution behavior.
- No unresolved decisions or blockers remain for implementation.
