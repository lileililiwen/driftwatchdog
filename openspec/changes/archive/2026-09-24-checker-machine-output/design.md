# Design: Machine-readable check output

## Ownership and boundaries

The checker runner already produces per-checker outcomes (status, parsed
alerts, protocol errors) and persistence writes them. This change adds a
renderer of that existing result structure; it must not alter execution,
storage, or the human format.

## Envelope

```json
{
  "contract": "driftwatch-checker/0.1.0",
  "tool": "driftwatchdog",
  "version": "<cli version>",
  "project": "<resolved project root, or omitted outside a project>",
  "generated_at": "<rfc3339>",
  "checkers": [
    {"name": "architecture",
     "status": "ok|alerting|protocol-error|failed|timeout",
     "alerts": [ {"severity": "warning", "message": "...",
                   "source": "...", "symbol": "..."} ]}
  ],
  "summary": {"total": 3, "ok": 1, "alerting": 1, "errors": 1,
               "alerts": 4}
}
```

- Alert objects reuse the exact wire vocabulary of the existing checker
  protocol (the same values persisted in alert snapshots), so downstream
  consumers need one parser, not two.
- `checkers[]` preserves declaration order from `driftwatch.toml`.
- Bounded strings: messages are truncated to the same limit the storage
  layer applies, with the existing truncation note, so JSON cannot exceed
  what the DB would have stored.
- stdout carries only the document; all human chatter/diagnostics stay on
  stderr in JSON mode.

## Semantics invariants

- `--format json` must not change which checkers run, isolation of a
  broken checker, exit status, or what is persisted (except `--dry-run`
  persists nothing in either format, unchanged).
- Unknown future fields are the responsibility of consumers; the sibling
  protocol's forward-compat rule (ignore unknown top-level fields) is the
  documented guarantee, hence no content-version churn for additions.
- No project (outside a driftwatch project dir) → same error path as
  human format, non-zero exit, no partial document.

## Module shape

CLI gains `--format` on `CheckArgs` (ValueEnum like GateFormatArg);
`checker::runner` returns the result structure it already has; a new
`checker::report::json` renders the envelope from that structure; human
rendering moves to call the same structure without copying logic. No
storage changes.

## Verification

- Unit: envelope rendering from synthetic checker results (every status,
  truncation, empty-alerts document = `alerts: []` present).
- CLI integration: json vs human parity of outcome; `--dry-run --format
  json` writes no rows (DB row-count assertion); exit-code equality tests.
- Docs: README example block + note that gate JSON covers gate-manifest
  projects while checker JSON covers checker-only projects.
