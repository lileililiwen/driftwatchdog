## 1. Runner

- [ ] 1.1 Add process-runner types for program, argv, cwd, tags, timestamps, exit status, and bounded captured streams.
- [ ] 1.2 Write tests for arbitrary arguments, success, nonzero exit, missing executable, streaming, and output truncation.
- [ ] 1.3 Implement child-process execution without shell interpolation and preserve the child's exit code.

## 2. Persistence and commands

- [ ] 2.1 Persist every attempted run with Git metadata and captured output according to the configured storage policy.
- [ ] 2.2 Implement `run` rendering and structured failure diagnostics.
- [ ] 2.3 Implement `list` queries and table output for limit, status, and tag filters.
- [ ] 2.4 Implement `top` query/output with empty-state behavior and date/tag filters.

## 3. Verification

- [ ] 3.1 Run CLI integration tests using fake cross-platform helper commands.
- [ ] 3.2 Verify representative Rust, Python, Node, and .NET command invocations are accepted without language-specific branching.
- [ ] 3.3 Run formatting, linting, unit tests, and OpenSpec validation.
