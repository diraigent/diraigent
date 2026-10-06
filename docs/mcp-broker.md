# Orchestra MCP broker

`diraigent_orchestra::mcp` is an independent library service, not worker wiring
or a provider flag. It accepts only authenticated registry `Approval` snapshots
and a `Registry` implementation which obtains **current** authoritative policy.
It never reads task context, repository configuration or ambient MCP settings.
The API bridge must bind credential resolution to the same server/revision.

`Broker::connect` returns a broker, opaque `TaskAccess` capability and sanitized
`McpCheckResult`. `tools_list` and `tools_call` require that capability. It cannot
be forged through public fields or serialized, and belongs to one task/project/
profile/session. There is no unauthenticated TCP listener. A future provider
adapter must keep this capability private and authenticate any task-local IPC
bridge; it must never expose the upstream configuration or credentials.

## Policy

- Revalidate project, server, enabled state, approved revision, full configuration
  and endpoint policy on each operation, including immediately before a call.
  Registry outages fail closed. There is no promise to revoke an already-running
  upstream operation when approval changes mid-call.
- Only exact human-approved names discovered in the initial session are exposed.
  Changes to an exposed definition or newly appearing approved tools fail closed;
  reconnect/reapproval is required. Direct calls cannot bypass the filter.
- Review, research and delivery profiles get only human-classified `read` tools;
  execute can use classified read/write tools. Server annotations cannot upgrade
  permissions. This is authorization, not proof that a server behaves read-only.
- Only initialization, initialized notification, tools/list and tools/call are
  forwarded. Server requests for sampling, elicitation or roots are rejected.
  Resources/prompts, installation and capability negotiation are not implicit.
- `AuditSink` receives only identifiers, pinned revision, approved tool name and
  typed outcome (including cancellation). Its callback must be nonblocking.
  Errors and failed checks are sanitized; no upstream exception strings, bodies,
  arguments, credentials or endpoint URLs enter audit/check payloads.

## Transports and bounds

Stdio launches an explicitly approved **preinstalled absolute executable** with
exact arguments, no shell, no PATH lookup, empty inherited environment, only bound
credential variables and discarded stderr. Known package-manager entry points
are rejected, including canonical symlinks. The broker never downloads a server
or constructs installer commands. Human approval still needs to review arbitrary
executables/wrappers: this is not an OS sandbox for approved server code. Unix
process groups kill descendants on drop, cancellation, timeout and expiry; normal
shutdown waits for the child to be reaped.

Streamable HTTP supports protocol `2025-06-18`, JSON and bounded SSE responses,
session IDs and best-effort session DELETE. It uses TLS verification, no proxies,
no redirects, and pins all resolved DNS addresses after endpoint validation.
Private/loopback/link-local/special addresses are denied by default, including
IPv4-mapped IPv6. `allow_private_network` is a **separate explicit human endpoint
approval**, not inferred from discovery or task JSON; the API bridge must leave it
false without such approval. It applies only to the pinned configured endpoint.
HTTP is allowed only with this local-server override; registry v1 currently only
accepts HTTPS. There is no arbitrary URL check/probe or SSE GET background stream.

Defaults: 10-second connection phases, 30-second operations, 30-minute session,
1 MiB message/aggregate catalog, 256 tools, eight queued operations and one active
upstream request. Configurable limits have hard upper bounds. Queued inputs are
size-checked before admission; discovery pagination, JSON lines and SSE bytes are
bounded before deserialization. Full queues return `Busy`. Dropping an operation
future closes the session, preventing stale responses from being reused. Protocol,
transport, size, stale-policy and timeout failures close the session. Dropping the
last broker stops the actor and cleans up its upstream transport.

## SDK choice and validation

Inspected official `rmcp` 3.5.1 (Rust >=1.88; compatible with this toolchain).
Its async-rw transport reads into an extensible line buffer, and its reqwest JSON
response path calls `response.text()` before parsing. SSE limits alone do not
cover the required stdio/JSON pre-allocation bounds. This initial implementation
therefore uses a small explicit, bounded tools-only transport instead of adopting
those transports with weaker limits. Reconsider the SDK when all ingress paths
provide bounded framing hooks.

`cargo test -p diraigent-orchestra mcp` runs credential-free local HTTP and stdio
fake-server tests (stdio fixtures require preinstalled `python3`). Tests cover
JSON/SSE initialization, sessions, exact/read-only permissions, capabilities from
another task/project, catalog drift, stale/revoked approval, duplicate/paginated
catalogs, input/output bounds, queue pressure, disconnects, timeouts, cancellation
and subprocess cleanup. Worker/provider integration is deliberately separate.
