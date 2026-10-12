# Design: Mandatory governance quality for every profile

## Boundary

`profile_defaults_for` owns effective concern selection. It preserves each
built-in or project-defined concern set and adds `governance-quality` when
absent. The ten accepted workspace profiles with no other Driftwatchdog set
start with governance quality only. Keeping those names explicit preserves
unknown-profile typo detection. The required concern does not synthesize a
command; an unbound concern remains `UNBOUND` and fails under existing rules.

## Concern ownership

| Profile family | Defaults |
| --- | --- |
| backend, frontend, full, minimal | Existing defaults plus governance quality |
| product, rust-product | Existing product concerns, already including governance quality |
| release | Existing release concerns plus governance quality |
| deployable | Existing deployable concerns plus governance quality |
| Ten accepted workspace profiles | `governance-quality` only |
| Project-defined profile | Declared concerns plus governance quality |
| Unknown profile | Rejected with `UnknownProfile` |

Product-quality concerns remain limited to the product profiles. The fallback
must never include API, migration, frontend, release, deployment, source-size,
or placeholder checks.

## Resolution compatibility

Project-defined profiles are stored in `GateManifest.profiles` and continue to
contribute their declared concerns through `profile_defaults_for`, with the
mandatory concern appended once. The ten workspace profile names are
enumerated in the supported profile list so misspellings remain errors rather
than silently becoming governance-only profiles.

## Failure behavior

The fallback supplies an ID only. Missing `commands.governance-quality` stays
unbound and fails through the current required-check path. No checker path is
inferred, and no command is executed by profile resolution itself.

## Verification boundary

Unit tests cover all explicit built-ins, a declared project profile, a fallback
profile, exactly one governance concern, and exclusion of product concerns.
Integration verification uses the rebuilt local binary on an existing project
manifest and confirms the gate schedules governance quality. No stale binary on
`PATH` is evidence.
