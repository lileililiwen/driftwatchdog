# Design: Bind every declared verification command into the gate that blocks

Change: `bind-declared-verification-command`. Spec-first; BFS → DFS → BFS.

## 1. The binding mechanism, and why it is generated

The ruling is per-repository, and a template cannot satisfy a per-repository
ruling: the value to bind is not a constant anywhere in this workspace, it is
the string each repository wrote into its own `.project.json`. Two existing
facts pin the shape:

- `profile_defaults` (`src/gate/manifest.rs:110`) returns concern **ids** and
  nothing else. It is data, and it never invents a command. That is correct
  and this change does not widen it.
- `ManifestCheck` carries `command: Option<String>` as project-owned data, and
  the YAML form maps `commands.<id>` onto it (`src/gate/aigate.rs:169`).

So the binding is a *write into each repository's own manifest*, and the writer
must read the declaration from the same repository. A template cannot do that
and must not pretend to.

### Decision: the generator lives in `workspace-governance`

`workspace-governance/scripts/bind_verification_command.py`, not in
`driftwatchdog`. Reasons, in order of weight:

1. **The runtime must not know the fleet.** `AGENTS.md` requires that "managed
   Gate tooling [be kept] separate from project runtimes" and that OpenSpec not
   become a dependency of the generic Gate core. A generator that walks 81
   sibling directories is fleet tooling, not a Gate runtime feature.
2. **The fleet policy owner already reads these files.** `workspace_check.py`
   already resolves `.project.json` and `.ai-gate/gate.yaml` for its own audit.
   The rule that finds an unbound repository and the tool that binds it belong
   beside each other, so the two cannot disagree about what "bound" means.
3. **`driftwatchdog` keeps only the contract.** The concern id and the declared
   code are runtime vocabulary; the writer is governance's.

The contract this repository owns is exactly two things: the concern id
`declared-verification`, and the addition of
`QUALITY_VERIFICATION_COMMAND_UNBOUND` to `GOVERNANCE_QUALITY_DECLARED_CODES`.

## 2. The concern id: `declared-verification`

**Shape.** A stable concern id, selected in `checks:` as `true` and bound in
`commands:`, whose value is the repository's declared command verbatim.

**Adapter.** None new. `src/commands/gate.rs:212` dispatches by concern
vocabulary; `declared-verification` is in none of the closed vocabularies
(product-quality, governance-quality, release-gate, capability-conformance,
deployable), so it falls through to `run_adapter(runtime, …)`, the
`project-runtime` **text** adapter. That adapter's mapping is already the one
this ruling wants (`src/gate/adapters.rs:981`): exit 0 → `PASS`, any non-zero
exit → `FAIL`, and a timeout or `sh` spawn failure → `REVIEW_REQUIRED` with
missing evidence. A failing declared command reports `FAIL`. It is not
softened, skipped, or reinterpreted.

**Why not `governance-quality`.** That concern is served by
`src/gate/governance/mod.rs`, which requires the command to emit a governance
report — a JSON object with `findings` and `summary` — and derives status from
*matching findings' levels*, never from the exit code (`mod.rs:20-40`). A
repository's declared verification command is `npm test && npm run build`; it
emits test output, not a governance report. Binding it there would produce
`REVIEW_REQUIRED` for unparseable output on every repository, which is a false
claim about the checker rather than a real result. The two are different
contracts and are kept apart.

**Non-exemptable.** `declared-verification` joins `NON_EXEMPTABLE_CONCERNS`
(`src/gate/manifest.rs:281`) and `NON_EXEMPTABLE_ALIASES`, with one spelling.
Without this, any repository could add
`not_applicable: declared-verification: "we prefer npm run check"` and be green
while its declared command still ran nothing — the exact hole the spine exists
to close. `source-file-size` is the precedent for a concern that needs no
command and is still non-exemptable.

**Idempotence of the runtime half.** The concern id is pure data; adding it
changes no existing plan for any repository that does not select it.

## 3. The generator's contract

`workspace-governance/scripts/bind_verification_command.py`.

- **Reads** `<repo>/.project.json` → `verification.command`.
- **Refuses** rather than inventing: absent `.project.json`, unparseable JSON,
  absent `verification`, absent/blank/non-string `command` — each is a named
  refusal for that repository, and the run continues to the next one. It never
  synthesises a command, never falls back to a profile default, and never
  reuses another repository's command.
- **Writes** into `.ai-gate/gate.yaml`: a `declared-verification: true` entry in
  `checks:` and a `declared-verification: "<declared command>"` entry in
  `commands:`, creating `commands:` if absent. Both are required: a
  `commands.<id>` naming an unselected check is a parse error
  (`src/gate/aigate.rs:186`), so a binding without a selection would be refused.
