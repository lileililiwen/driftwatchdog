# Design: Post-MVP README readiness for Driftwatchdog

## 1. Implementation boundary

**Repository:** `/home/paul/code/driftwatchdog` — a single Rust binary.

**Files to change:**

- `README.md` — add the terminal capture embeds, the consolidated built-in
  profile/concern catalog, and the ordered integration recipe; keep the command,
  MCP, checker, and Gate sections accurate.
- `docs/assets/` (new) — the checked-in terminal capture(s), stored as plain
  text (ANSI stripped) and/or SVG/PNG generated from a synthetic project.
- `openspec/changes/post-mvp-readiness/` — this change package.

**Must not change:** `src/**` (especially `src/gate/**` and
`src/gate/concerns.rs`), `Cargo.toml`/`Cargo.lock`, `tests/**`, `templates/**`,
`examples/**`, `npm/**`, `scripts/**`, `driftwatch.toml.example`, or any
canonical spec beyond promoting this change.

## 2. Language and runtime

Documentation is Markdown with fenced `text`/`sh` blocks. Terminal captures are
produced from a synthetic local project (no network, no real repository data)
and committed under `docs/assets/` as plain text and/or SVG (no external image
host).

Exact commands:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
sh tests/packaging/run_all.sh
openspec validate --changes --strict --no-interactive
git diff --check
```

The applicable local Gate is `driftwatch gate` for this repository (its
`.ai-gate/gate.yaml` declares `runtime: driftwatchdog`, `profile: rust-product`).

## 3. Ownership and shared code

Driftwatchdog **owns the central workspace Gate** and its vocabulary. 75
workspace `.ai-gate/gate.yaml` files name `runtime: driftwatchdog`, so the
README's profile/concern catalog is a shared reference. This change documents
the shipped contract; it does not change it.

- **Exported contract:** the profile names, stable concern IDs, exit-code
  authority rule, and envelope behaviour, as already implemented.
- **Consumers:** every workspace project with `.ai-gate/gate.yaml`; Forge
  consumes `checker-machine-output`; Workspace Governance owns the
  release-evidence vocabulary consumed here.
- **Dependency direction:** consumers reference driftwatchdog; driftwatchdog
  consumes Workspace Governance's vocabulary but does not import it.
- **Release boundary:** ships with the repository; no packaging change.

## 4. Behavioral model

| Readiness gap | README surface | Target state |
|---|---|---|
| Output unseen | Terminal capture embeds | A checked-in capture shows a `driftwatch run` failure and recurring-bug report (and optionally a Gate run) |
| Catalog scattered | Built-in profile/concern catalog | Each profile maps to its concern IDs, required/optional default, exit-code authority, and text-mode fallback |
| Integration unclear | Ordered integration recipe | Step-by-step from no manifest to a first `driftwatch gate` run |
| Shared Gate reference | Reconnaissance note | States that driftwatchdog owns the central Gate and documents the shipped contract |

**Contract facts the catalog MUST preserve (from the shipped specs):**

- `product` / `rust-product` select `product-code-boundary` and
  `placeholder-threshold`; `release` selects `release-evidence` and
  `capability-conformance`.
- Exit-code authority: `PASS`→0, `FAIL`→1, `REVIEW_REQUIRED`→2,
  `NOT_APPLICABLE`→0; mismatches and malformed/wrong-version envelopes
  downgrade to `REVIEW_REQUIRED`.
- Product-quality concerns keep a text-mode fallback; the release-gate concerns
  deliberately do **not**.
- Unbound required concerns become `REVIEW_REQUIRED`; `required = false` relaxes
  one concern.

## 5. Contract and compatibility

- The change is additive; every existing README section, command, and example
  is preserved.
- The catalog restates only shipped behavior and matches `src/gate/concerns.rs`
  and the canonical specs; if a documented name disagrees with the code, the
  code wins and the doc is corrected.
- Capture links are repository-relative and resolve on disk; no external host.
- No wire version, concern ID, CLI flag, or profile default changes.

## 6. Failure and boundary policy

| Case | Result |
|---|---|
| Capture link points at an uncommitted file | Change incomplete; link check fails |
| Capture contains secrets or real repository data | MUST NOT be committed; regenerate from a synthetic project |
| Catalog names a concern/profile that is not in `src/gate/concerns.rs` | Rejected; code is authoritative |
| Recipe implies driftwatchdog publishes, signs, generates SBOMs, or deploys | Rejected; driftwatchdog is an executor/aggregator |
| Any `src/gate/**`, concern ID, envelope, or CLI change | Out of boundary; revert |
| New network call, telemetry, or LLM dependency | Rejected |

## 7. Verification oracle

- `cargo fmt --check` — clean.
- `cargo test` — all tests pass, including the packaging bridge
  (`tests/packaging.rs` → `sh tests/packaging/run_all.sh`), which also runs
  `test_change_workflow.sh` (this change must satisfy BFS/DFS/BFS and carry a
  Scenario) and `test_repo_hygiene.sh`.
- `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- `openspec validate --changes --strict --no-interactive` — this change and
  `gate-evidence-export` pass.
- `git diff --check` — no whitespace errors.
- Manual oracle: a project owner can follow the recipe to a first
  `driftwatch gate` run, and the capture matches real output on a synthetic
  project.

## 8. Decision ledger

**Verified assumptions:**

- The built-in profiles and concern IDs are as shipped; `src/gate/concerns.rs`
  and the canonical specs are authoritative.
- The release-gate concerns intentionally have no text-mode fallback; the
  product-quality concerns do (README "Product-quality Gate contract" and
  "Release-evidence and capability-conformance Gate contract").
- 75 workspace `.ai-gate/gate.yaml` files name `runtime: driftwatchdog`.

**Resolved alternatives:**

- (a) Capture as a screenshot image vs checked-in text/SVG → chose
  **text plus optional SVG**, so the content is diffable and verifiable.
- (b) Catalog inside the existing prose sections vs one consolidated table →
  chose **one catalog table** with links to the existing sections.
- (c) Recipe in `templates/` docs vs the README → chose the **README**, so the
  shared reference stays in one place.
- (d) Document the central Gate ownership in the recon table vs the README body
  → noted in **both**, because consumers search either.

**Blockers:** none. If a documented profile/concern name cannot be confirmed
from `src/gate/concerns.rs`, stop and record the mismatch rather than documenting
a guessed name.
