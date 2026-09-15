# Proposal: Generic context providers

## Why

Gate evaluation needs change context, but not every project uses OpenSpec. The
core should consume generic context documents and providers while allowing
OpenSpec, Git, Gherkin, Markdown, or future systems to integrate independently.

## What Changes

- Define a generic context-provider interface and bounded context documents.
- Add Git and project-file context providers.
- Add OpenSpec as an optional adapter, without making it a core dependency.

## Capabilities

### New Capabilities

- `generic-context-providers`: specification-system-neutral Gate context.

### Modified Capabilities

## Impact

Affects gate planning, evidence inputs, configuration, CLI options, and optional
OpenSpec parsing. Depends on gate contract, project configuration, and evidence.

## Non-Goals

- No OpenSpec lifecycle management.
- No automatic mutation of specs or tasks.
- No requirement that every project have a specification system.
