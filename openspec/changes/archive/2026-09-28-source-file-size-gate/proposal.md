# Proposal: Enforce repository source-file size boundaries in the common Gate

## Why

The current product Gate can require project-owned quality commands, but it
does not provide a common source-size check. Requiring every monitored project
to write and maintain its own line-count script creates inconsistent boundaries
and allows oversized source files to pass when no local checker is configured.

The Gate should provide one repository-aware, language-independent checker that
counts physical lines like `wc -l` and blocks an oversized source file.

## What Changes

- Add the stable `source-file-size` Gate concern to the built-in `product` and
  `rust-product` profiles.
- Add a common Driftwatchdog scanner that discovers repository-owned source
  files, reads `.gitignore`/Git exclusion state, excludes dependency/vendor/
  build trees, and counts raw newline bytes without parsing any language.
- Set the default maximum to 1,000 physical lines per included file.
- Produce normalized `PASS`, `FAIL`, or `REVIEW_REQUIRED` results directly from
  Driftwatchdog; no project command or external checker is required.
- Allow repository-relative include and exclude globs for layouts that cannot
  be inferred from conventional source roots.
- Keep findings bounded, deterministic, repository-relative, and compatible
  with existing Gate JSON, persistence, redaction, and blocking policy.

## Package Boundary and Split Assessment

This is one package because source-boundary discovery, raw line counting, the
`source-file-size` concern, and Gate result normalization share one owner,
execution lifecycle, configuration contract, and acceptance oracle: a local
Gate run must identify and block an oversized repository-owned source file.

The package does not include a language parser, project-specific checker
scripts, IDE integration, automatic refactoring, or changes to sibling
repositories. No independent deployment, persistence migration, or external
service lifecycle is introduced.

| Package | Single outcome | Owner/project and language | Boundary/contract | Depends on | Independent oracle |
|---|---|---|---|---|---|
| `source-file-size-gate` | Common Gate blocks included files over 1,000 raw physical lines | Driftwatchdog, Rust 2021 | Built-in `source-file-size` concern and repository-relative source-boundary policy | Existing Gate manifest, result, aggregation, and persistence contracts | Fixture repository with ignored dependency tree, included oversized file, and passing boundary cases |

## Sibling and Shared Architecture Reconnaissance

| Candidate | Evidence path/symbol | Reusable code/config/architecture | Compatibility gap | Owner and release boundary | Decision |
|---|---|---|---|---|---|
| Driftwatchdog Gate | `src/gate/manifest.rs`, `src/commands/gate.rs`, `src/gate/types.rs`, `src/gate/aggregate.rs` | Built-in concern vocabulary, resolved profiles, local execution, bounded findings, blocking and JSON output | No repository source-boundary scanner exists yet | Driftwatchdog release owns common Gate execution | **extend shared owner** |
| Workspace Governance | `.ai-gate/gate.yaml` runtime/profile convention and existing product-quality concern contract | Project policy declarations and runtime selection | It does not own Driftwatchdog's local filesystem scan or Gate result execution; per-project commands are the problem this change removes | Workspace Governance owns cross-project policy; Driftwatchdog owns the central runtime | **adapt through a generic adapter** |
| Monitored project scripts | Existing `[[checks]] command` bindings | Generic command adapter remains reusable for unrelated checks | Local scripts duplicate source-boundary logic and cannot be the common checker | Each project owns its own optional commands | **keep local only for unrelated checks** |

The scanner remains in Driftwatchdog because the requested capability is a
central Gate primitive, not a provider contract. Workspace Governance may later
consume the stable concern and configuration vocabulary, but this change does
not edit that sibling project or require its checker.

## BFS Impact Map

| Area | Impact |
|---|---|
| Concern vocabulary | Add `source-file-size` to the product-quality concern set. |
| Profile resolution | `product` and `rust-product` select it as required by default; explicit `enabled = false` and existing optional policy remain available. |
| Source boundary | Resolve explicit repository-relative `include`/`exclude` globs first; otherwise use Git-listed non-ignored files and conventional source roots, with dependency/vendor/build exclusions. |
| Counting | Count only `0x0A` newline bytes; an unterminated final line is treated exactly as `wc -l` treats it. No language parsing. |
| Execution | Route the concern to an in-process scanner, not `project-runtime` and not a project command. |
| Results | Emit bounded file findings on `FAIL`; emit `REVIEW_REQUIRED` for boundary/read failures; aggregate required failures as blocking. |
| Configuration | Extend the manifest with a `[source_size]` policy containing `max_lines`, `include`, and `exclude`; reject absolute or escaping patterns. |
| Persistence/output | Reuse existing `GateResult`, aggregate status, human/JSON rendering, and `gate_runs`; no database migration. |
| Compatibility | Existing profiles, explicit project commands, checker-only `driftwatch check`, foreign runtimes, and unrelated concerns remain unchanged. |
| Tests | Unit-test boundary resolution/counting and integration-test profile selection, ignored dependencies, failure, review, dry-run, JSON, persistence, and opt-out behavior. |
| Security | Do not follow symlinks, leave the repository root, execute discovered files, or upload source. Bound paths and diagnostics. |

## Capabilities

- `source-file-size-gate`

## Non-goals

- Counting aggregate project lines instead of enforcing a per-file boundary.
- Parsing Rust, JavaScript, Python, C#, or any other language.
- Treating comments, strings, or generated code specially based on language
  syntax; the count is raw physical lines.
- Scanning dependencies, vendored libraries, build output, or Git-ignored
  working-tree content by default.
- Requiring every project to add a checker command.
- Automatically splitting, rewriting, or approving oversized files.
- Proving code quality, correctness, test coverage, or architectural health.
