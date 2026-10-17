#!/usr/bin/env sh
# Forge standard snapshot baseline-service@1.2.0: run the project's shared rehearsal script.
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
if [ ! -f ./verify-local.sh ]; then
  echo "verify-local.sh is missing from the project root; the standard snapshot's rehearsal script was removed" >&2
  exit 1
fi
exec sh ./verify-local.sh "$@"
