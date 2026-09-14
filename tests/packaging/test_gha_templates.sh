#!/usr/bin/env sh
# tests: github-actions template shape. Asserts that the reusable
# workflow file at templates/github-actions/driftwatch-check.yml
# declares the right triggers, references the same release base
# the shell installer uses, always renders the AI report, and
# never uploads .driftwatch/state.db or command logs. The
# matching README.md must document the inputs and the never-
# upload contract.

test_gha_templates() {
    template="$repo_root/templates/github-actions/driftwatch-check.yml"
    readme="$repo_root/templates/github-actions/README.md"
    [ -f "$template" ] || { printf 'missing %s\n' "$template" >&2; return 1; }
    [ -f "$readme" ]   || { printf 'missing %s\n' "$readme"   >&2; return 1; }

    # 1. Triggers: workflow_call + workflow_dispatch.
    grep -qE '^on:' "$template" || { printf 'template missing `on:` key\n' >&2; return 1; }
    grep -qE '^[[:space:]]+workflow_call:' "$template" \
        || { printf 'template missing workflow_call trigger\n' >&2; return 1; }
    grep -qE '^[[:space:]]+workflow_dispatch:' "$template" \
        || { printf 'template missing workflow_dispatch trigger\n' >&2; return 1; }

    # 2. The installer base URL must point at the same GitHub
    #    repository the shell installer uses. The template builds
    #    the URL as `<base>/install.sh` so look for the bare base
    #    (the `latest` pseudo-tag lives at
    #    `releases/latest/download/` on github.com).
    expected_repo=$(DRIFTWATCH_LIB_DIR="$repo_root/scripts/lib" \
        sh -c '. "$DRIFTWATCH_LIB_DIR/release.sh"; driftwatch_release_base v0.0.0' \
        | sed 's|^https://github.com/\([^/]*/[^/]*\)/.*|\1|')
    [ -n "$expected_repo" ] || { printf 'release.sh did not echo a base URL\n' >&2; return 1; }
    expected_url="https://github.com/${expected_repo}/releases/latest/download"
    grep -qF "$expected_url" "$template" \
        || { printf 'template installer base %s not found\n' "$expected_url" >&2; return 1; }

    # 3. `driftwatch check` and the report render must both run.
    grep -qE 'driftwatch[[:space:]]+check([[:space:]]|$)' "$template" \
        || { printf 'template does not run `driftwatch check`\n' >&2; return 1; }
    grep -qE 'driftwatch[[:space:]]+report[[:space:]]+--ai' "$template" \
        || { printf 'template does not render `driftwatch report --ai`\n' >&2; return 1; }
    # The render step must be `if: always()` so the report uploads
    # even when checkers reported drift.
    grep -qE 'if:[[:space:]]+always\(\)' "$template" \
        || { printf 'template missing `if: always()` for the report step\n' >&2; return 1; }

    # 4. The upload step targets only `drift.md`; never `state.db`
    #    or anything under `.driftwatch/`. Inspect each upload
    #    block's `path:` field rather than greppping the whole
    #    file, so a comment that mentions these strings (as a
    #    "never published" warning) does not falsely fail.
    if grep -qE 'actions/upload-artifact' "$template"; then
        # Walk every upload block. An upload-artifact step in
        # this template has a `path:` field immediately below the
        # `with:` block. Use awk to print each path value.
        upload_paths=$(awk '
            /actions\/upload-artifact/ { in_step = 1; next }
            in_step && /^[[:space:]]+path:[[:space:]]*/ {
                sub(/^[[:space:]]+path:[[:space:]]*/, "")
                print
                in_step = 0
                next
            }
            in_step && /^[[:space:]]*- / { in_step = 0 }
        ' "$template")
        if [ -z "$upload_paths" ]; then
            printf 'template upload step missing `path:` field\n' >&2
            return 1
        fi
        bad=$(printf '%s\n' "$upload_paths" | grep -E 'state\.db|\.driftwatch/|\.driftwatch$|full[[:space:]]+log|captured' || true)
        if [ -n "$bad" ]; then
            printf 'template upload path must not include: %s\n' "$bad" >&2
            return 1
        fi
        printf '%s\n' "$upload_paths" | grep -qx 'drift.md' \
            || { printf 'template upload paths must include `drift.md`\n' >&2; return 1; }
    else
        printf 'template missing actions/upload-artifact step\n' >&2
        return 1
    fi

    # 5. Step summary must include `driftwatch top`.
    grep -qE 'GITHUB_STEP_SUMMARY' "$template" \
        || { printf 'template missing $GITHUB_STEP_SUMMARY write\n' >&2; return 1; }
    grep -qE 'driftwatch[[:space:]]+top' "$template" \
        || { printf 'template step summary missing `driftwatch top`\n' >&2; return 1; }

    # 6. `fail_on_drift` input is documented in both files.
    grep -qE 'fail_on_drift' "$template" \
        || { printf 'template does not document fail_on_drift input\n' >&2; return 1; }
    grep -qE 'fail_on_drift' "$readme" \
        || { printf 'README does not document fail_on_drift input\n' >&2; return 1; }

    # 7. Permissions must be `contents: read`.
    grep -qE 'contents:[[:space:]]+read' "$template" \
        || { printf 'template must set `contents: read`\n' >&2; return 1; }

    # 8. Embedded `run:` blocks pass `shellcheck -S error` when
    #    shellcheck is available. Soft-skip otherwise so the
    #    test stays runnable on hosts without shellcheck.
    if command -v shellcheck >/dev/null 2>&1; then
        # Extract every `run: |` / `run: >-` block into a temp
        # file. Keep the script simple: a single python3 pass
        # produces one temp script per `run:` block.
        check_dir=$(mktemp -d) || return 1
        trap 'rm -rf "$check_dir"' EXIT INT TERM
        if ! python3 - "$template" "$check_dir" <<'PY'
import os, re, sys
src_path, out_dir = sys.argv[1], sys.argv[2]
src = open(src_path).read()
# Match every `run: |` / `run: >-` block (literal scalar or
# folded scalar) including an optional leading `-` and a
# trailing key. Capture the indented body.
pattern = re.compile(
    r"^[ \t-]*run:[ \t]*\n([ \t]+.+(?:\n\1[ \t]+.*|\n[ \t]*)*)",
    re.MULTILINE,
)
idx = 0
for m in pattern.finditer(src):
    body = m.group(1)
    # Unindent: drop the common leading whitespace.
    lines = body.splitlines()
    indent = min((len(l) - len(l.lstrip(" \t"))) for l in lines if l.strip())
    body = "\n".join(l[indent:] if len(l) >= indent else l for l in lines) + "\n"
    out = os.path.join(out_dir, f"step-{idx}.sh")
    open(out, "w").write(body)
    idx += 1
if idx == 0:
    sys.exit("no `run:` blocks extracted")
PY
        then
            printf 'failed to extract run blocks\n' >&2
            return 1
        fi
        rc=0
        shellcheck -S error "$check_dir"/*.sh 2>&1 || rc=$?
        if [ "$rc" -ne 0 ]; then
            printf 'shellcheck reported errors in embedded run blocks\n' >&2
            return 1
        fi
    fi
}
