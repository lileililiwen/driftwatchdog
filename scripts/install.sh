#!/usr/bin/env sh
# Driftwatchdog installer.
#
# Usage:
#   curl -fsSL https://github.com/lileililiwen/driftwatchdog/releases/latest/download/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --version v0.1.0
#   curl -fsSL .../install.sh | sh -s -- --dest /opt/driftwatchdog
#
# Detects the host operating system and architecture, downloads the
# matching release archive and SHA-256 checksum manifest over HTTPS,
# verifies the archive before extracting, and atomically installs the
# `driftwatchdog` binary into the destination directory. By default
# the destination is "${XDG_BIN_HOME:-$HOME/.local/bin}" and no root
# privileges are required.
#
# Exits non-zero (and never replaces an existing binary) when:
#   * the host is not in the supported target set,
#   * the manifest or archive cannot be downloaded,
#   * the archive's SHA-256 does not match the manifest,
#   * required tools (`tar`, `sha256sum`, `curl` or `wget`) are missing,
#   * the destination is not writable.
#
# The installer deliberately never `eval`s downloaded content. The only
# things fetched from the network are the release archive and the
# checksum manifest; both are verified before any local mutation.

set -eu

# Default destination, overridable via --dest or DRIFTWATCH_INSTALL_DIR.
DRIFTWATCH_DEFAULT_DEST="${DRIFTWATCH_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}"

# Default version, overridable via --version. "latest" resolves to the
# current GitHub release tag.
requested_version="latest"
dest_dir=""
allow_root=0
dry_run=0
skip_verify=0

# Print usage to stderr and exit 64 (EX_USAGE).
usage() {
    cat >&2 <<'EOF'
driftwatchdog installer

Usage:
  install.sh [--version <version-or-tag>] [--dest <dir>] [--allow-root] [--dry-run]

Defaults:
  --version   latest   (any explicit vX.Y.Z or X.Y.Z is also accepted)
  --dest      $DRIFTWATCH_INSTALL_DIR or $XDG_BIN_HOME or $HOME/.local/bin
EOF
}

err() {
    printf 'install: %s\n' "$*" >&2
}

die() {
    err "$@"
    exit 1
}

# Parse arguments. `--key=value` and `--key value` are both accepted so
# the script behaves well when piped from curl into sh.
while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            [ "$#" -ge 2 ] || die "--version requires a value"
            requested_version=$2
            shift 2
            ;;
        --version=*)
            requested_version=${1#*=}
            shift
            ;;
        --dest)
            [ "$#" -ge 2 ] || die "--dest requires a value"
            dest_dir=$2
            shift 2
            ;;
        --dest=*)
            dest_dir=${1#*=}
            shift
            ;;
        --allow-root)
            allow_root=1
            shift
            ;;
        --dry-run)
            dry_run=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            break
            ;;
        -*)
            err "unknown option: $1"
            usage
            exit 64
            ;;
        *)
            err "unexpected positional argument: $1"
            usage
            exit 64
            ;;
    esac
done

# Resolve lib dir relative to this script. Works whether invoked as
# `sh scripts/install.sh` or via `curl | sh`, in which case BASH_SOURCE
# is unavailable and we fall back to a known location on the host.
if [ -n "${DRIFTWATCH_LIB_DIR:-}" ]; then
    lib_dir=$DRIFTWATCH_LIB_DIR
elif [ -n "${BASH_SOURCE:-}" ] && [ -f "${BASH_SOURCE[0]}" ]; then
    lib_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/lib
elif [ -f "$0" ] && [ "$0" != "sh" ] && [ "$0" != "bash" ]; then
    lib_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/lib
else
    # Piped from curl; the helper scripts live next to install.sh in the
    # repo, but curl only fetched this one file. We use a built-in
    # fallback by re-deriving the helpers inline (kept tiny to avoid drift).
    lib_dir=""
fi

if [ -n "$lib_dir" ] && [ -f "$lib_dir/release.sh" ]; then
    DRIFTWATCH_LIB_DIR="$lib_dir"
    export DRIFTWATCH_LIB_DIR
    . "$lib_dir/platform.sh"
    . "$lib_dir/release.sh"
