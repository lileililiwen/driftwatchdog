#!/usr/bin/env sh
# tests: deployable Compose/CI starter templates remain portable and wired to
# the same local contract that Driftwatchdog enforces.

test_deployable_templates() {
    dir="$repo_root/templates/deployable"
    for file in \
        "$dir/gate.toml" \
        "$dir/scripts/ci.sh" \
        "$dir/compose.yaml" \
        "$dir/.github/workflows/ci.yml" \
        "$dir/README.md"; do
        [ -f "$file" ] || { printf 'missing %s\n' "$file" >&2; return 1; }
    done

    grep -q '^profile = "deployable"$' "$dir/gate.toml" \
        || { printf 'template does not select deployable profile\n' >&2; return 1; }
    grep -q 'id = "compose-contract"' "$dir/gate.toml" \
        || { printf 'template does not bind compose-contract\n' >&2; return 1; }
    grep -q 'id = "ci-contract"' "$dir/gate.toml" \
        || { printf 'template does not bind ci-contract\n' >&2; return 1; }
    grep -q 'docker compose -f compose.yaml config --quiet' "$dir/scripts/ci.sh" \
        || { printf 'local CI contract does not validate Compose\n' >&2; return 1; }
    grep -q 'run: ./scripts/ci.sh' "$dir/.github/workflows/ci.yml" \
        || { printf 'CI workflow does not run local contract\n' >&2; return 1; }
    grep -q 'run: driftwatchdog gate' "$dir/.github/workflows/ci.yml" \
        || { printf 'CI workflow does not run Driftwatch Gate\n' >&2; return 1; }
    if grep -R -nE '/home/|/Users/|[A-Za-z]:\\\\' "$dir" >/dev/null 2>&1; then
        printf 'deployable templates contain machine-specific paths\n' >&2
        return 1
    fi
}
