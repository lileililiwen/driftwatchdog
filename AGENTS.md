# Agent Instructions

## Scope

Driftwatchdog is a local-first Rust CLI for remembering runtime failures across AI-assisted coding sessions. Rust describes the implementation, not the monitored project: command execution and failure normalization must remain language-agnostic.

## Required workflow

- Read README.md, ROADMAP.md, and the relevant OpenSpec change before implementation.
- Implement one OpenSpec change at a time and keep its tasks.md accurate.
- Every change follows BFS → DFS → BFS: map impact first, implement one
  coherent requirement at depth, then perform a system-wide regression review.
- `proposal.md` is the initial BFS impact map; `design.md` defines all module,
  process, storage, security, and compatibility boundaries; `tasks.md` contains
  explicit BFS, DFS, and final BFS task groups; `spec.md` defines behavior.
- Run local verification and the applicable Gate before archive or completion;
  CI repeats local verification and is not the first check.
- A change with open final-BFS tasks must not be archived or represented as complete.
- Run openspec validate --changes --strict --no-interactive after spec edits.
- Use tests for every behavior change; run formatting, tests, and clippy before claiming completion.
- Preserve unrelated user changes and do not rewrite or delete existing state without explicit authorization.

## Change completion workflow

When a change is implemented and its `tasks.md` is fully checked off, follow this exact sequence before moving to the next change. Do not skip steps. The change is not "done" until step 6 completes.

1. Run verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
2. Update `openspec/changes/<change>/tasks.md` and mark every box finished (`[x]`). Do not proceed with open boxes.
3. Only after tasks.md is fully finished, archive the change with `openspec archive <change> -y`; canonical capability
   specs must be promoted from the change when a `spec.md` is present.
4. Commit 1 — implementation + archive in a single commit: `git add -A && git commit -m "Implement <change>"`.
5. Commit 2 — update `HANDOFF.md` (change status, next action, module map if applicable) and related docs (`ROADMAP.md`, `README.md` when sequencing or surface changes) and commit them: `git add HANDOFF.md ROADMAP.md README.md && git commit -m "Update HANDOFF after <change>"` (stage only the docs that actually changed).
6. Re-run `openspec validate --changes --strict --no-interactive` to confirm the change list still resolves.

**Why:** OpenSpec's `archive` moves a change out of `openspec/changes/` into `openspec/changes/archive/` and updates main specs from the change's `spec.md`. Splitting the work into one implementation commit and one doc commit keeps the implementation reviewable as a unit and lets reviewers see handoff context separately from code.

**How to apply:** Apply this to every change listed in `ROADMAP.md` in the
documented sequence. Planning-only changes remain unchecked and are not archived
until separately authorized for implementation.

**Standing auto-mode authorization:** the operator runs in auto mode with no
time for per-change confirmation. This paragraph is standing authorization to
work through the `ROADMAP.md` planning queue without asking: pick the next
dependency-ordered change with open tasks, implement it fully (BFS → DFS →
BFS), run the change completion workflow above, then continue to the next
change. Still one change at a time; still no confirmation prompts.

## Architecture rules

- Keep the core as a single Rust binary with focused modules.
- Store local state under .driftwatch/; do not add network calls, telemetry, account flows, or LLM API calls to the core.
- Wrap arbitrary child commands without assuming Cargo, Rust, or a particular shell language.
- Keep the generic normalizer as the compatibility baseline; language-specific profiles are optional and additive.
- Treat external spec tools as adapters behind the normalized checker protocol.
- A checker failure must not abort remaining checker runs.
- Keep specification systems optional context providers; OpenSpec must not be a
  dependency of generic Gate core.
- Keep managed Gate tooling separate from project runtimes such as .NET, Rust,
  Python, and Node.
- Heuristic correlations are evidence, not root-cause detection. Use cautious labels.

## Documentation rules

- Update ROADMAP.md when sequencing or release scope changes.
- Keep user-facing command examples in README.md accurate.
- Record implementation state, blockers, and verification results in HANDOFF.md.
- Do not silently expand the explicit v1 non-goals.
