## ADDED Requirements
### Requirement: Serve MCP over stdio
`driftwatch mcp` MUST speak newline-delimited JSON-RPC 2.0 on stdin/stdout and MUST complete the MCP handshake (`initialize` → `tools/list` → `tools/call`).

#### Scenario: Client handshake
- **WHEN** a client sends `initialize` with a supported protocol version
- **THEN** the server responds with matching `id`, `serverInfo.name "driftwatchdog"`, and `capabilities.tools`

#### Scenario: Unknown method
- **WHEN** a client sends a method outside `initialize`/`tools/list`/`tools/call`/`notifications/initialized`
- **THEN** the server responds with JSON-RPC error `-32601` and keeps serving further requests

#### Scenario: Malformed input
- **WHEN** a line is not valid JSON
- **THEN** the server emits `-32700` (when an `id` is recoverable) or skips the line with a stderr note, and never panics or exits

### Requirement: Expose read-only bug tools
The server MUST expose `top_bugs`, `show_bug`, `ai_report`, and `doctor_status`, each returning the same content as its CLI counterpart.

#### Scenario: List tools
- **WHEN** a client sends `tools/list`
- **THEN** exactly the four tools are advertised, each with a closed JSON input schema

#### Scenario: Tool output parity
- **WHEN** `ai_report` is called with `{}` on a project with known state
- **THEN** the returned Markdown is byte-identical to `driftwatch report --ai` on the same state

#### Scenario: Unknown bug id
- **WHEN** `show_bug` is called with an id matching nothing
- **THEN** the tool result is an error carrying the CLI's `hint:` remediation, not a panic or empty success

### Requirement: Stay read-only and offline
The MCP server MUST NOT mutate state, execute child commands, or touch the network.

#### Scenario: Read-only database
- **WHEN** the server opens the project database
- **THEN** it uses read-only open flags so any mutation attempt fails at the driver level

#### Scenario: No execution surface
- **WHEN** the `src/mcp/` module is inspected
- **THEN** it contains no process-spawning calls and no socket listeners; stdio is the only I/O
