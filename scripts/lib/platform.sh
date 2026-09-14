# shellcheck shell=sh
# Sourcing rule: files under scripts/lib/ are sourced by executable
# scripts (*.sh) and must NOT be executable (mode 644, enforced in CI).
# Target selection: map (os, arch) tuples produced by `uname` to the
# driftwatchdog artifact suffix used in release archive names. Used by the
# shell installer to pick the right download for the current host and by
# the packaging tests to assert the supported matrix.

# Echoes a sorted, newline-separated list of supported artifact suffixes.
driftwatch_supported_targets() {
    printf 'linux-x86_64\nlinux-arm64\ndarwin-x86_64\n'
}

# Map a (os, arch) pair to an artifact suffix. Accepts common uname
# aliases (e.g. amd64, aarch64) and normalizes them before lookup.
#   driftwatch_target_for <os> <arch>
# Recognized inputs:
#   linux / x86_64 | amd64       -> linux-x86_64
#   linux / aarch64 | arm64      -> linux-arm64
#   darwin / x86_64 | amd64      -> darwin-x86_64
#   darwin / arm64               -> (unsupported, exits 2)
# Anything else exits 2 as well. Echoes the suffix on success.
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
        darwin/aarch64)
            printf 'unsupported: macOS arm64 is not in the supported target set\n' >&2
            return 2 ;;
        *)
            printf 'unsupported: no artifact for os=%s arch=%s\n' "$os" "$arch" >&2
            return 2 ;;
    esac
}

# Detect the current host target. Echoes the artifact suffix on success,
# exits 2 with a diagnostic on stderr listing supported targets on failure.
# Honors DRIFTWATCH_TARGET env override for tests.
driftwatch_detect_target() {
    if [ -n "${DRIFTWATCH_TARGET:-}" ]; then
        case "$DRIFTWATCH_TARGET" in
            linux-x86_64|linux-arm64|darwin-x86_64)
                printf '%s\n' "$DRIFTWATCH_TARGET"
                return 0 ;;
            *)
                printf 'unsupported: DRIFTWATCH_TARGET=%s\n' "$DRIFTWATCH_TARGET" >&2
                return 2 ;;
        esac
    fi

    os=$(uname -s 2>/dev/null || echo unknown)
    arch=$(uname -m 2>/dev/null || echo unknown)

    case "$os" in
        Linux)  os=linux ;;
        Darwin) os=darwin ;;
        *)      os=$(printf '%s' "$os" | tr '[:upper:]' '[:lower:]') ;;
    esac

    case "$arch" in
        x86_64|amd64)       arch=x86_64 ;;
        aarch64|arm64)      arch=aarch64 ;;
        *) ;;
    esac

    if target=$(driftwatch_target_for "$os" "$arch"); then
        printf '%s\n' "$target"
    else
        printf 'detected os=%s arch=%s\nsupported: %s\n' \
            "$os" "$arch" "$(driftwatch_supported_targets | tr '\n' ' ')" >&2
        return 2
    fi
}
