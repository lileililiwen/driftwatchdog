## 1. BFS: Impact and Structure

- [x] 1.1 Map current `manifest_path`/`load`/`resolve`/`render_plan` callers, gate command, doctor gate-history, and context selection.
- [x] 1.2 Define the `.ai-gate/gate.yaml` schema, TOML precedence, runtime guard, and blocking-list mapping.
- [x] 1.3 Define domain-profile acceptance (`[profiles.<name>]` in TOML, declared selection in YAML) and identity/digest rules.

## 2. DFS: Requirement Implementation

- [x] 2.1 Add a maintained YAML parser dependency and a strict `YamlGatePolicy` model with actionable unknown-field errors.
- [x] 2.2 Implement YAML→`GateManifest` conversion (checks, commands, blocking, project_commands, contexts, rule_pack) with empty-command and unknown-concern rejection before execution.
- [x] 2.3 Extend `manifest_path`/`load` to resolve `.ai-gate/gate.yaml` after the TOML sources and add a runtime-filtered loader for the gate command.
- [x] 2.4 Add `[profiles.<name>]` support so a project-defined profile resolves to its declared concerns; keep unknown-profile rejection otherwise.
- [x] 2.5 Wire `driftwatch gate` to report a foreign-runtime policy without execution or persistence.
- [x] 2.6 Add unit/integration tests: valid YAML, precedence, foreign runtime, unknown field, blocking list, command binding, domain profile, and dry-run identity.

## 3. BFS: Regression and Completion

- [x] 3.1 Re-check `gate.toml` behavior, `driftwatch check`, context provider selection, and doctor gate-history wording.
- [x] 3.2 Confirm no new network access, no rule-pack evaluator, and deterministic digests for equivalent TOML/YAML plans.
- [x] 3.3 Update README Gate section with the `.ai-gate/gate.yaml` example and precedence.
- [x] 3.4 Run `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo deny check`, and strict OpenSpec validation.
