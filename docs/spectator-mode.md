# Spectator publication contract (MVP)

Spectator viewing is anonymous, opt-in publication of one project's approved
textual content to **anyone with the link** `/spectate/:projectId`. It is not a
globally read-only project: existing authenticated members retain their normal
permissions. There is no public index, project directory, guest membership,
demo-user impersonation, or general authentication bypass.

## Opt in and revoke

Use the existing authenticated project create/update endpoints and their existing
authorization (project updates require `manage`). Set
`project.metadata.spectator_enabled` to the JSON boolean `true`. Absent or `false`
means private. Other flag types, including null and the string `"true"`, are
validation errors; legacy malformed values never grant public access. Other
metadata validation and persistence semantics are unchanged; no migration or
special settings endpoint is required. Preserve unrelated metadata when updating
according to the existing endpoint's semantics.

Every anonymous request must reload/check the project and owning tenant with
`spectator::ensure_access(&project, tenant.as_ref())` **before loading child
content**. Only the matching tenant with `encryption_mode == "none"` is allowed.
Missing tenants and unknown/encrypted modes fail closed. Never fetch/decrypt keys,
use an unlocked DEK cache, or decrypt tenant content for spectators. Disabling
publication takes effect on subsequent requests; it cannot erase copies already
downloaded. Responses must use `Cache-Control: no-store` to avoid stale public
responses after revocation.

## Read-only API

The route implementation is a dependent task. Agreed GET endpoints:

| Path | DTO |
| --- | --- |
| `/v1/spectator/projects/:projectId` | `PublicProject` |
| `/v1/spectator/projects/:projectId/tasks` | list of `PublicTask` |
| `/v1/spectator/projects/:projectId/tasks/:taskId` | `PublicTaskDetail` |
| `/v1/spectator/projects/:projectId/work` | list of `PublicWork` |
| `/v1/spectator/projects/:projectId/work/:workId` | `PublicWork` |
| `/v1/spectator/projects/:projectId/knowledge` | list of `PublicKnowledge` |
| `/v1/spectator/projects/:projectId/knowledge/:knowledgeId` | `PublicKnowledge` |
| `/v1/spectator/projects/:projectId/decisions` | list of `PublicDecision` |
| `/v1/spectator/projects/:projectId/decisions/:decisionId` | `PublicDecision` |

Lists accept integer `limit` (default `spectator::DEFAULT_LIMIT = 20`, range 1–100,
maximum `spectator::MAX_LIMIT = 100`) and nonnegative integer `offset` (default 0).
Reject invalid/out-of-range values with 400; bound queries before fetching rows.
Use the existing paginated response shape
`{data: [...], total: integer, limit: integer, offset: integer, has_more: boolean}`
and stable ordering with ID as a tie-breaker. Detail responses are plain DTOs.
Missing/private/encrypted projects and missing/cross-project child IDs return
404 with the normal error envelope (`error`, `errorCode`). A child's project ID
must match the URL project; do not trust possession of a child UUID. Unsupported
mutation methods must not write anything. No spectator chat, SSE, or WebSocket.

## Explicit JSON allowlist

All names below are exported from `apps/api/src/spectator.rs`. Construct DTOs
using `From<&Model>` **after** authorization and project-scoping checks. Never
serialize or flatten raw database models.

| DTO | Exact JSON fields |
| --- | --- |
| `PublicProject` | `id`, `name`, `description` (string/null), `spectator` (always true) |
| `PublicTask` | `id`, `number`, `title`, `kind`, `state`, `created_at`, `updated_at`, `completed_at` (timestamp/null) |
| `PublicTaskDetail` | all `PublicTask` fields at top level, `spec` (string/null), `acceptance_criteria` (string array) |
| `PublicWork` | `id`, `title`, `description` (string/null), `status`, `work_type`, `success_criteria` (string array) |
| `PublicKnowledge` | `id`, `title`, `category`, `content`, `tags` (string array) |
| `PublicDecision` | `id`, `title`, `status`, `context` (string), `decision` (string/null), `rationale` (string/null) |

IDs are UUID strings; task numbers are integers; timestamps are UTC RFC 3339.
Task detail selects only the string `context.spec` and textual
`context.acceptance_criteria`. Criteria in task/work projections normalize a
single string to a one-item array; arrays retain only string items; absent/null
or other types become an empty array. No nested objects are published. Malformed
spec values become null. Lists never expose task context.

No identities, ownership, tenant/project relationship fields, arbitrary
metadata/context, assignment, cost, playbook configuration, filesystem paths,
repository configuration, integrations, credentials, source, logs, comments,
artifacts, observations, key material, or additional timestamps are exposed.
The selected text itself is deliberately published, **not automatically scrubbed
for secrets**. Before opting in, managers must review descriptions, task titles,
specs/criteria, work descriptions/criteria, knowledge content/tags, and decision
text for confidential information. Render all published text safely as untrusted
content; this contract does not authorize HTML execution or embedded resource
access.
