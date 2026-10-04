# Design: Every gate check declares its own timeout, and shows it

Change: `per-check-gate-timeout`. Spec-first; BFS → DFS → BFS.

## 1. The field, and why `Option<u64>` and not a defaulted `u64`

`ManifestCheck.timeout_ms: Option<u64>` (`src/gate/manifest.rs:661`),
`#[serde(default)]`, matching `CheckerEntry.timeout_ms`
(`src/project/config.rs:81`) field for field.

`Option` is not a style preference here — it is what makes "absent" and
"declared to be the default" different things, which is the only way a
canonical manifest digest stays stable. `ResolvedGatePlan.manifest_digest`
is computed over `toml::to_string_pretty(manifest)`, and the TOML
serializer omits `None`. A defaulted `u64` would emit `timeout_ms = 60000`
into every canonical document and change the digest of all 76 existing
manifests, for a field none of them declared. `Option` keeps the
canonical form byte-identical for every manifest that does not use the
key, exactly as `SourceSizePolicy::is_default` keeps `max_lines` out of
the canonical form when unset. A test pins this.

`ManifestCheck` is `#[serde(deny_unknown_fields)]`, so the native
`gate.toml` form gets the key for free once the field exists — and a
typo in the native form is a parse error with a `did you mean` hint,
which is the behaviour the rest of the struct already has. `timeout_ms`
joins the module's `KNOWN_FIELDS` so the hint can reach it.

## 2. Default: 60s, unchanged, deliberately

`CHECK_TIMEOUT_MS = 60_000` stays the default and moves nowhere. An
absent `timeout_ms` resolves to exactly what it resolves to today, so
all 76 `.ai-gate/gate.yaml` manifests and every native `gate.toml` keep
current behaviour with zero edits.

The alternative — raising the global default to accommodate
`alltools-platform` at >15 min — was rejected in `proposal.md`: it
widens the window for the two near-miss repositories without guaranteeing
either of them margin. The default is a decision with a blast radius of
every check in the fleet, and it stays where the fleet has already
measured it. **This change ships the field and changes no default.** A
default that changes silently is a default nobody reviewed.

## 3. Validation: two refusals, both at parse time

Both live in `manifest::validate` (`src/gate/manifest.rs:935`), which is
the single owner for both manifest forms — see §5.

