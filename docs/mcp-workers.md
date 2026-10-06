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
`codex` and `opencode`. Until those adapters are implemented, these providers
explicitly reject nonempty broker access rather than silently ignoring it.
`anthropic`, `openai`, `copilot` and `ollama` reject MCP as unsupported. Ordinary
execution with no selected MCP servers is unchanged. Headless local sources
cannot approve or connect servers themselves.
