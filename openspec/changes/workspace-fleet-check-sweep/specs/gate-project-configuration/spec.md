## ADDED Requirements

### Requirement: Both manifest forms declare one canonical manifest

The business `.ai-gate/gate.yaml` form and the native `gate.toml` form SHALL
convert the same declarations into the same canonical manifest, so that a
manifest written in either form produces the same `manifest_digest` for the same
declared checks, commands, exemptions, policies, and timeouts. No field of the
canonical form SHALL record which of the two forms carried the declaration,
because the digest is what a consumer compares to decide whether a manifest
moved, and a form label would make the same declarations hash differently.

This requirement is proposed by the carried residue item `1.1` — the divergence
is real and recorded, and the runtime does **not** satisfy it yet. It states the
target, so that the fix is reviewable against a written contract rather than
against a diff; it is not a claim that the behaviour ships today.

#### Scenario: The same declarations in either form

- **WHEN** the same checks, commands, exemptions, policies and per-check
  timeouts are declared in a `.ai-gate/gate.yaml` manifest and in the equivalent
  native `gate.toml` manifest
- **THEN** both resolve to the same canonical manifest and the same
  `manifest_digest`, because the two forms are two spellings of one declaration
  and not two declarations

#### Scenario: The form a declaration was read from is not part of it

- **WHEN** a manifest is declared through the business YAML form
- **THEN** no field of its canonical form records that it came from the YAML
  form rather than the native form, so rewriting the same declarations in the
  other form does not change the digest

#### Scenario: The equivalence holds for every field both forms declare

- **WHEN** both manifest forms declare `not_applicable`, `binding_policy` and a
  per-check `timeout_ms` for the same checks
- **THEN** their canonical manifests still agree, so the validation rules those
  fields are subject to are proven once against one manifest rather than
  separately against two