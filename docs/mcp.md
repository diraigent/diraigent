# Approved project MCP registry (contract v1)

The API is the sole authority for approved configuration. Orchestra must not
interpret legacy `task.context.worker` JSON, repository files, discovery results,
or provider defaults as approval. Registry operations never install packages,
launch executables, connect to endpoints, or discover tools.

## Configuration

All paths below are authenticated `/v1/{project_id}/mcp-servers` paths. Shared
configuration DTOs live in `diraigent-types::mcp`. Configuration is a complete
replacement, not a merge:

```json
{
  "name": "documentation",
  "transport": {"transport": "stdio", "executable": "/opt/tools/docs-server", "arguments": []},
  "tools": [{"name": "lookup", "access": "read"}],
  "credential_bindings": {"DOCS_TOKEN": "docs_token"}
}
```

Alternatively use `{"transport":"streamable_http","endpoint":"https://example.com/mcp"}`.
Only explicit absolute executable paths and HTTPS Streamable HTTP endpoints are
accepted. URLs cannot contain userinfo, queries or fragments. No SSE transport or
automatic package installation. Arguments and names are **non-secret** public
configuration; do not put passwords or tokens there. Environment variable names
(stdio) or header names (HTTP) map to credential keys, not raw values. Runtimes
must use a minimal environment and must not inherit ambient MCP servers/tools.

Tools are an exact, case-sensitive allowlist. Names use ASCII letters, digits,
`_`, `-`, `.` (1–128 bytes), without wildcards, patterns, duplicates or implicit
permissions. Each allowed tool declares `read` or `write`. An empty list grants
no tools. Classification is the approving human's policy assertion, not a claim
that an external server is trustworthy. Adapters must enforce exact names and
fail closed if they cannot enforce the policy.

## Endpoints and approval

| Method | Suffix | Behavior |
|---|---|---|
| GET | (empty) | Public registry rows, never secret values |
| POST | (empty) | Configuration; tools must be empty; disabled/unapproved revision 1 |
| GET | `/{id}` | Public row, scoped to this exact project |
| PUT | `/{id}` | `{revision, configuration}`; full replacement |
| DELETE | `/{id}` | Delete server and credential record |
| PUT | `/{id}/credentials` | `{revision, credentials: {key: value}}`; write-only full replacement; `{}` clears |
| POST | `/{id}/approve` | `{revision}`; approve and enable the exact current revision |
| POST | `/{id}/disable` | `{revision}`; disable and revoke approval |
| GET | `/resolve` | Agent-only `{contract_version: 1, servers: [...]}`; approved enabled rows only |
| POST | `/{id}/credentials/resolve` | Agent-only `{revision}`; runtime-only credential map with `Cache-Control: no-store` |

Every configuration or credential replacement increments the revision, clears
approval actor/time/revision, and disables the server, even for no-op writes.
Stale writes/approvals fail with 409. Approval also rejects missing credential
bindings. Configuration/credentials cannot set enabled or approval fields.
Approval and credential writes lock/update the same server row transactionally.
Only a human with existing project `manage` authorization can approve/enable.
Agent keys are identified from the bearer token even without `X-Agent-Id` and
cannot self-approve; mismatched identities fail closed. A JWT with agent context
is also an agent, not a human approver. Human authorization follows the API's
existing same-tenant implicit authority rules. Agent CRUD uses `manage`, reads
require membership; runtime resolution additionally requires `execute`.
All requests check project/tenant alignment, and item queries always include the
project id. There is no cross-project inheritance of server configuration.

Resolution is a point-in-time snapshot, not a permanent execution grant. Runtimes
must resolve before each execution/session, pin the revision, retrieve credentials
only for that revision, and stop/re-resolve when approval is revoked or revision
changes. Never persist secrets in task prompts, logs, debug output, or context.
Credential resolution returns only keys bound by the pinned configuration, and
is the **only** secret-bearing runtime endpoint; normal
registry reads and configuration resolution expose only credential key names and
bindings. The API does not yet provide live revocation events or runtime tool
execution; adapters must not claim mid-call cancellation guarantees.

## Storage and audit

Credential values are stored in a separate cascading table, not on project or
server configuration rows. CryptoDb encrypts them with the tenant DEK when tenant
encryption is configured, using server-specific authenticated associated data.
Locked/unavailable encryption keys fail writes and runtime reads. With encryption
mode `none`, the separate credential table contains plaintext JSON: separation
and response redaction are **not encryption at rest**. Secure database/backups and
transport accordingly. Key migration when changing tenant encryption modes must
follow the existing encryption lifecycle; this registry does not rewrap old data.

Mutation events use the existing audit/webhook mechanism with only id, revision,
enabled and approved revision metadata. They never snapshot configuration,
credentials, or request bodies. Write DTO debug output is redacted; internal
secret envelopes implement neither serialization nor debug formatting.
