# Proposal: Bind every declared verification command into the gate that blocks

## Why

Ruling D18 in this workspace: *a verifier that no gate runs does not gate; wire
every check into the gate that blocks, or it is documentation.* At fleet scale
that ruling is being violated by omission. Every repository already recorded how
it should be verified, and the gate ignores that record.

**Measured across the 81 sibling repositories on 2026-10-04**, re-derived from
the files rather than taken from the checkpoint:

| Fact | Count |
| --- | --- |
| Git repositories in the workspace | 81 |
| Carrying a `.project.json` | 81 |
| Declaring a non-empty `verification.command` | **81** |
| Carrying a `.ai-gate/gate.yaml` | **76** |
| Binding at least one `commands:` entry | **6** |
| Carrying a manifest, declaring a command, binding none | **70** |
| Carrying no `.ai-gate/gate.yaml` at all | **5** |

The checkpoint figures (83 manifests, 7 binding, 74 unbound) are superseded.
81 is the git-repository count and is confirmed; the manifest count is 76 and
the binding count is 6. The 5 repositories without a `.ai-gate/gate.yaml` are
`chronicleaf`, `devloom`, `kairovia`, `lileililiwen.github.io` and `ziban`;
`devloom` carries a native `gate.toml` at its root instead, and the other four
carry no Gate manifest in any form. The gap is therefore **70 repositories
whose gate executes nothing** while their own `.project.json` names the command
that would execute something. The 6 that already bind are `byok-translator`,
`careBridge`, `labrys`, `openpanel`, `therapist-commons` and `vowra`.

**This is not a template problem, and that is the design constraint.** A
template cannot know a sibling repository's verification command. `profile_defaults`
in `src/gate/manifest.rs:110` selects concern *ids* only and never invents a
command, by design, and `ManifestCheck` carries the program/args binding as
project-owned data. The binding is therefore **generated per repository from
that repository's own declaration**, never templated and never defaulted.

### Two pre-existing defects this change measures but does not own

**11 of the 76 manifests do not parse at all.** `actoria`, `argoscope`,
`crossalheart`, `dharmatlas`, `hookit`, `kairion` and `workspace-governance`
carry a top-level `notes:` key; `citylens` and `openTeacherKit` carry `status:`;
`campusMesh` and `trailCrew` carry `evidence_boundary:`. All three are outside
`YAML_FIELDS` (`src/gate/aigate.rs:57`), so the manifest is refused before any
check resolves. Binding a command into one of these is *necessary and
insufficient*: the binding is recorded and inert until the unknown field is
resolved. Resolving it means deleting or relocating prose the owning repository
wrote — an owner decision, not this change's, and left unchecked in
`tasks.md` §4 with the reason recorded.

**4 of the 70 targets carry an untracked `.ai-gate/gate.yaml`.**
`crossalheart`, `dharmatlas`, `hookit` and `trailCrew` report `??` for that path
in `git status`. The workspace constraint is to stop rather than write into a
repository that is dirty in a file the change must change, so these 4 are
excluded from the write and named here. The write set is therefore **66**.

## What

1. **A stable concern id** `declared-verification`, served by the existing
   `project-runtime` text adapter. Exit 0 is `PASS`, a non-zero exit is `FAIL`,
   and a timeout or spawn failure is `REVIEW_REQUIRED`. It joins the
   non-exemptable spine, so no repository can write itself out of the ruling
   with a `not_applicable` reason.
2. **A shared generator** in `workspace-governance/scripts/` that reads each
   repository's `.project.json` and writes that repository's declared command
   into its own `.ai-gate/gate.yaml` under both `checks:` and `commands:`.
   Idempotent, and it refuses rather than inventing a command.
3. **A drift rule** `QUALITY_VERIFICATION_COMMAND_UNBOUND` in
   `workspace_check.py`, ERROR, held off the fleet by a committed constant
   `VERIFICATION_BINDING_AUDIT_ENABLED`, and added to
   `GOVERNANCE_QUALITY_DECLARED_CODES` so the `governance-quality` concern can
   actually see it.

## Non-goals

- Not a fix for the 11 unparseable manifests, and not a decision to delete their
  unknown fields.
- Not a change to how any concern's adapter maps an exit code.
- Not a commitment of 70 repositories' test suites. This change makes the gate
  *run* them; whether they pass is the repositories' business and is measured
  and reported, not suppressed.
- No archive. The change stays active with its residual items unchecked.
