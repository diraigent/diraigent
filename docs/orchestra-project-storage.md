# Orchestra project content storage

This first stage moves execution log bodies and metadata, stored diffs, artifact
update bodies and metadata, and personal chat history to an explicitly assigned
Orchestra. Tasks, ordinary progress updates, comments, plans, knowledge and
permissions remain in the API. Existing projects retain central storage until
a manager assigns an owner in project settings.

## Ownership and requests

The API checks project permissions and forwards a typed content request over the
owner's authenticated outbound WebSocket. Replies are bound to that worker.
Detailed content fails with an unavailable response when its owner is offline;
another connected worker is not a substitute. Central log/file summaries remain
readable offline. Other workers can execute tasks and submit content through the
API to the owner. This migration changes storage, not task claiming authority.

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
   `content_protocol: 1` in their metadata and must have their WebSocket enabled.
2. Persist and privately back up the owner's `DATA_DIR`. Project content lives in
   `project-content.db`, independently of operational `orchestra.db` and repos.
3. In project settings, choose the updated owner and select **Use Orchestra
   storage**. New payloads go to it immediately. Existing central records remain
   readable until migrated. Reload the app after changing ownership.
4. Take a private central backup, then select **Move existing content to
   Orchestra**. Each request transfers at most 20 records per payload kind. It
   verifies the remote copy before atomically clearing the central payload and
   inserting its routing reference. Repeating the operation resumes remaining
   work and does not overwrite previously stored objects.
5. Verify content reads, an owner disconnect/reconnect, a completed chat, and a
   local store backup/restore before removing any old backup.

`GET/PUT /v1/{project}/storage` inspect and assign ownership;
`POST /v1/{project}/storage/migrate` runs a bounded migration batch.
The log, changed-file and task-update endpoints preserve their response shapes.
Legacy local-mode artifact/changed-file sync is rejected for owned projects;
updated workers use the content-aware endpoints instead.

Migration 052 is additive. Ownership is opt-in, and no deployment automatically
copies or removes existing project payloads. Stored objects are immutable, so
retrying a transfer is safe. If the central index write fails after a successful
remote write, an unreferenced local object can remain; it is not exposed through
central task/log listings. Local orphan cleanup is deferred.

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
Migration does not purge historical audit/webhook snapshots, delivery logs,
old browser caches or existing backups. Account/project deletion removes central
access and routing but does not purge local content; local retention and deletion
maintenance are follow-up work. Keep those stores private. Central account
exports contain central records, not the external content store.
