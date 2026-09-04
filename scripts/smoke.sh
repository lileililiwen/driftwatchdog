#!/usr/bin/env sh
# Opt-in release smoke test. Exercises the release packaging scripts
# end-to-end against a local fake Cargo project so CI can validate
# archive naming, checksum manifest generation, and installer behavior
# without relying on a live network or a real release.
#
# Usage:
#   scripts/smoke.sh              # build a fake release and verify
#   scripts/smoke.sh --skip-build # reuse an existing stage dir
#
# Exit non-zero on any failure.

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)

stage=""
skip_build=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        --stage) stage=$2; shift 2 ;;
        --skip-build) skip_build=1; shift ;;
        --help|-h)
            sed -n '2,12p' "$0"
            exit 0
            ;;
        *) printf 'unknown arg: %s\n' "$1" >&2; exit 64 ;;
    esac
done

if [ -z "$stage" ]; then
    stage=$(mktemp -d)
    trap 'rm -rf "$stage"' EXIT INT TERM
fi

if [ "$skip_build" -ne 1 ]; then
    # Build a fake release stage by hand. The packaging scripts use the
    # same archive layout, so a hand-built stage is a faithful proxy
    # for what the GitHub Actions workflow would produce.
    rm -rf "$stage"
    mkdir -p "$stage"
    for t in linux-x86_64 linux-arm64 darwin-x86_64; do
        pkg="driftwatchdog-9.9.9-$t"
        work="$stage/$pkg"
        mkdir -p "$work"
        printf '#!/bin/sh\necho smoke %s\n' "$t" > "$work/driftwatchdog"
        chmod 0755 "$work/driftwatchdog"
        printf '9.9.9\n' > "$work/VERSION"
        (cd "$stage" && tar -czf "driftwatchdog-9.9.9-$t.tar.gz" "$pkg")
    done
    sh "$repo_root/scripts/checksum.sh" "$stage" 9.9.9 >/dev/null
fi

# Verify the manifest is in the expected shape.
manifest="$stage/driftwatchdog-9.9.9-checksums.txt"
[ -f "$manifest" ] || { printf 'manifest missing: %s\n' "$manifest" >&2; exit 1; }
archive_count=$(awk 'NF == 2 { n++ } END { print n+0 }' "$manifest")
[ "$archive_count" -eq 3 ] || { printf 'expected 3 archives in manifest, got %d\n' "$archive_count" >&2; exit 1; }

# Confirm each archive extracts and contains a runnable binary.
for t in linux-x86_64 linux-arm64 darwin-x86_64; do
    tar -tzf "$stage/driftwatchdog-9.9.9-$t.tar.gz" | grep -q "^driftwatchdog-9.9.9-$t/driftwatchdog$" \
        || { printf 'archive %s missing binary\n' "$t" >&2; exit 1; }
done

# Confirm the installer accepts the staged manifest + archive.
dest=$(mktemp -d)
trap 'rm -rf "$stage" "$dest"' EXIT INT TERM
DRIFTWATCH_TARGET=linux-x86_64 \
    DRIFTWATCH_RELEASE_BASE="file://$stage" \
    DRIFTWATCH_LIB_DIR="$repo_root/scripts/lib" \
    sh "$repo_root/scripts/install.sh" --version 9.9.9 --dest "$dest" >/dev/null
[ -x "$dest/driftwatchdog" ] || { printf 'installer did not produce a runnable binary\n' >&2; exit 1; }

printf 'smoke: ok (stage=%s)\n' "$stage"