- **Round-trips minimally.** Only the two entries are inserted; the rest of the
  document is preserved byte-for-byte, including comments and ordering. A
  second run finds the exact command already present and writes nothing, so the
  run is a no-op.
- **Refuses to overwrite a differing binding.** If `commands.declared-verification`
  exists with a value that is not the declared command, the repository is
  reported as a conflict and left untouched. An operator's hand-written command
  is never clobbered by a generated one.
- **Dry run by default.** `--apply` is required to write; without it the tool
  reports the plan. Fleet writes are therefore always an explicit act.
- **No `rm`.** Deletion in tests and tooling uses `Path.unlink(missing_ok=True)`
  and `shutil.rmtree(..., ignore_errors=True)`.

## 4. The drift rule, and the ordering constraint it exposes

`QUALITY_VERIFICATION_COMMAND_UNBOUND`, ERROR, in `workspace_check.py`: a
repository whose `.project.json` declares a non-empty `verification.command` and
whose `.ai-gate/gate.yaml` binds no command for it is a finding.

**The ordering is a real constraint, not a formality.** The `governance-quality`
concern filters report findings through the closed allow-list
`GOVERNANCE_QUALITY_DECLARED_CODES` (`src/gate/governance/mod.rs:73`) at the
filter, and treats the checker's exit 1 as an *expected* exit rather than a
status. So while the new code sits outside that list, a checker that emits only
this code exits 1 with a report the concern filters to nothing, and the concern
would report **PASS while the checker fails** — the "checker that reports OK
without checking" failure this workspace keeps finding. The code must therefore
be declared in the allow-list for the rule to gate anything at all, and the
declaration must land for the rule to be visible. This change lands both in one
commit, in this order: the declared code first, then the emitting rule.

The precedent is the same two-step: `spec-purpose-backfill-gate` shipped the
emitting rule and recorded that D18 was *not* satisfied because the code was not
yet in the allow-list; `declare-spec-purpose-placeholder-code` then closed that.

## 5. The fence

`VERIFICATION_BINDING_AUDIT_ENABLED`, a committed repo-relative constant in
`workspace_check.py`, mirroring `SPEC_PURPOSE_AUDIT_ENABLED` exactly: never an
environment variable, no skip flag, no exemption registry, one mechanism. The
rule's level is fixed at **ERROR** below the fence and is never softened to
WARNING to achieve the hold — softening the level would make the rule
invisible in a second, dishonest way.

**Shipped state: `False`.** The rule is fully implemented, tested, and gated on
one committed constant. The alternative, `True`, turns the audit into an ERROR
for every repository that declares a command and binds none — which at the
moment of writing is 70, of which 4 are excluded here for being dirty and 11
are unparseable, so `True` today reports red for reasons this change does not
own. Flipping the constant is the owner's single decision, and a test proves
that flipping it changes the result.

## 6. The consequence, measured rather than hidden

Binding a command means the gate starts running each repository's test suite.
Repositories whose suites are red go red. That is the gate working, and no
failing command is suppressed, skipped, or softened to keep a gate green. The
measured green / red / failed-to-start split across the 70 is reported in
`HANDOFF.md` from a real run, with the red repositories named.

`sh -c` returns 127 for a command it could not execute, and the text adapter
maps *any* non-zero exit to `FAIL` (`src/gate/adapters.rs:981`), so a missing
toolchain and a genuinely failing test suite are both `FAIL` to the gate. The
report separates them anyway, because "the toolchain is not installed" and "the
tests fail" are different facts for the owner, and collapsing them would hide
the second behind the first.

## 7. Compatibility

- A repository that does not select `declared-verification` sees no change: the
  id is inert data until selected.
- `NON_EXEMPTABLE_ALIASES` grows from 5 entries to 6; the manifest tests that
  pin the alias table are updated to the new count. This *widens* the spine.
- The concern runs through the existing `project-runtime` adapter, so the
  60 s `CHECK_TIMEOUT_MS` and 1 MiB output cap apply unchanged.
- No new dependency, no new execution path, no network call.

## 8. Security

The declared command is written into a manifest and later executed by `sh -c`
by the Gate runtime. It is the repository's own string, from its own file, and
it is already the string the repository documents as its verification command —
this change does not widen what a repository can ask the gate to run. The
generator refuses to write a command it did not read from that repository, and
refuses to overwrite a differing one, so no command can be introduced by the
generator itself. Redaction of command output is the adapter's existing
`redact_secrets` path and is unchanged.
