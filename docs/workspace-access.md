# Shared workspaces and human project access

Users can belong to several workspaces. The web sidebar's Workspace selector
chooses one; API clients select it with `X-Tenant-Id`. Omitting the header retains
the user's first workspace. Selection always requires membership.

In Settings, Workspace people shows the current user's ID. After a new user has
signed in, a workspace owner can add their ID as a member. Then a project manager
assigns a role under People with project access:

| Project role | Access |
| --- | --- |
| Viewer | Read project data; cannot change data or start chat/tools |
| Editor | Work on tasks, reviews, decisions and project content |
| Manager | Editor access plus project settings, credentials and access management |

Workspace owners and administrators manage every project. Project creators
retain manager access while they belong to the workspace. Other members need
explicit project grants. Removing a grant takes effect on the next request;
leaving a workspace also removes its explicit grants. Access is not inherited
from parent projects. Agent roles and authorities remain separate.

The workspace `viewer` role caps the entire account at read-only, including other
workspaces and agent credentials. Use it for a shared demo login; use workspace
`member` plus project `viewer` when a person needs different rights on other
projects. Neither grants access to every project automatically.

Lists, dashboards and live notifications respect access. Credentials require
management access; account-wide exports require administration of every joined
workspace. Workspace resources such as agent roles and provider defaults require
workspace administration. Sharing encrypted workspaces still requires their
existing encryption-key setup.
Deployment-wide logs, paths and package changes require administration of the
seeded primary workspace; owning a new personal workspace does not grant these.

Migration 050 preserves existing ordinary members' previous access by granting
manager on existing projects. Review those grants when tightening an existing
workspace. Future grants are explicit. No historical migration is modified.

The web app provides workspace switching and permission management. The API
enforces permissions for all clients, including iOS; iOS does not yet provide
the workspace switcher or human access-management screens.