- `timeout_ms = 0` → `ManifestError::InvalidCheckTimeout { id, reason }`,
  wording mirroring `src/project/config.rs:239` ("use a positive value or
  omit the key"). Zero is refused rather than treated as "no timeout"
  because the difference between "0 ms" and "60s" is the difference
  between a check that always fails and a check that passes, and no
  reader of a manifest should have to guess which was meant.
- `timeout_ms > MAX_CHECK_TIMEOUT_MS` → the same variant, naming the cap
  and the accepted range.

**`MAX_CHECK_TIMEOUT_MS = 3_600_000` (1h).** One hour is longer than any
gate check in this workspace needs — the slowest measured is
`alltools-platform` at >15 min — and it is short enough that a hung
command cannot hold a work session open indefinitely. Without a bound,
"raise it to 24h" becomes the next footgun and the value of the field
stops meaning anything. The error names the cap, so the fix is a number
the operator can read rather than a hunt for a constant. The style
mirrors `MAX_MAX_LINES` (`src/gate/manifest.rs:562`): a named public
constant next to the field it bounds, enforced in validation, and quoted
in the error.

A single `InvalidCheckTimeout { id, detail }` variant serves both
refusals rather than two variants, because the *action* is identical
(read the number, fix the number) and a separate variant per bound would
double the match arms for one sentence of guidance.

## 4. Visibility: the resolved timeout is on the plan

`ResolvedGatePlan.check_timeouts: BTreeMap<String, u64>` (`#[serde(default)]`,
so an older serialized plan still deserializes), populated in `resolve`
for every *selected* check — declared value, else the 60s default.

`render_plan` prints a `check timeouts:` block, one line per check, so
`--dry-run` shows the effective deadline for every check that will run.
This is the point of the change, not a nicety: a timeout nobody can see
is indistinguishable from a slow test, which is why one hardcoded
constant stayed invisible across 76 manifests and shipped two repos that
cannot pass. Same reasoning as ruling D18 applied to time — the value an
operator must reason about has to be on the surface before anything runs.

## 5. Both manifest forms produce the same field — one owner for the rule

`src/gate/aigate.rs` gains a per-check `timeouts:` mapping
(`id -> milliseconds`) alongside the existing `commands:` mapping. It is
converted straight onto `ManifestCheck.timeout_ms` during check
construction, so:

- the YAML form and the native TOML form produce an **identical**
  `ManifestCheck` for the same declarations, and
- every rule in §3 runs exactly once, in `manifest::validate`, for both
  forms — it is never duplicated into the YAML parser.

`YAML_FIELDS` is the **top-level** key list; `timeouts` is added there
because `timeouts` is a new top-level mapping. The per-check key names
inside it are `commands`' vocabulary, not a second schema. The module's
own header already states this single-owner pattern for
`not_applicable` and `binding_policy`, and this change follows it rather
than inventing a second validation path.

A `timeouts.<id>` naming a check that is not selected is a parse error,
mirroring the existing `commands.<id>` rule — a timeout bound to nothing
is a value that will never be read.

## 6. Timeout stays distinguishable from failure

**Verified in the code, and the proposal's claim holds.** `run_text_adapter`
(`src/gate/adapters.rs:936`) checks `run.timed_out` *before* the exit-code
match and returns `infra_result(..., &AdapterError::Timeout { .. })`,
which is `GateStatus::ReviewRequired` with `missing_evidence` extended by
`{tool_id}:timeout` (adapters.rs:952-960). The exit-code branch, reached
only when the child finished, maps exit 0 → `PASS` and any non-zero →
`GateStatus::Fail`. So a killed-mid-run check and a genuinely failing
check already produce different statuses with different evidence, and
this change does not touch that mapping — it only changes the deadline.

This matters for the negative control in §7: raising a timeout from 60s
to 600s turns a `REVIEW_REQUIRED` timeout into whatever the command
actually reports, and it is only meaningful as a fix if the timeout
status stays the honest one it is today.

## 7. Negative control

A field nobody can trip is a field nobody has tested.

1. **Default path.** A manifest with no `timeout_ms` resolves to 60000 on
   the plan. The real 60s wall-clock wait is not affordable in a unit
   test, so the test pins the *resolved value* and the fact that
   `AdapterInput.timeout_ms` is built from the plan — and the wiring is
   covered end-to-end by control 2, which uses the same code path with a
   value small enough to actually wait for. The two together cover what
   a 60s wait would prove, without spending a minute of wall clock to
   prove it a fourth time.
2. **Honored, not ignored.** A manifest declaring `timeout_ms = 1000`
   bound to `sleep 5` actually runs, is actually killed, and actually
   reports the timeout as `REVIEW_REQUIRED` with `project-runtime:timeout`
   missing evidence. If the field were ignored this test would hang for
   5s and then report `PASS`; it would fail. That asymmetry is the
   control.

Neither control weakens a test, filters output, or skips a case.

## 8. Compatibility

- **Zero edits to 76 manifests.** The field is additive and `Option`;
  absent means the current 60s.
- **Digest stability.** The canonical TOML omits a `None`
  `timeout_ms`, so `manifest_digest` is unchanged for every manifest
  that does not declare one. Pinned by a test, because a silent digest
  change across 76 manifests would be exactly the kind of invisible
  consequence this change is arguing against.
- **No new execution path.** The same `project-runtime` text adapter,
  the same 1 MiB output cap, the same process-group kill. Only the
  `timeout_ms` value on an existing `AdapterInput` field changes.
- **No new dependency, no network call.**
- `ResolvedGatePlan.check_timeouts` is `#[serde(default)]`, so a plan
  serialized by an older build still deserializes.
- `to_gate_run_json` and the persisted run are untouched: the resolved
  map is on the *plan*, not on a `GateResult`, so no run-record schema
  changes.

## 9. Security

A timeout is a deadline, not an authority. It cannot widen what a
repository may ask the gate to run — the command still comes from the
repository's own manifest and is still executed by `sh -c` under the
existing redaction and output-cap paths. The cap is a bound in the
*safe* direction: it prevents an unbounded wait, and it is the only
value in this change that a repository could use to make the gate hold a
process longer, so it is validated at parse time rather than trusted.
`MAX_CHECK_TIMEOUT_MS` is `pub` and named in the error, so an operator
who needs a different ceiling changes one constant and one documented
decision — not a scattered literal.
