## Approach
- Migrations: wrap tracking-table creation + version read + DDL in one transaction (`IMMEDIATE`), use `ADD COLUMN IF NOT EXISTS`-equivalent (SQLite: check `PRAGMA table_info` first) so reruns are safe; add a concurrent-open test.
- Root: canonicalize with `std::fs::canonicalize` (fallback to non-canonical on error), open DB with `OpenFlags` instead of check-then-use, return explicit error past max depth instead of silent fallback.
- Git: single `git` invocation budget (timeout ~2s, kill group), `dirty: None` when git missing/times out; never report clean on unknown.
- Doctor: open DB read-only for checks (`SQLITE_OPEN_READ_ONLY` + no migrations); writability probe via atomic `create_new` temp file cleaned in `Drop`; version probes with argv parsing (no shell split) + timeout.
- Tags: escape `%_\` in LIKE, or switch to exact match via a tags join; add `auth` vs `oauth` regression test.
- Occurrences: `occurrence_count+1` always, `first_seen=MIN`, `last_seen=MAX`; backdated-seen test.
- Report: single aggregate query for trend cells, propagate DB errors (no `unwrap_or(0)`), timezone-aware day bucketing; export prints `warning: capped at 100000 rows` to stderr when hit.

## Non-goals
- No new schema version unless DDL demands it; no distributed locking.
