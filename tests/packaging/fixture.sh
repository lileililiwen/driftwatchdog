#!/usr/bin/env sh
# Test fixture: build a stage directory containing valid-looking release
# artifacts for the three supported targets. Other test files source
# this to avoid duplicating the setup.

# Build a fixture stage into $1. Sets `fixture_dir` in the parent shell
# to the absolute path of the populated stage.
build_fixture_stage() {
    out=$1
    version=${2:-9.9.9}
    rm -rf "$out"
    mkdir -p "$out"
    printf '#!/bin/sh\necho driftwatchdog-fixture %s\n' "$version" > "$out/driftwatchdog"
    chmod 0755 "$out/driftwatchdog"

    for t in linux-x86_64 linux-arm64 darwin-x86_64; do
        pkg="driftwatchdog-${version}-${t}"
        work="$out/$pkg"
        mkdir -p "$work"
        cp "$out/driftwatchdog" "$work/driftwatchdog"
        chmod 0755 "$work/driftwatchdog"
        printf 'driftwatchdog version: %s\n' "$version" > "$work/VERSION"
        (cd "$out" && tar -czf "driftwatchdog-${version}-${t}.tar.gz" "$pkg")
    done

    sh "$repo_root/scripts/checksum.sh" "$out" "$version" >/dev/null
    fixture_dir=$out
}
