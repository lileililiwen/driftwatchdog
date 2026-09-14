## Approach
- Config: `#[serde(deny_unknown_fields)]` on `Config`/`CheckerEntry`; validator returning `ConfigError::{DuplicateName, EmptyCommand, ZeroTimeout, WorkingDirEscape, UnknownField}` with `did-you-mean` hints; `working_dir` resolved against canonical project root, rejected if it escapes.
- Protocol: `#[serde(default, deny_unknown_fields)]` removed at top level → allow + ignore unknowns; `alerts` field required (`Option` → missing is `ProtocolError::MissingAlerts`); add `MAX_ALERTS=10000`, `MAX_MESSAGE_BYTES` guards.
- Check command: dry-run prints parsed alert count without persisting; malformed output maps to `Status::Failed` with truncated diagnostic, never success.

## Non-goals
- No new checker output formats (JSON only); no network checkers.
