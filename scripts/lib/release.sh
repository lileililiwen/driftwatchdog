# shellcheck shell=sh
# URL construction and archive naming shared by the installer, the
# release builder, and the GitHub workflow.
#
# Callers must set DRIFTWATCH_LIB_DIR to the directory holding this
# file before sourcing. POSIX `sh` does not update $0 inside a sourced
# file, so we cannot reliably locate config.sh from inside this script.

if [ -z "${DRIFTWATCH_LIB_DIR:-}" ]; then
    printf 'release.sh: DRIFTWATCH_LIB_DIR is not set; cannot locate config.sh\n' >&2
    return 1 2>/dev/null || exit 1
fi
. "$DRIFTWATCH_LIB_DIR/config.sh"

# Echoes the GitHub Releases download base for a given version tag
# (with or without the leading "v"; normalized either way).
driftwatch_release_base() {
    version_tag="$1"
    case "$version_tag" in
        v*) ;;
        *)  version_tag="v$version_tag" ;;
    esac
    printf 'https://github.com/%s/releases/download/%s\n' \
        "$DRIFTWATCH_REPOSITORY" "$version_tag"
}

# Echoes the archive name for a given (version, target) pair.
#   driftwatch_archive_name <version> <target>
driftwatch_archive_name() {
    version="$1"
    target="$2"
    printf 'driftwatchdog-%s-%s.%s\n' "$version" "$target" "$DRIFTWATCH_ARCHIVE_EXT"
}

# Echoes the checksum manifest name.
#   driftwatch_checksum_name <version>
driftwatch_checksum_name() {
    version="$1"
    printf 'driftwatchdog-%s-checksums.txt\n' "$version"
}

# Echoes the full URL to the checksum manifest for a version.
driftwatch_checksum_url() {
    base=$(driftwatch_release_base "$1")
    name=$(driftwatch_checksum_name "$2")
    printf '%s/%s\n' "$base" "$name"
}

# Echoes the full URL to the archive for a (version, target) pair.
driftwatch_archive_url() {
    base=$(driftwatch_release_base "$1")
    name=$(driftwatch_archive_name "$2" "$3")
    printf '%s/%s\n' "$base" "$name"
}
