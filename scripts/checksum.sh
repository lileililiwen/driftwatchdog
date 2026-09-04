#!/usr/bin/env sh
# Generate the SHA-256 checksum manifest for a release stage.
#
# Usage:
#   scripts/checksum.sh <stage-dir> <version>
#
# Reads every driftwatchdog-<version>-*.tar.gz in <stage-dir> and writes
# a stable manifest at <stage-dir>/driftwatchdog-<version>-checksums.txt
# with one "<sha256>  <filename>" line per archive (two spaces between
# fields, matching `sha256sum -c` expectations). Echoes the manifest
# path on stdout. The manifest is the integrity source for both the
# shell installer and the npm launcher.

set -eu

if [ "$#" -ne 2 ]; then
    printf 'usage: %s <stage-dir> <version>\n' "$0" >&2
    exit 64
fi

stage_dir=$1
version=$2
manifest_name="driftwatchdog-${version}-checksums.txt"
manifest_path="$stage_dir/$manifest_name"

if ! command -v sha256sum >/dev/null 2>&1; then
    printf 'sha256sum is required to generate the manifest\n' >&2
    exit 1
fi

if [ ! -d "$stage_dir" ]; then
    printf 'stage directory not found: %s\n' "$stage_dir" >&2
    exit 1
fi

# Build the manifest deterministically by sorting filenames.
archives=$(cd "$stage_dir" && ls "driftwatchdog-${version}-"*.tar.gz 2>/dev/null | sort)
if [ -z "$archives" ]; then
    printf 'no archives matching driftwatchdog-%s-*.tar.gz in %s\n' "$version" "$stage_dir" >&2
    exit 1
fi

# Truncate the manifest and rewrite from scratch.
: > "$manifest_path"
for archive in $archives; do
    (cd "$stage_dir" && sha256sum "$archive") >> "$manifest_path"
done

printf '%s\n' "$manifest_path"
