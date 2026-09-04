#!/usr/bin/env sh
# tests: checksum manifest generation. Asserts the production
# `scripts/checksum.sh` emits a manifest that `sha256sum -c` accepts.

test_checksum_manifest() {
    stage=$(mktemp -d) || return 1
    trap 'rm -rf "$stage"' EXIT INT TERM

    # Build a minimal but realistic set of archives.
    for t in linux-x86_64 linux-arm64 darwin-x86_64; do
        pkg="driftwatchdog-7.7.7-$t"
        mkdir -p "$stage/$pkg"
        printf 'hi %s\n' "$t" > "$stage/$pkg/driftwatchdog"
        chmod 0755 "$stage/$pkg/driftwatchdog"
        (cd "$stage" && tar -czf "driftwatchdog-7.7.7-$t.tar.gz" "$pkg") || return 1
    done

    manifest=$(sh "$repo_root/scripts/checksum.sh" "$stage" 7.7.7) || return 1
    [ -f "$manifest" ] || { printf 'manifest not created: %s\n' "$manifest" >&2; return 1; }

    # Three lines, each with two spaces between hash and filename.
    line_count=$(wc -l < "$manifest" | tr -d ' ')
    [ "$line_count" = "3" ] || { printf 'expected 3 lines, got %s\n' "$line_count" >&2; return 1; }

    if ! awk 'NF != 2 || length($1) != 64 { exit 1 }' "$manifest"; then
        printf 'manifest line shape is wrong\n' >&2
        cat "$manifest" >&2
        return 1
    fi

    # Files are listed in sorted order for stability.
    sorted=$(awk '{print $2}' "$manifest" | sort)
    actual=$(awk '{print $2}' "$manifest")
    [ "$sorted" = "$actual" ] || { printf 'manifest not sorted\n%s\n' "$actual" >&2; return 1; }

    # The manifest must be usable by sha256sum -c from within the stage.
    (cd "$stage" && sha256sum -c "driftwatchdog-7.7.7-checksums.txt") || return 1
}
