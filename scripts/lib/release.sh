# shellcheck shell=sh
# URL construction and archive naming shared by the installer, the
# release builder, and the GitHub workflow.

# The lib dir is normally passed in by the caller (install.sh, package.sh,
# etc.). When this file is sourced directly, fall back to the directory
# holding the file so the script can be invoked standalone for ad-hoc
# debugging.
if [ -z "${DRIFTWATCH_LIB_DIR:-}" ]; then
    DRIFTWATCH_LIB_DIR=$(CDPATH= cd -- "$(dirname -- "${0:-.}")" && pwd)
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
