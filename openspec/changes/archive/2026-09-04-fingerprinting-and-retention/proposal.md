## Why

Raw locations, durations, UUIDs, and process details change between executions even when the logical bug is the same. Driftwatch needs stable failure memory without retaining unbounded logs.

## What Changes

Add configurable generic normalization, SHA-256 fingerprinting, bug/occurrence aggregation, `show` and Markdown `report`, and age-based `gc` that removes bulky streams while preserving long-term statistics.

## Capabilities

### New Capabilities
- `fingerprinting-and-retention`: normalize failures, group logical bugs, report history, and garbage-collect bulky evidence.

### Modified Capabilities
- `runtime-memory`: attach normalized bug identities to failed runs.

## Impact

Adds normalization rules for arbitrary text and configurable ignore patterns. Language-specific profiles are optional enhancements; generic behavior is the compatibility baseline.
