#!/usr/bin/env sh
# tests: BFS-DFS-BFS change workflow authoring checks. Every active
# change under openspec/changes/ must record the three execution
# phases in tasks.md (BFS impact, DFS implementation, BFS
# regression), keep proposal/design/spec responsibilities distinct,
# and name verifiable tasks. This guards the planning queue from
# silently dropping a phase or archiving before the final BFS
# regression review.

# Check one change directory. Prints the reason to stderr and
# returns non-zero when the directory violates the workflow.
# (All locals are wf_-prefixed: this file is sourced into the
# packaging harness, so plain names like $name would clobber the
# harness loop variables.)
check_workflow_dir() {
    wf_dir="$1"
    wf_name=$(basename "$wf_dir")
    wf_tasks="$wf_dir/tasks.md"
    wf_proposal="$wf_dir/proposal.md"
    wf_design="$wf_dir/design.md"

    [ -f "$wf_tasks" ] || { printf '%s: missing tasks.md\n' "$wf_name" >&2; return 1; }
    [ -f "$wf_proposal" ] || { printf '%s: missing proposal.md\n' "$wf_name" >&2; return 1; }
    [ -f "$wf_design" ] || { printf '%s: missing design.md\n' "$wf_name" >&2; return 1; }
    # Spec lives either as a top-level spec.md or under specs/*/spec.md.
    if [ ! -f "$wf_dir/spec.md" ] && ! ls "$wf_dir"/specs/*/spec.md >/dev/null 2>&1; then
        printf '%s: missing spec.md (or specs/*/spec.md)\n' "$wf_name" >&2
        return 1
    fi

    # The three phases must appear in order: BFS, then DFS, then BFS.
    wf_first_bfs=$(grep -niE '^#+.*BFS' "$wf_tasks" | head -n 1 | cut -d: -f1)
    wf_dfs=$(grep -niE '^#+.*DFS' "$wf_tasks" | head -n 1 | cut -d: -f1)
    wf_last_bfs=$(grep -niE '^#+.*BFS' "$wf_tasks" | tail -n 1 | cut -d: -f1)
    [ -n "$wf_first_bfs" ] || { printf '%s: tasks.md missing BFS phase heading\n' "$wf_name" >&2; return 1; }
    [ -n "$wf_dfs" ] || { printf '%s: tasks.md missing DFS phase heading\n' "$wf_name" >&2; return 1; }
    [ -n "$wf_last_bfs" ] || { printf '%s: tasks.md missing final BFS phase heading\n' "$wf_name" >&2; return 1; }
    [ "$wf_first_bfs" -lt "$wf_dfs" ] || { printf '%s: DFS phase must follow initial BFS\n' "$wf_name" >&2; return 1; }
    [ "$wf_dfs" -lt "$wf_last_bfs" ] || { printf '%s: final BFS phase must follow DFS\n' "$wf_name" >&2; return 1; }

    # Each phase section must name at least one verifiable checkbox task.
    wf_total=$(grep -cE '^- \[.\] ' "$wf_tasks" || true)
    [ "$wf_total" -ge 3 ] || { printf '%s: tasks.md needs a checkbox per phase (found %s)\n' "$wf_name" "$wf_total" >&2; return 1; }

    # The final BFS section must mention verification (local gates or
    # OpenSpec validation), otherwise completion could be claimed
    # without running anything.
    wf_final_section=$(sed -n "${wf_dfs},\$p" "$wf_tasks")
    printf '%s' "$wf_final_section" | grep -qiE 'verif|validat|openspec validate|cargo (test|clippy|fmt)' \
        || { printf '%s: final BFS section names no verification step\n' "$wf_name" >&2; return 1; }

    # Every spec file must carry at least one Scenario so requirements
    # stay behavior-focused and reviewable.
    for wf_spec in "$wf_dir"/spec.md "$wf_dir"/specs/*/spec.md; do
        [ -f "$wf_spec" ] || continue
        grep -qE '^[[:space:]#]*Scenario' "$wf_spec" \
            || { printf '%s: %s has no Scenario\n' "$wf_name" "$wf_spec" >&2; return 1; }
    done
}

test_change_workflow() {
    wf_changes_dir="$repo_root/openspec/changes"
    [ -d "$wf_changes_dir" ] || { printf 'openspec/changes/ directory missing\n' >&2; return 1; }

    # 1. Repository guidance must document the workflow, the artifact
    #    responsibilities, local-before-CI verification, and the
    #    no-archive-before-final-BFS gate.
    wf_agents="$repo_root/AGENTS.md"
    grep -qE 'BFS.*DFS.*BFS' "$wf_agents" \
        || { printf 'AGENTS.md does not document the BFS-DFS-BFS workflow\n' >&2; return 1; }
    grep -qE 'proposal\.md' "$wf_agents" \
        || { printf 'AGENTS.md does not assign proposal.md responsibilities\n' >&2; return 1; }
    grep -qE 'design\.md' "$wf_agents" \
        || { printf 'AGENTS.md does not assign design.md responsibilities\n' >&2; return 1; }
    grep -qE 'spec\.md' "$wf_agents" \
        || { printf 'AGENTS.md does not assign spec.md responsibilities\n' >&2; return 1; }
    grep -qE 'CI repeats local verification|local verification.*CI|CI repeats it' "$wf_agents" \
        || { printf 'AGENTS.md does not state local-before-CI verification\n' >&2; return 1; }

    # 2. Every active (non-archived) change must satisfy the workflow.
    wf_found=0
    for wf_dir in "$wf_changes_dir"/*/; do
        [ -d "$wf_dir" ] || continue
        # Skip the archive container itself.
        [ "$(basename "$wf_dir")" = "archive" ] && continue
        [ -f "$wf_dir/tasks.md" ] || continue
        wf_found=$((wf_found + 1))
        check_workflow_dir "$wf_dir" || return 1
    done
    [ "$wf_found" -ge 1 ] || { printf 'no active changes found under openspec/changes/\n' >&2; return 1; }

    # 3. Negative fixture: a tasks.md without the DFS phase must be
    #    rejected, proving the check is not vacuously green.
    wf_fixture=$(mktemp -d) || return 1
    mkdir -p "$wf_fixture/broken/specs/broken"
    printf '# Proposal\n' > "$wf_fixture/broken/proposal.md"
    printf '## Context\n' > "$wf_fixture/broken/design.md"
    printf '## ADDED Requirements\n### Requirement: x\n#### Scenario: y\n' > "$wf_fixture/broken/specs/broken/spec.md"
    printf '## 1. BFS: Impact\n\n- [ ] 1.1 map things\n\n## 3. BFS: Regression\n\n- [ ] 3.1 verify things\n' > "$wf_fixture/broken/tasks.md"
    if check_workflow_dir "$wf_fixture/broken" 2>/dev/null; then
        printf 'negative fixture: tasks.md without DFS phase was accepted\n' >&2
        rm -rf "$wf_fixture"
        return 1
    fi
    rm -rf "$wf_fixture"
}
