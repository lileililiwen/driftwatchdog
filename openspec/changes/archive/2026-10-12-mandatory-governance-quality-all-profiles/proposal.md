# Proposal: Make governance quality mandatory for every profile

## Why

The workspace gate has repositories using profile names beyond Driftwatchdog's
built-in product and tooling profiles. If those names resolve to no defaults,
the gate can schedule no checks, leaving governance enforcement opt-in by
profile spelling. The central governance check must run for every usable gate
profile while product-specific concerns remain scoped to product profiles.

## Impact map

- `src/gate/manifest.rs`: profile defaults and resolution determine which
  concerns are scheduled and whether project-defined profiles remain usable.
- Gate manifest tests: pin each known profile, unknown-name behavior, and
  project-defined profile compatibility.
- `.ai-gate/gate.yaml`: existing profile declarations resolve through this
  function; no fleet manifest is edited by this change.
- `HANDOFF.md`: record measurements, checks, and the still-active change state.

## Scope

- Add `governance-quality` as the sole fallback concern when a profile has no
  built-in defaults.
- Keep built-in product, release, and deployable concern sets unchanged.
- Preserve project-defined profiles and explicit check additions.
- Prove that fallback profiles schedule exactly one governance concern and do
  not acquire product-quality concerns.

## Non-goals

- Do not add any other concern to the fallback.
- Do not change concern adapters, gate bindings, or workspace manifests.
- Do not change product-quality profile membership.
- Do not archive this change without separate authorization.
