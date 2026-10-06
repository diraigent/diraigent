# Optional Diraigent MCP server for external assistants

**Status: design proposal only; implementation and deployment require human approval.**
This task adds no server, dependency, installation, enabled integration or listener.

## Boundary and recommended first slice

Orchestra **consuming third-party MCP servers** is the first-version integration:
the API owns their approved registry and provider adapters supply permitted tools
to workers. This proposal is separate, optional scope: an external assistant
**consumes Diraigent tools**, backed by the authenticated Diraigent API. Neither
registry approval nor legacy worker JSON authorizes this server or its clients.
Skills describe workflows; MCP tools do not grant authority to execute them.

After approval, prefer a small Rust stdio adapter (`apps/mcp`, proposed), using
the official maintained [Rust SDK `rmcp`](https://github.com/modelcontextprotocol/rust-sdk).
It fits the workspace's Tokio, Serde and HTTP client stack. At implementation
time review and pin a published release, license, security advisories, Rust
minimum version and workspace compatibility; do not track SDK Git branches.
Use the SDK's protocol negotiation and schema support, not handwritten JSON-RPC.
Start with tools only: no resources, prompts, sampling, shell, filesystem, Git,
package installation, task execution or generic HTTP/SQL proxy.

Stdio has no network listener; a human explicitly configures the executable in
their client. Protocol frames alone go to stdout; sanitized diagnostics go to
stderr. The adapter calls only a fixed, operator-approved API origin over TLS
(loopback development exceptions require separate approval), rejects redirects,
and has no database access. Never accept an API URL/path/header from tool input.

Remote Streamable HTTP is a later, separately approved deployment, preferably a
separate service behind TLS/auth ingress rather than an unauthenticated API
route. Require MCP-compatible OAuth authorization-code + PKCE, exact redirect
URIs, issuer/audience/resource validation, short-lived tokens, protected-resource
metadata, Origin/Host validation and session-to-principal binding. Do not forward
MCP bearer tokens to the API: explicitly design audience-correct delegation or
token exchange. Existing API JWT handling is not an MCP OAuth resource server.
No anonymous or spectator MCP mode is proposed.

## Identity, consent and exact permissions

For the local first slice, use a dedicated, human-provisioned owned API agent
with only the necessary role authorities, not an Orchestra manager's credential.
Load its API key from an OS keychain/secret manager (a private owner-readable
file is an explicitly approved fallback); configure only a secret reference in
client settings. No tokens in Git, command arguments, URLs, prompts or tool
parameters. This is operator-provisioned stdio authentication, not OAuth.
Binding that credential to a client is explicit local consent; possession of a
stdio pipe alone must not select a different identity or project.

A trusted human creates a revocable grant containing `client_id`, `tenant_id`,
one exact `project_id`, bound `agent_id`, exact tool names, expiry and policy
revision. Default deny; read-only grants are the default. No wildcards, inherited
project grants or model-supplied identity fields. A changed client, project,
identity, tool schema or permission revision requires renewed consent. Validate
grant expiry/revocation, agent ownership and API access on every call; a cached
tool list is not authorization. Credentials and grants are distinct stores;
encryption-disabled storage must not be described as encrypted.

Effective access is the intersection of the human grant, exact tool permission,
bound project's tenant and current API authority. Always send the bound
`X-Agent-Id` with the API credential; never accept or omit it on client request.
Currently `AuthUser` resolves `dak_` keys to the owner, while `OptionalAgentId`
provides the agent context. Human calls without that context have implicit
tenant-wide authority (`apps/api/src/auth.rs`, `apps/api/src/authz.rs`). Thus API
authentication alone is not sufficient project-scoped MCP consent. Reject
ownerless agents and do not expose development auth headers or bypass modes.

Each write additionally needs a **server-verifiable human approval** of the
exact canonical payload, project, tool and grant revision, expiring after five
minutes and consumed once. Obtain it through a trusted out-of-band UI/helper
outside the model-controlled channel. A tool argument such as `confirmed:true`
or an assistant's claim of consent is not evidence. If that channel is absent,
do not advertise or execute writes. This mechanism is future work, not an
existing API feature. Revocation must close active access, not just new sessions.

## Minimal tool contract (proposed v1)

All input schemas are JSON objects with `additionalProperties:false`. Fields
marked `?` are optional (omitted, not null); all other fields are required.
`project_id` is a UUID and must equal the grant, not switch the session project.
Unknown tool names and extra fields are rejected before any API request.

| Exact tool / permission | Input properties | Existing API operation | API check |
| --- | --- | --- | --- |
| `diraigent_list_tasks` / same exact name, read | `project_id`, `state?:enum`, `search?:string`, `limit?:integer`, `offset?:integer` | `GET /v1/{project_id}/tasks` | `require_membership` |
| `diraigent_get_task` / same exact name, read | `project_id`, `task_id:UUID` | `GET /v1/tasks/{task_id}` | `ensure_member` on task's project |
| `diraigent_list_observations` / same exact name, read | `project_id`, `limit?:integer`, `offset?:integer` | `GET /v1/{project_id}/observations` | `require_membership` |
| `diraigent_create_task` / same exact name, write | `project_id`, `title:string`, `spec:string`, `kind?:string` | `POST /v1/{project_id}/tasks` | `require_authority("create")`, package validation and task quota |
| `diraigent_record_observation` / same exact name, write | `project_id`, `title:string`, `description:string`, `kind?:string`, `severity?:enum` | `POST /v1/{project_id}/observations` | `require_authority("execute")` and package validation |

Schema bounds: title 1–200 characters, spec/description 1–8,000, search 1–200,
kind 1–64; `limit` 1–50 (default 20), `offset` 0–10,000 (default 0).
State enum: `backlog`, `ready`, `working`, `human_review`, `done`, `cancelled`.
Severity enum: `info`, `low`, `medium`, `high`, `critical`; default `low`.
Omitted kinds use API defaults; supplied kinds must pass project package rules.

Task creation maps `spec` to `context.spec`, sets `urgent:false`, and supplies
no work/parent/decision links, capabilities, worker configuration or state.
Tasks start in backlog; use a dedicated agent without active worker tasks to
avoid the existing work-inheritance path promoting them to ready. Creation
does not authorize execution. Observation creation sets `agent_id` from the
grant and `source:"external-mcp"`; it accepts no evidence, metadata or task link.
Later link support must verify all referenced entities belong to the grant's
project. No claim, transition, delete, delegate, promote, approve or registry
management tools exist in v1, even for an API manager.

Output schemas use closed objects too. `TaskSummary` contains only
`id:UUID`, `project_id:UUID`, `number:integer`, `title:string`, `kind:string`,
`state:string`, `urgent:boolean`. `ObservationSummary` contains only `id:UUID`,
`project_id:UUID`, `title:string`, `kind:string`, `severity:string`, `status:string`.
Get/create task returns `{task:TaskSummary}`; record observation returns
`{observation:ObservationSummary}`. Lists return
`{data:Summary[],total:integer,limit:integer,offset:integer,has_more:boolean}`
matching the API's paginated envelope, not an array. No full context, credentials,
evidence, metadata, actor details or arbitrary upstream fields are returned.
Check every returned entity's `project_id` before projection, including lists
and global-ID lookup; mismatch fails closed without leaking existence/content.

Return schema-valid MCP structured content with an equivalent safe text
representation. Mark reads `readOnlyHint:true`, writes `readOnlyHint:false` and
`idempotentHint:false`; annotations are hints, never permission enforcement.
Malformed calls use protocol invalid-params errors. API failures produce safe
tool errors (`isError:true`) with stable codes such as `access_denied`,
`not_found`, `validation_failed`, `rate_limited`, `upstream_unavailable`.
Never relay upstream bodies/headers or distinguish foreign IDs from absent IDs.
Returned titles and other user text are untrusted data, not instructions.

## Isolation, audit and operational limits

- Do not enumerate projects/tenants or use spectator `/v1/projects/...` routes
  as an authentication fallback. Spectator status grants no MCP permissions;
  authenticated access still requires the explicit project grant. Parent/child
  membership or `manage` authority cannot widen that grant.
- Keep grants, API credentials and remote-client credentials out of project/task
  context and third-party MCP registry DTOs. The planned
  `libs/common-rust/diraigent-types/src/mcp.rs` is absent at this design baseline;
  coordinate with its owning registry task later, without treating registry
  approval/revision fields as external-client consent.
- Record grant creation/revocation, approvals and every tool attempt in an
  access-controlled audit sink: server-generated correlation ID, principal/client,
  tenant/project, tool, grant revision, decision/outcome, resulting entity ID,
  timestamp and latency. Never log payloads, titles, descriptions, query text,
  tokens, headers, raw errors or response snapshots. Use fixed event titles and
  allowlisted metadata. Existing API mutation audits remain independent and
  may retain domain content; do not claim this adapter changes their retention.
  Durable audit acceptance is required before writes; failed audit blocks them.
- Proposed defaults per client + project: 60 reads/minute, 10 writes/minute,
  burst at most those limits, four in-flight calls, 32 KiB input, 128 KiB output,
  10-second upstream deadline. Add a tenant-wide deployment ceiling before
  remote rollout; API quotas/rate limits still apply. Never silently truncate a
  result into a different schema or fetch unbounded pages.
- Cancel pending reads on disconnect. No automatic POST retries: the API has
  no assumed idempotency contract. Timeout after submission returns
  `outcome_unknown` and asks the human to inspect before retrying; cancellation
  cannot promise rollback. Rotate credentials, revoke grants and terminate
  processes/sessions without exposing secret material.

## Future authorized implementation and evidence

One-session outline for an explicitly authorized local prototype (not remote
OAuth/deployment): (1) review/pin SDK and add isolated adapter package;
(2) implement typed closed schemas, injected API client, grant/identity checks
and output projections; (3) add stdio lifecycle and read tools; (4) implement
trusted approval/audit interfaces with default-deny writes, enabling write tools
only when real approved implementations are available; (5) run mock conformance
and security tests, formatting and package Clippy; (6) submit evidence for human
review. Mocks are test-only and must never approve production writes. If consent
infrastructure needs a broader product change, keep writes disabled and request
separate authorization rather than expanding this prototype silently.

Use an in-memory fake MCP client, mock HTTP API, fake clock and captured audit
sink; no real tenant, secrets, package installation or public port required:

1. Negotiate supported/unsupported protocol versions, initialize, list tools,
   call each tool, correlate request IDs, cancel and close cleanly. Assert exact
   advertised names, input/output schemas, annotations and paginated envelopes;
   unknown methods/capabilities cannot expose more tools.
2. For each tool assert exact API method/path/body and server-bound headers.
   Unknown fields, invalid UUIDs/enums, negative pagination and oversized input
   produce no upstream call. Test API validation, quota and malformed responses.
3. Test absent/expired/revoked grants, missing write permission, wrong client,
   spoofed agent, ownerless agent, wrong project/tenant, global-ID foreign lookup
   and foreign list entries. No foreign data reaches the client; preflight
   denials make zero API calls. Test API membership/authority revocation too.
4. Assert no write before trusted consent; reject model confirmation, payload
   changes, expired/replayed approvals and changed policy revisions. Revocation
   during a session blocks the next call. Audit failure causes zero POSTs.
5. Put sentinel secrets in upstream headers, nested context/evidence/metadata
   and error bodies. Assert absence from stdout, stderr, tool results and audit
   events; check no dev headers, spectator fallback or configurable outbound URL.
6. With a fake clock verify limits, concurrency, deadlines and revocation;
   disconnect/timeout never retries POST or reports unconfirmed success. Hostile
   text cannot create new calls or grant approval. For a later HTTP transport,
   separately test audience/issuer/PKCE, redirects, Origin/Host, CSRF/session
   fixation and cross-principal session reuse before deployment.

## Human decisions required before proceeding

Approve implementation scope and SDK/version; local stdio client/executable and
API origin; exact project/agent/tools, expiry and data-disclosure consent;
credential storage and rotation; trusted per-write approval mechanism; audit
retention/access and rate ceilings. Agents cannot self-enable, self-consent or
install this integration. Remote OAuth issuer/delegation, ingress/listener,
deployment environment and external-client onboarding require a **separate**
security review and explicit deployment approval. No decision is implied by
this design document or by approval of Orchestra's consumption integration.

API mappings verified against `apps/api/src/routes/tasks.rs`,
`apps/api/src/routes/observations.rs`, `apps/api/src/models.rs`,
`apps/api/src/authz.rs` and `apps/api/src/repository/tasks.rs` at this baseline.
