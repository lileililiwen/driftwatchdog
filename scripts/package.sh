#!/usr/bin/env sh
# Build a release archive for a single target.
#
# Usage:
#   scripts/package.sh <rust-target> <artifact-suffix> <stage-dir>
#
# Builds the binary in release mode for the given target, stages it under
# <stage-dir>/driftwatchdog-<version>-<suffix>/driftwatchdog, and writes
# <stage-dir>/<archive-name>.tar.gz. Echoes the absolute path of the
# resulting archive on stdout. Designed to be invoked by the GitHub
# Actions release workflow; safe to run locally once a cross toolchain
# (or the host's native target) is available.
#
# This script is a thin wrapper around `cargo build --release`. The
# archive layout is:
#
#   driftwatchdog-<version>-<target>/
#     driftwatchdog                  # the native executable
#     VERSION                        # text file: bare version (no leading v)
#     README.url                     # text file: where the package came from
#   driftwatchdog-<version>-<target>.tar.gz

set -eu

if [ "$#" -ne 3 ]; then
    printf 'usage: %s <rust-target> <artifact-suffix> <stage-dir>\n' "$0" >&2
    exit 64
fi

rust_target=$1
artifact_suffix=$2
stage_dir=$3

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
. "$script_dir/lib/version.sh"
. "$script_dir/lib/release.sh"

version=$(driftwatch_version)
archive_name=$(driftwatch_archive_name "$version" "$artifact_suffix")
package_dir="driftwatchdog-${version}-${artifact_suffix}"

mkdir -p "$stage_dir"
work_dir="$stage_dir/$package_dir"
rm -rf "$work_dir"
mkdir -p "$work_dir"

# Build the release binary. `--locked` keeps the build deterministic
# against Cargo.lock so the workflow and local builds stay in lockstep.
(cd "$(dirname "$script_dir")" && \
    cargo build --release --locked --target "$rust_target")

src_binary="$(dirname "$script_dir")/target/$rust_target/release/driftwatchdog"
if [ ! -x "$src_binary" ]; then
    printf 'expected binary not found at %s\n' "$src_binary" >&2
    exit 1
fi

cp "$src_binary" "$work_dir/driftwatchdog"
chmod 0755 "$work_dir/driftwatchdog"
printf '%s\n' "$version" > "$work_dir/VERSION"
printf '%s\n' "$(driftwatch_release_base "$version")" > "$work_dir/README.url"

# Archive. `tar` is part of POSIX; use it without --acls/--xattrs so the
# archive stays portable across the macOS and Linux runners.
archive_path="$stage_dir/$archive_name"
rm -f "$archive_path"
(
    cd "$stage_dir"
    tar -czf "$archive_name" "$package_dir"
)

printf '%s\n' "$(cd "$stage_dir" && pwd)/$archive_name"
