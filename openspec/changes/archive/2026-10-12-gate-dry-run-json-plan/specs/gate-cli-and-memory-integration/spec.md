## ADDED Requirements

### Requirement: Emit a machine-readable rehearsal document for a dry run

`driftwatch gate --dry-run --format json` MUST emit one parseable JSON
document on stdout, composed from the resolved plan alone. The dry run MUST
select its output format from the same `--format` value a real run uses, so
that no format value is accepted and then ignored.

The document MUST declare itself as a rehearsal through all three of: a root
`dry_run` field that is `true`, a root document-kind field whose only value
this producer emits is `gate-plan-rehearsal`, and a root `contract` field
carrying a versioned constant in the shape already used by
`driftwatch-checker/0.1.0` and `platform.gate-result/1.0.0`. The declaration
MUST come from a single named constant, and the producer MUST NOT introduce a
second spelling such as `schema_version`, `$schema` or `contract_version`.

The document MUST carry the information a rehearsal exists to show: the
resolved plan (every check id, whether it is required or optional, whether it
is exempt, which manifest layer selected it, where its command would come
from, and its effective execution deadline including the default), the
resolved blocking policy, the default timeout, the `manifest_digest`, the
`rule_pack_version`, the plan's contract version, the project profile, the
execution-mode marker, the declared exemptions, the unbound required checks,
the exemption budget with its verdict, the explicitly unscheduled concerns,
the context providers, the `source_size` policy and the `governance-quality`
code scope.

The composition MUST be pure: no child process, no database access, no clock
read, no filesystem write, and no state mutation of any kind.

The existing `--format human` dry run MUST remain byte-identical, and the real
`--format json` and `--format human` documents of a real run MUST remain
byte-identical. `--format platform` and its refusal set MUST be untouched.

#### Scenario: A rehearsal in JSON parses and declares what it is

- **WHEN** `driftwatch gate --dry-run --format json` is invoked on a project
  with a Gate manifest
- **THEN** stdout is one parseable JSON document whose `dry_run` is `true`,
  whose document-kind field is `gate-plan-rehearsal`, and whose `contract`
  names the versioned rehearsal contract constant

#### Scenario: The human dry run is unchanged

- **WHEN** `driftwatch gate --dry-run --format human` is invoked
- **THEN** stdout is byte-identical to the human plan render the same release
  produced before this format existed

#### Scenario: A real run's documents are unchanged

- **WHEN** `driftwatch gate --format json` and `driftwatch gate --format
  human` are invoked without `--dry-run`
- **THEN** each emits byte-identical output to the same invocation before
  this change, and the JSON document still carries its verdict fields

#### Scenario: Every planned check is described, none is scored

- **WHEN** the rehearsal document lists a planned check
- **THEN** the row carries its id, required flag, exemption flag, selecting
  layer, command source and effective timeout, and carries no status,
  severity, diagnostic, finding or outcome

### Requirement: A rehearsal MUST NOT be readable as a verdict

A rehearsal document describes a **plan**. It MUST NOT carry a field that
states the outcome of execution, and it MUST NOT reuse the key names the real
`--format json` document uses for its verdict.

Specifically the document MUST NOT contain, at the root or at any depth
inside any object or array, any of: `status`, `result`, `results`, `passed`,
`blocked`, `outcome`, `aggregate`, `exit_code`, `gate_status`, `ok`, `success`,
`failures`, `pending_reviews`. This is a closed list, enforced mechanically by
a recursive walk over the emitted document, not by review.

The document MAY carry a plan-level prediction of whether the resolved plan
would block, because a rehearsal that cannot answer that question is not
worth running. The prediction MUST be labelled as a prediction, MUST NOT be
named `blocked`, and MUST be a sufficient-but-not-necessary condition: it
MAY be `true` when the resolved plan alone already forces a blocking outcome
regardless of what every bound command does, and it MUST be `null` — never
`false` — when the plan alone cannot decide, because a false prediction would
claim that nothing ran and passed.

A `would_block` value of `false` MUST be unreachable in production code.

#### Scenario: No verdict field at any depth

- **WHEN** the rehearsal document is parsed and every key at every depth is
  inspected
- **THEN** no key is any of the closed deny-set names

#### Scenario: A rehearsal never predicts a pass

- **WHEN** the rehearsal document is emitted for any manifest
- **THEN** `blocking_prediction.would_block` is `true` or `null`, and never
  `false`

#### Scenario: Plan-forced blocking is stated as a prediction

- **WHEN** the resolved plan schedules at least one required check that binds
  no command and declares no exemption, or declares an exemption budget that
  is over cap or exempts a never-exempable concern
- **THEN** the document's `blocking_prediction` has `kind` of
  `plan-forced-block`, `would_block` of `true`, and `reasons` naming each
  forcing check and the rule that forces it

#### Scenario: An undecidable plan is undecidable, not passing

- **WHEN** every required check binds a command and the exemption budget is
  within its allowance
- **THEN** the document's `blocking_prediction` has `kind` of
  `undetermined-before-execution`, `would_block` of `null`, and no reasons

### Requirement: A rehearsal persists nothing

A rehearsal MUST remain side-effect-free. It MUST NOT create a `gate_runs`
row, MUST NOT write any file, MUST NOT execute any check, and MUST NOT journal
anything. Its exit code MUST be `0` in every `--format` value, independently
of what the plan it renders would block on, because the gate contract's
nonzero-exit condition is defined over a blocking failure or an unresolved
blocking review **observed by a run**, and a rehearsal observes neither.

#### Scenario: No gate run row is created

- **WHEN** `driftwatch gate --dry-run` is invoked in any `--format` value and
  the project's local database is inspected afterwards
- **THEN** the `gate_runs` table is empty

#### Scenario: The exit code stays zero for a plan that would block

- **WHEN** `driftwatch gate --dry-run --format json` is invoked on a project
  whose real run would be blocked by an unbound required check
- **THEN** the command exits `0`, and the document's blocking prediction, not
  the exit code, is where the plan's own answer is recorded