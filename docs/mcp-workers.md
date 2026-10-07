# Worker MCP resolution

Workers resolve enabled, human-approved project servers from the authenticated
API immediately before execution. They do not read approval from Git or local
task files. Registry state is rechecked at every broker operation; revocation,
deletion or revision changes invalidate an existing connection.

By default all approved project servers are selected. A task may only narrow
access using `context.worker.mcp`:

```json
{
  "servers": [
    {
      "server_id": "00000000-0000-0000-0000-000000000001",
      "tools": ["lookup"]
    }
  ]
}
```

Omit `tools` to use the server's approved tools, still limited by the execution
profile. An empty `tools` list grants no tools. An empty `servers` list opts out
of MCP. Unknown server IDs, unapproved tools, extra definition fields, legacy
`context.worker.mcp_servers`, and raw MCP entries in worker settings or sub-agent
definitions are rejected.
Register servers through the project registry, store credentials separately and
obtain human approval instead of putting executable/endpoint/credential definitions
in task JSON.

Resolved connections are opaque, non-serializable execution-owned capabilities.
Only upstream transports receive registry credentials; prompts and provider
scripts never receive them. Normal completion and errors await session shutdown;
dropping a cancelled worker also signals upstream cleanup.

The centralized capability matrix reserves broker adapters for `claude-code`,
`codex` and `opencode`. Claude Code implements the broker adapter; Codex and
OpenCode explicitly reject nonempty broker access until their adapters exist.
`anthropic`, `openai`, `copilot` and `ollama` reject MCP as unsupported. Ordinary
execution without a resolved MCP registry is unchanged. Headless local sources
cannot approve or connect servers themselves.

## Claude Code

Managed workers (including an empty resolved registry) use `--strict-mcp-config`
and a private runtime JSON file containing only authenticated loopback HTTP
broker endpoints. User/project/local settings sources are disabled for managed
execution. Each endpoint dispatches only initialize, ping, tools/list and
tools/call through the task-scoped opaque connection; broker policy remains
authoritative on every list/call. No resources, prompts or upstream configuration
are exported. Requests are bounded to 1 MiB and connections to eight concurrent
clients with a 35-second lifetime.

Native MCP grants are exact `mcp__orchestra_<index>__<tool>` names. Managed runs
use `--permission-mode dontAsk`, never permission bypass. Worker MCP/permission,
hook, plugin and runtime-environment overrides are rejected; ordinary native
tool rules, model, budget, named/custom agents and non-conflicting settings remain
available. The adapter requires CLI help to advertise strict configuration,
settings-source isolation and dontAsk; unsupported versions fail closed with an
upgrade diagnostic. Claude Code >= 2.1.246 is required for its documented fix
to strict headless sessions waiting on ambient project approvals. Chrome MCP is
explicitly disabled. See the [Claude MCP reference](https://code.claude.com/docs/en/mcp#project-scope)
and [CLI reference](https://code.claude.com/docs/en/cli-reference).
Non-worker invocations without a resolved registry retain
their prior CLI behavior.

Runtime files are owner-only, outside Git in an OS temporary directory, and
removed on success, failure or cancellation. Only a fresh task-local broker
bearer token enters the private JSON file; no upstream credentials enter CLI
arguments or wrapper scripts. Cancellation closes the HTTP listener and kills
the PTY process group. No global or repository Claude configuration is written.

Fake CLI tests live in `apps/orchestra/tests/fixtures/mcp_claude_code.rs` and are
included as binary unit tests (provider wiring is not a library export). Run
`cargo test -p diraigent-orchestra mcp`.