else
    # Inline fallback for curl-pipe invocations. Mirrors scripts/lib/*.
    driftwatch_release_base() {
        version_tag="$1"
        case "$version_tag" in v*) ;; *) version_tag="v$version_tag" ;; esac
        printf 'https://github.com/lileililiwen/driftwatchdog/releases/download/%s\n' "$version_tag"
    }
    driftwatch_archive_name() {
        printf 'driftwatchdog-%s-%s.tar.gz\n' "$1" "$2"
    }
    driftwatch_checksum_name() {
        printf 'driftwatchdog-%s-checksums.txt\n' "$1"
    }
    driftwatch_supported_targets() {
        printf 'linux-x86_64\nlinux-arm64\ndarwin-x86_64\n'
    }
    driftwatch_target_for() {
        os=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')
        arch=$2
        case "$arch" in
            x86_64|amd64)  arch=x86_64 ;;
            aarch64|arm64) arch=aarch64 ;;
        esac
        case "$os/$arch" in
            linux/x86_64)   printf 'linux-x86_64\n' ;;
            linux/aarch64)  printf 'linux-arm64\n' ;;
            darwin/x86_64)  printf 'darwin-x86_64\n' ;;
            darwin/aarch64) printf 'unsupported: macOS arm64 is not in the supported target set\n' >&2; return 2 ;;
            *)              printf 'unsupported: no artifact for os=%s arch=%s\n' "$os" "$arch" >&2; return 2 ;;
        esac
    }
fi

# Detect target. DRIFTWATCH_TARGET allows test fixtures to override the
# detected host; the installer honors it the same way the platform lib
# does.
if [ -n "${DRIFTWATCH_TARGET:-}" ]; then
    case "$DRIFTWATCH_TARGET" in
        linux-x86_64|linux-arm64|darwin-x86_64) target=$DRIFTWATCH_TARGET ;;
        *) die "unsupported DRIFTWATCH_TARGET=$DRIFTWATCH_TARGET" ;;
    esac
else
    if command -v uname >/dev/null 2>&1; then
        uname_s=$(uname -s 2>/dev/null || echo unknown)
        uname_m=$(uname -m 2>/dev/null || echo unknown)
        case "$uname_s" in Linux) os=linux ;; Darwin) os=darwin ;; *) os=$(printf '%s' "$uname_s" | tr '[:upper:]' '[:lower:]') ;; esac
        case "$uname_m" in x86_64|amd64) arch=x86_64 ;; aarch64|arm64) arch=aarch64 ;; *) arch=$uname_m ;; esac
    else
        die "uname is required to detect the host"
    fi
    if ! target=$(driftwatch_target_for "$os" "$arch"); then
        err "supported targets: $(driftwatch_supported_targets | tr '\n' ' ')"
        die "host os=$os arch=$arch is not in the supported set"
    fi
fi

# Resolve version. We expose a stable URL shape so a smoke test fixture
# can serve artifacts from a local directory without GitHub.
if [ "$requested_version" = "latest" ]; then
    if [ -n "${DRIFTWATCH_RELEASE_BASE:-}" ]; then
        # Smoke test mode: a fixture base is provided directly.
        :
    else
        # Resolve the redirect from /releases/latest. curl -fsSL is
        # sufficient; the redirect target already includes the tag.
        latest_url="https://github.com/lileililiwen/driftwatchdog/releases/latest"
        if command -v curl >/dev/null 2>&1; then
            tag_path=$(curl -fsSL -o /dev/null -w '%{url_effective}' "$latest_url" 2>/dev/null || true)
        elif command -v wget >/dev/null 2>&1; then
            tag_path=$(wget -q -O /dev/null --server-response "$latest_url" 2>&1 \
                | awk '/^  Location:/ {print $2; exit}' | tr -d '\r' || true)
        else
            die "either curl or wget is required"
        fi
        case "$tag_path" in
            */tag/*) version_tag=${tag_path##*/tag/} ;;
            *)       version_tag="latest" ;;
        esac
    fi
else
    case "$requested_version" in
        v*) version_tag=$requested_version ;;
        *)  version_tag="v$requested_version" ;;
    esac
fi

