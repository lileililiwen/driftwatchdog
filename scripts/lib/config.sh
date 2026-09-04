# shellcheck shell=sh
# Shared packaging configuration: version, target matrix, release URL.
# Sourced by package.sh, checksum.sh, install.sh, and the GitHub workflow
# (the workflow duplicates the matrix in YAML; this file is the source of
# truth for shell-side packaging and the installer).

# Repository host used for release URLs.
DRIFTWATCH_REPOSITORY="lileililiwen/driftwatchdog"

# Supported release targets: "<go-arch> <artifact-suffix>".
# Order matters only for human readability.
DRIFTWATCH_TARGETS="
x86_64-unknown-linux-gnu linux-x86_64
aarch64-unknown-linux-gnu linux-arm64
x86_64-apple-darwin darwin-x86_64
"

# Number of targets in the matrix.
DRIFTWATCH_TARGET_COUNT=3

# Archive extension.
DRIFTWATCH_ARCHIVE_EXT="tar.gz"
