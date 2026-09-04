## Why

Recurring runtime failures and spec alerts are valuable separately, but the product's distinctive value is showing plausible relationships and giving AI coding agents durable context.

## What Changes

Add deterministic heuristic correlation, persisted correlation records, manual `link`/`unlink`, and `report --ai` output with cautious language and actionable investigation instructions.

## Capabilities

### New Capabilities
- `correlation-and-ai-context`: relate runtime bug fingerprints to drift alerts and generate AI-oriented context.

### Modified Capabilities
- `fingerprinting-and-retention`: expose bug context to correlation and AI reports.
- `checker-and-drift-alerts`: expose normalized alerts to correlation and AI reports.

## Impact

Adds tokenization/similarity logic, correlation persistence, CLI link management, and a stable Markdown context format. It does not call an LLM or claim causal root-cause detection.