version=${version_tag#v}

# Compose URLs. Honor a fixture base for offline smoke tests.
if [ -n "${DRIFTWATCH_RELEASE_BASE:-}" ]; then
    base=$DRIFTWATCH_RELEASE_BASE
else
    base=$(driftwatch_release_base "$version_tag")
fi

manifest_name=$(driftwatch_checksum_name "$version")
archive_name=$(driftwatch_archive_name "$version" "$target")
manifest_url="$base/$manifest_name"
archive_url="$base/$archive_name"

# Choose a downloader. Prefer curl; fall back to wget.
fetch() {
    url=$1
    out=$2
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 --retry-delay 1 -o "$out" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -q --tries=3 -O "$out" "$url"
    else
        die "either curl or wget is required"
    fi
}

# Set up working directory and clean it up on exit.
work_dir=$(mktemp -d 2>/dev/null) || die "failed to create temporary directory"
trap 'rm -rf "$work_dir"' EXIT INT TERM

if [ "$skip_verify" -eq 0 ]; then
    if ! command -v sha256sum >/dev/null 2>&1; then
        die "sha256sum is required for verification"
    fi
fi

if ! command -v tar >/dev/null 2>&1; then
    die "tar is required to extract the archive"
fi

# Download the manifest first. Knowing the SHA-256 before downloading
# the archive lets us verify integrity on a single download and refuse
# to replace an existing binary if the bytes are wrong.
printf 'install: target=%s version=%s\n' "$target" "$version_tag" >&2
printf 'install: fetching manifest %s\n' "$manifest_url" >&2
fetch "$manifest_url" "$work_dir/$manifest_name" \
    || die "failed to download manifest $manifest_url"

expected_hash=$(awk -v want="$archive_name" '$2 == want { print $1; exit }' \
    "$work_dir/$manifest_name")
if [ -z "$expected_hash" ]; then
    die "manifest does not list $archive_name"
fi

printf 'install: fetching archive %s\n' "$archive_url" >&2
fetch "$archive_url" "$work_dir/$archive_name" \
    || die "failed to download archive $archive_url"

if [ "$skip_verify" -eq 0 ]; then
    actual_hash=$(sha256sum "$work_dir/$archive_name" | awk '{print $1}')
    if [ "$actual_hash" != "$expected_hash" ]; then
        die "checksum mismatch: expected $expected_hash got $actual_hash"
    fi
fi

# Decide destination.
if [ -z "$dest_dir" ]; then
    dest_dir=$DRIFTWATCH_DEFAULT_DEST
fi

# Root safety. Refuse to install as root unless explicitly allowed so
# `curl | sh` does not silently write to system locations.
if [ "$(id -u 2>/dev/null || echo 1)" -eq 0 ] && [ "$allow_root" -eq 0 ]; then
    die "refusing to install as root; pass --allow-root or set DEST to a user-writable directory"
fi

if [ ! -d "$dest_dir" ]; then
    if [ "$dry_run" -eq 1 ]; then
        printf 'install: (dry-run) would create %s\n' "$dest_dir" >&2
    else
        mkdir -p "$dest_dir" || die "failed to create destination $dest_dir"
    fi
fi

if [ ! -w "$dest_dir" ]; then
    die "destination $dest_dir is not writable"
fi

# Extract into the work dir and move the binary into place. The archive
# layout is driftwatchdog-<version>-<target>/driftwatchdog.
expected_top="driftwatchdog-${version}-${target}"
if ! tar -tzf "$work_dir/$archive_name" | grep -q "^${expected_top}/driftwatchdog\$"; then
    die "archive does not contain ${expected_top}/driftwatchdog"
fi

tar -xzf "$work_dir/$archive_name" -C "$work_dir"

binary_path="$dest_dir/driftwatchdog"
tmp_binary="$work_dir/${expected_top}/driftwatchdog"
if [ ! -x "$tmp_binary" ]; then
    die "extracted binary is missing or not executable"
fi

if [ "$dry_run" -eq 1 ]; then
    printf 'install: (dry-run) would install to %s\n' "$binary_path" >&2
    printf '%s\n' "$binary_path"
    exit 0
fi

# Atomic install. Move the new binary into a sibling temp path first,
# then rename over the target so a concurrent reader never sees a
# half-written file. If the target exists, it is preserved on failure.
dest_tmp="$dest_dir/.driftwatchdog.install.$$"
if ! mv "$tmp_binary" "$dest_tmp"; then
    die "failed to stage binary to $dest_tmp"
fi
chmod 0755 "$dest_tmp"
if ! mv -f "$dest_tmp" "$binary_path"; then
    rm -f "$dest_tmp" 2>/dev/null || true
    die "failed to move binary into place at $binary_path"
fi

printf 'install: installed %s to %s\n' "$version_tag" "$binary_path" >&2
printf '%s\n' "$binary_path"
