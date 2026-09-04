# shellcheck shell=sh
# Read the package version from Cargo.toml. The crate's [package].version
# field is the single source of truth for release versions, so this script
# extracts it without depending on jq or any other JSON tool.

# Echoes the version string (e.g. "0.1.0") on stdout.
driftwatch_version() {
    awk '
        /^\[package\]/ { in_pkg = 1; next }
        in_pkg && /^version[ \t]*=/ {
            sub(/^version[ \t]*=[ \t]*/, "")
            sub(/[ \t]*$/, "")
            gsub(/^"|"$/, "")
            print
            exit
        }
        /^\[/ { in_pkg = 0 }
    ' Cargo.toml
}

# Echoes the version with a leading "v" (e.g. "v0.1.0") on stdout.
driftwatch_version_tag() {
    printf 'v%s\n' "$(driftwatch_version)"
}
