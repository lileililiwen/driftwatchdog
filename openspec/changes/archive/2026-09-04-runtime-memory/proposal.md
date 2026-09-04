## Why

Developers need to run their existing test/build commands without changing language-specific workflows while retaining enough context to recognize repeated failures.

## What Changes

Add a cross-platform child-process runner and `run`, `list`, and `top` commands. Persist command metadata, output excerpts, status, tags, duration, and Git context, while preserving the wrapped command's exit status and terminal experience as far as practical.

## Capabilities

### New Capabilities
- `runtime-memory`: execute arbitrary project commands and query recorded runs and recurring failures.

### Modified Capabilities
- `project-foundation`: use its database and Git/configuration contracts.

## Impact

Adds process spawning, stdout/stderr capture, streaming, run persistence, and terminal table rendering. It supports arbitrary command ecosystems; no Rust-specific parser is required.
