# Orchestra project content storage

Full Orchestra storage moves project descriptions, task specifications/context,
progress updates, comments, work plans and acceptance criteria, knowledge and its
embedding vectors, decisions, observations/evidence, event bodies, audit snapshots,
reports, verification evidence and webhook payload history to the assigned worker.
Execution logs, diffs, artifacts, personal chat and repository files already live
there. PostgreSQL keeps authentication, authorization, IDs, titles, statuses,
dependencies, timestamps and verified routing references. Small operational
metadata (up to 1 KiB) and task worker controls (up to 4 KiB on new writes) remain
central. Security and integration configuration remain API-owned.

Storage is opt-in. Existing owners retain protocol-1 behavior until a manager
explicitly enables full storage with an updated worker. Existing body data remains
readable during migration; enabling storage does not automatically remove it.

## Ownership and requests

The API checks project permissions and forwards a typed content request over the
owner's authenticated outbound WebSocket. Replies are bound to that worker.
Detailed content fails with an unavailable response when its owner is offline;
another connected worker is not a substitute. Central log/file summaries remain
readable offline. Reads that include externalized fields require the owner. Other workers can
execute tasks and submit content through the API to the owner. This migration changes storage, not task claiming authority.

Ownership records both the agent ID and the durable local store ID. Keep a
separate agent registration for each Orchestra instance. Duplicate active
connections for an agent are rejected. Replacing the owner's data directory
produces an identity mismatch; restore its original store rather than assigning
an empty replacement. Automatic owner transfer and failover are not supported.

Web and iOS use the same authenticated API. Read-only response redaction still
applies to relayed content. The API sees transient content while relaying it;
this is not end-to-end encryption. Encrypted workspaces cannot opt in yet:
activation rejects them instead of silently changing their encryption policy.
Workspace encryption cannot be enabled while an external owner is assigned.

## Rollout

1. Deploy the API and clients, then update the intended Orchestra. Workers report
   `content_protocol: 2` in their metadata and must have their WebSocket enabled.
2. Persist and privately back up the owner's `DATA_DIR`. Project content lives in
   `project-content.db`, independently of operational `orchestra.db` and repos.
3. In project settings, choose the updated owner and select **Use Orchestra
   storage**. New payloads go to it immediately. Existing central records remain
   readable until migrated. Reload the app after changing ownership.
4. Take a private central backup, then select **Move existing content to
   Orchestra**. Each request transfers at most 20 records per entity kind. It
   verifies every remote field before replacing its central body with a reference.
   A compare-and-set preserves edits made after the exported snapshot. Embeddings
   are cleared and referenced in one database transaction. Repeating the operation
   resumes remaining work and does not overwrite previously stored objects.
5. Verify content reads, an owner disconnect/reconnect, a completed chat, and a
   local store backup/restore before removing any old backup.

`GET/PUT /v1/{project}/storage` inspect and assign ownership;
`POST /v1/{project}/storage/migrate` runs a bounded migration batch.
Content endpoints preserve their response shapes: the database adapter hydrates
verified references after checking project access. Anonymous spectator content
checks publication and project scope before hydration. Full-text search and cosine
ranking run on Orchestra; the API sends metadata and current object references,
not every document or vector. Search/ranking currently allow up to 10,000 indexed
records per request and return up to 100 results. During embedding migration the
API uses the legacy compatibility ranking path until all vectors have moved.
Legacy local-mode artifact/changed-file sync is rejected for owned projects;
updated workers use the content-aware endpoints instead.

Migrations 052 and 053 are additive; previously committed migrations are unchanged.
Ownership is opt-in, and no deployment automatically
copies or removes existing project payloads. Stored objects are immutable, so
retrying a transfer is safe. If the central index write fails after a successful
remote write, an unreferenced local object can remain; it is not exposed through
central task/log listings. Local orphan cleanup is deferred.
Immutable field revisions are retained locally
until a separate retention process can safely identify unreferenced versions.

## Chat and retention

Conversations are keyed by project and authenticated internal user ID. The caller
cannot select another user's history. `GET /v1/{project}/chat/history` returns
the durable messages, revision and busy status; POST to its `/clear` suffix
requires the current revision. Sending chat includes `history_revision`.
Orchestra validates the preceding history and exclusively reserves that user's
conversation, saves the new question before execution, then saves completion or
partial output before sending a terminal event. Stale requests and clears during
execution are rejected. A restart releases interrupted conversation reservations.

Clients do not automatically import old device histories, which may lack account
identity. Export any needed device history before enabling ownership. Once
enabled, the web client no longer writes new chat messages to localStorage.

The local store limits individual payloads to 8 MiB and chat inputs to 4 MiB.
Oversized completed responses are replaced with a storage-limit notice. API
request body limits still apply independently.

Back up SQLite using its backup facility or stop Orchestra before copying its
database; copying only an active WAL database file is insufficient. Protect the
data directory and its backups with host permissions and disk encryption.
The content database uses owner-only file permissions and a per-store process
lock, so a second instance cannot release a running conversation's reservation.
Migration moves audit/webhook payload history as well as active content, but does
not purge old browser caches or existing backups. Account/project deletion removes central
access and routing but does not purge local content; local retention and deletion
maintenance are follow-up work. Keep those stores private. Central account
exports contain central records/references, not the external content store. Back up
the owner database together with the central metadata database to preserve a
complete project.
