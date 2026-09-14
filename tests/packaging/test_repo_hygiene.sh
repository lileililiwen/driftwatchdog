#!/usr/bin/env sh
# tests: repo hygiene. The repo root must not track driftwatch runtime
# state: `.driftwatch/` and `driftwatch.toml` are local-only and covered
# by .gitignore. `driftwatch.toml.example` is the tracked starter.

test_repo_hygiene() {
    # Neither path may be tracked.
    if git -C "$repo_root" ls-files | grep -qx '.driftwatch/state.db'; then
        printf 'stray tracked file: .driftwatch/state.db\n' >&2
        return 1
    fi
    if git -C "$repo_root" ls-files | grep -qx 'driftwatch.toml'; then
        printf 'stray tracked file: driftwatch.toml\n' >&2
        return 1
    fi
    # Both must be ignored so accidental local runs stay untracked.
    git -C "$repo_root" check-ignore -q .driftwatch/state.db \
        || { printf '.driftwatch/ not ignored\n' >&2; return 1; }
    git -C "$repo_root" check-ignore -q driftwatch.toml \
        || { printf 'driftwatch.toml not ignored\n' >&2; return 1; }
    # The example starter must stay tracked.
    git -C "$repo_root" ls-files | grep -qx 'driftwatch.toml.example' \
        || { printf 'driftwatch.toml.example not tracked\n' >&2; return 1; }
}
