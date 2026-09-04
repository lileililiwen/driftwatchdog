## 1. Runner

- [x] 1.1 Add process-runner types for program, argv, cwd, tags, timestamps, exit status, and bounded captured streams.
- [x] 1.2 Write tests for arbitrary arguments, success, nonzero exit, missing executable, streaming, and output truncation.
- [x] 1.3 Implement child-process execution without shell interpolation and preserve the child's exit code.

## 2. Persistence and commands

- [x] 2.1 Persist every attempted run with Git metadata and captured output according to the configured storage policy.
- [x] 2.2 Implement `run` rendering and structured failure diagnostics.
- [x] 2.3 Implement `list` queries and table output for limit, status, and tag filters.
- [x] 2.4 Implement `top` query/output with empty-state behavior and date/tag filters.

## 3. Verification

- [x] 3.1 Run CLI integration tests using fake cross-platform helper commands.
- [x] 3.2 Verify representative Rust, Python, Node, and .NET command invocations are accepted without language-specific branching.
- [x] 3.3 Run formatting, linting, unit tests, and OpenSpec validation.
