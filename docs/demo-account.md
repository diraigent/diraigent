# Authenticated read-only demo

A demo account signs in through the normal OAuth provider and browses the normal
application. Give it a `viewer` tenant membership in a dedicated workspace with
synthetic demo data. A viewer membership makes the entire account read-only,
including any agent API keys owned by it, so it cannot create another workspace
or use agent execution to escape the restriction.

The API rejects mutation requests before route handlers execute. Viewer accounts
cannot connect worker WebSockets, obtain live-stream tickets, export account data,
or read encryption keys/provider credentials. Ordinary GET/HEAD project browsing
still follows tenant access checks. Account permissions are checked from the
database on each authenticated request; changing a role does not wait for JWT
expiry. `/v1/account` reports `read_only` for clients.

Use an unencrypted dedicated demo workspace, without live integrations or real
credentials. A read-only account sees the normal workspace's data, unlike the
separate anonymous spectator feature's reduced public projections. Do not add
the shared demo login to a workspace containing private information.

Provision the OAuth login first, map its verified subject to `auth_user`, then
give it only a `viewer` membership before allowing first login. Otherwise the
normal first-login flow creates an owner workspace. A separate administrator
owns and maintains the demo workspace. Credentials should be provided privately,
never stored in repository fixtures or documentation.

For a demo of an existing project, add only its explicit `viewer` project grant
as described in [Shared workspace access](workspace-access.md). Other projects
in the workspace stay inaccessible. The demo sees the selected project's normal
views. Read-only responses redact
credential fields and recognizable private data in nested content, audit snapshots,
stored diffs, logs and source files. Original stored data and authorized editor/manager
responses remain intact. Set `READ_ONLY_REDACT_TERMS` in private deployment
configuration to cover private hostnames or other installation-specific identifiers.
Encoded text source is decoded, redacted and re-encoded; binary source content is
replaced with a notice. Responses larger than 16 MiB fail closed. This is a defense
in depth, not proof that arbitrary free text is suitable for sharing.

The web and iOS clients display read-only status and omit chat and editing
controls. iOS verifies both account and selected-project access, stays read-only
if permission discovery fails, and clears cached content on logout. The Logs
view shows the selected project's stored execution logs to viewers; deployment-wide
Loki queries remain restricted to workspace administrators. Source, audit, diffs
and execution logs keep their normal views with API-redacted content. The default
redaction also covers local hostnames, private IPv6 and Tailscale address ranges,
and quoted credentials containing spaces.

Regression coverage lives in `apps/api/tests/read_only_account.rs`,
`apps/web/e2e/read-only-account.spec.ts`, and the iOS authentication and demo UI
tests. API tests use isolated PostgreSQL databases and real worker-key authentication
to check project isolation and prevent a viewer's previously issued key from
executing tools. Browser and simulator UI fixtures use synthetic, already-redacted
content; they validate rendering and controls, while API tests validate redaction.

Migration 049 adds the role without modifying earlier migrations. It does not
create a login, grant access, or publish projects. Existing spectator opt-ins
remain explicit and independent of this account.
