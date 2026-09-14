#!/usr/bin/env sh
# tests: agent-examples consistency. Asserts that every MCP client
# config under examples/ parses, points at the stdio server, and is
# accompanied by a workflow.md that names only real subcommands.
# This guards the example from silently drifting when the CLI surface
# changes (renames, removals) and prevents the workflow from
# instructing the agent to execute write/run commands.

test_agent_examples() {
    # Locate a real driftwatchdog binary. Prefer the release build
    # the verification gates have already produced; fall back to a
    # one-off build so a developer running just this test still has
    # --help introspection.
    binary="$repo_root/target/debug/driftwatchdog"
    if [ ! -x "$binary" ]; then
        (cd "$repo_root" && cargo build --bin driftwatchdog >/dev/null 2>&1) || true
    fi
    [ -x "$binary" ] || { printf 'no driftwatchdog binary available for --help introspection\n' >&2; return 1; }

    # Collect the subcommand names printed in the "Commands:" section
    # of `driftwatchdog --help`. The output is stable enough for a
    # `grep` + `awk` sweep; the first column of every indented line
    # under "Commands:" is the subcommand.
    help_output=$("$binary" --help) || { printf 'driftwatchdog --help failed\n' >&2; return 1; }
    subcommands=$(printf '%s\n' "$help_output" \
        | awk '/^Commands:/{flag=1; next} flag && /^$/{flag=0} flag && /^[ ]+[a-z]/{print $1}')

    json_parser=""
    if command -v python3 >/dev/null 2>&1; then
        json_parser="python3"
    elif command -v node >/dev/null 2>&1; then
        json_parser="node"
    else
        printf 'agent-examples test requires python3 or node for JSON validation\n' >&2
        return 1
    fi

    examples_dir="$repo_root/examples"
    [ -d "$examples_dir" ] || { printf 'examples/ directory missing\n' >&2; return 1; }

    # Per-harness directory. The proposal fixes this exact set;
    # adding a fourth harness is a deliberate change to the change
    # proposal, not a silent expansion.
    for harness in claude-code opencode aider; do
        harness_dir="$examples_dir/$harness"
        [ -d "$harness_dir" ] || { printf 'missing examples/%s/\n' "$harness" >&2; return 1; }

        mcp="$harness_dir/mcp.json"
        workflow="$harness_dir/workflow.md"
        [ -f "$mcp" ] || { printf 'missing %s\n' "$mcp" >&2; return 1; }
        [ -f "$workflow" ] || { printf 'missing %s\n' "$workflow" >&2; return 1; }

        # 1. The mcp.json must parse as JSON.
        if [ "$json_parser" = "python3" ]; then
            python3 - "$mcp" <<'PY' >/dev/null 2>&1 || { printf 'invalid JSON: %s\n' "$mcp" >&2; return 1; }
import json, sys
json.load(open(sys.argv[1]))
PY
        else
            node -e "JSON.parse(require('fs').readFileSync(process.argv[1], 'utf8'))" "$mcp" \
                >/dev/null 2>&1 || { printf 'invalid JSON: %s\n' "$mcp" >&2; return 1; }
        fi

        # 2. The mcp.json must declare a stdio server whose command
        #    is "driftwatch" with "mcp" in its args.
        if [ "$json_parser" = "python3" ]; then
            python3 - "$mcp" <<'PY' || { printf 'mcp.json shape wrong: %s\n' "$mcp" >&2; return 1; }
import json, sys
doc = json.load(open(sys.argv[1]))
servers = doc.get("mcpServers") or {}
target = servers.get("driftwatch") or {}
if target.get("type") != "stdio":
    raise SystemExit("type != stdio")
if target.get("command") != "driftwatch":
    raise SystemExit("command != driftwatch")
args = target.get("args") or []
if "mcp" not in args:
    raise SystemExit("args does not contain mcp")
PY
        else
            node -e '
                const doc = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
                const t = (doc.mcpServers || {}).driftwatch || {};
                if (t.type !== "stdio") process.exit(1);
                if (t.command !== "driftwatch") process.exit(1);
                if (!(t.args || []).includes("mcp")) process.exit(1);
            ' "$mcp" >/dev/null 2>&1 \
                || { printf 'mcp.json shape wrong: %s\n' "$mcp" >&2; return 1; }
        fi

        # 3. Every `driftwatch <sub>` (or `driftwatchdog <sub>`) token
        #    in workflow.md must name a real subcommand. The binaries
        #    are interchangeable from the agent's perspective; both
        #    spellings are accepted and verified against the help
        #    output.
        tokens=$(grep -oE 'driftwatch(dog)?[[:space:]]+[a-z][a-z-]*' "$workflow" \
            | awk '{print $2}' | sort -u)
        if [ -z "$tokens" ]; then
            printf 'no driftwatch tokens in %s\n' "$workflow" >&2
            return 1
        fi
        for tok in $tokens; do
            if ! printf '%s\n' "$subcommands" | grep -Fxq "$tok"; then
                printf 'unknown subcommand %s in %s\n' "$tok" "$workflow" >&2
                return 1
            fi
        done

        # 4. The workflow MUST NOT instruct the agent to call
        #    `driftwatch run` or `driftwatch check`. The MCP surface
        #    is read-only by contract; execution stays human-driven.
        if grep -qE 'driftwatch(dog)?[[:space:]]+(run|check)([[:space:]]|$)' "$workflow"; then
            printf 'workflow %s instructs the agent to run/check; that is human-driven\n' "$workflow" >&2
            return 1
        fi
    done
}
