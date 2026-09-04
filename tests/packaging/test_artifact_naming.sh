#!/usr/bin/env sh
# tests: archive / manifest naming. Asserts the public artifact and
# checksum manifest filenames match the documented contract.

test_artifact_naming() {
    [ "$(driftwatch_archive_name 0.1.0 linux-x86_64)" = "driftwatchdog-0.1.0-linux-x86_64.tar.gz" ] || return 1
    [ "$(driftwatch_archive_name 0.1.0 linux-arm64)" = "driftwatchdog-0.1.0-linux-arm64.tar.gz" ] || return 1
    [ "$(driftwatch_archive_name 0.1.0 darwin-x86_64)" = "driftwatchdog-0.1.0-darwin-x86_64.tar.gz" ] || return 1
    [ "$(driftwatch_checksum_name 0.1.0)" = "driftwatchdog-0.1.0-checksums.txt" ] || return 1

    # URL construction honors v-prefix and bare versions.
    [ "$(driftwatch_release_base v1.2.3)" = "https://github.com/lileililiwen/driftwatchdog/releases/download/v1.2.3" ] || return 1
    [ "$(driftwatch_release_base 1.2.3)" = "https://github.com/lileililiwen/driftwatchdog/releases/download/v1.2.3" ] || return 1
    [ "$(driftwatch_archive_url v0.1.0 0.1.0 linux-arm64)" = "https://github.com/lileililiwen/driftwatchdog/releases/download/v0.1.0/driftwatchdog-0.1.0-linux-arm64.tar.gz" ] || return 1
    [ "$(driftwatch_checksum_url v0.1.0 0.1.0)" = "https://github.com/lileililiwen/driftwatchdog/releases/download/v0.1.0/driftwatchdog-0.1.0-checksums.txt" ] || return 1
}
