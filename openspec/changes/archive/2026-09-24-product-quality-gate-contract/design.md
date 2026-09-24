# Design: Product-quality Gate contract

## Ownership and contract identity

Driftwatchdog owns generic concern IDs, result normalization, aggregation, and
machine-readable Gate output. Workspace Governance owns the executable source
scanner. Project repositories own the command binding and source policy. The
contract is language-agnostic and does not import OpenSpec, Workspace
Governance, or any provider SDK.

The canonical concern IDs are `product-code-boundary` and
`placeholder-threshold`. Their result producer is the project command, while
Driftwatchdog records `source: project-command` and the manifest/rule-pack
identity already used by Gate runs.

## Command/report binding

A selected concern with a `commands` binding executes through the existing
`project-runtime` adapter using the project root as working directory. The
command receives no hidden language assumptions. A checker may emit the
versioned Workspace Governance JSON envelope; the generic adapter must also
preserve stderr and exit-code evidence when the output is not parseable.

Required mapping:

| Command outcome | Gate result | Aggregate behavior |
| --- | --- | --- |
| exit 0, valid report status PASS | PASS | no block |
| exit 1, valid FAIL report | FAIL | block when selected required |
| exit 2, valid REVIEW_REQUIRED report | REVIEW_REQUIRED | block when policy blocks review |
| missing command for required concern | REVIEW_REQUIRED | block |
| malformed report or contradictory exit/status | REVIEW_REQUIRED | block |
| selected optional concern without command | NOT_APPLICABLE | non-blocking, visible |

The command exit code remains authoritative when it contradicts a report; a
non-zero command cannot claim PASS. Existing generic check commands retain
their behavior. The new IDs are data-level contract values, not hard-coded
execution branches.

## Profiles and manifests

Built-in language profiles may select the two concerns as required defaults,
but selection does not invent a command. A project-defined profile can select
either concern and bind a command in `gate.toml` or `.ai-gate/gate.yaml`.
The resolved plan renders selected, not-scheduled, optional, and unbound
concerns so absent migration is visible. A profile that has no declared product
source roots must either disable the concerns explicitly or produce the
project command's `REVIEW_REQUIRED`; it cannot silently use `NOT_APPLICABLE`.

## JSON and persistence

The existing Gate JSON envelope carries the result IDs, status, severity,
diagnostic, remediation, evidence, and missing-evidence fields. No new
database columns are needed. A persisted run remains auditable through the
manifest digest, rule-pack version, command result, and bounded result JSON.

## Verification and compatibility

Unit tests cover the mapping table and contradiction rules. Integration tests
run temporary scripts that emit pass, fail, review, malformed, and no-output
cases through the real `gate` command, asserting exit codes, human/JSON parity,
persistence, and redaction. Existing profiles and checker-only `driftwatch
check` behavior remain unchanged. The Gate binary is not required to be
installed in Workspace Governance's own checker tests.
