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

Migration 049 adds the role without modifying earlier migrations. It does not
create a login, grant access, or publish projects. Existing spectator opt-ins
remain explicit and independent of this account.
