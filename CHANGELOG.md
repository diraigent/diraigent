# Changelog

## v20260313-03 — 2026-03-13

### API
- **Blocker surfacing endpoint**: `GET /{project_id}/tasks/with-blockers` returns active tasks that have `kind=blocker` task updates (excludes done/cancelled)
- **Per-task review authority check** in `bulk_transition_tasks` for review steps
- **Retry backoff** unit tests for `retry_api_call`

### Orchestra
- **Goal-based git strategy** (`feature_branch`): tasks branch from a goal branch (e.g. `goal/<slug>`) and merge back into it; the goal branch merges to default when the goal completes
- **Goal association inheritance**: subtasks created by agents automatically inherit goal associations
- **Observation guidance** added to agent CLAUDE.md workflow
- **UPX removed** from Containerfiles for faster builds
- Updated blocker handling

### Web Dashboard
- **Review queue: blocker surfacing** — tasks with blocker updates now appear in the Review Queue tab alongside human_review tasks, with red "Blocked" badge and blocker details in expanded view
- **Review queue: merge conflict resolution** — "Merge Conflict" badge and "Resolve Conflict" button for tasks with git branch conflicts
- **Playbook builder: feature branch strategy** — "Feature branch (per goal)" option added to git strategy dropdown with tooltip description
- **Goal drag-and-drop reordering** with sort_order field and reorder API endpoint
- i18n keys added for blocker/conflict UI (English + German)

### CI/CD
- **GitHub Actions release workflow** (`.github/workflows/release-diraigent.yml`) with Docker Buildx, GHA layer cache, and GHCR push
- GitHub release version bump

---

## v0.2.0 — 2026-03-12

### API
- **Goal epics**: Goals promoted to first-class containers with `goal_type` (epic, feature, milestone, sprint, initiative), `priority`, `parent_goal_id` hierarchy, and `auto_status` derivation from linked tasks
- **Goal stats endpoint**: `/goals/{id}/stats` returns task state breakdown, cost, token usage, blocked count, and completion metrics
- **Goal children endpoint**: `/goals/{id}/children` for navigating goal hierarchies
- **Goal comments**: New `goal_comment` table with CRUD endpoints for discussion threads on goals
- **Task-goal linking**: Bulk link/unlink tasks to goals, atomic `goal_id` on task creation, and searchable task picker endpoint
- **Step templates**: Reusable step template library with CRUD, fork, and integration into playbook builder
- **Playbook versioning**: Parent tracking and sync endpoint for playbook version management
- **Copy-on-write default playbooks**: Tenant default playbooks are immutable; editing auto-clones
- **Task tracking/flagging**: New flagged endpoint and task tracking indicators
- **Merge conflict detection**: Detect and resolve action for stranded task branches
- **Task reverted_at**: New field and visual indicator for reverted tasks
- **Playbook step bounds validation**: Prevent out-of-bounds `playbook_step` in transitions
- **Atomic pipeline advancement**: `done` is now terminal-only; step transitions are atomic
- **Security hardening**: Fixed `agent_id=None` authorization bypass, added scope guardrails
- **Cleanup endpoints**: Cleanup acknowledged observations
- **SSE push**: Real-time agent status updates via Server-Sent Events
- **Configurable settings**: Done task retention period, upload logs toggle, auto-push after merge
- **9 new migrations** (007–019): theme preferences, scratchpad, observations, reports, task logs, reverted_at, step templates, playbook versions, goal epics, goal comments

### Orchestra
- Auto-push after `merge_to_main`
- Retry logic for `transition_task` and `get_task`/`get_playbook` in `check_next_step`
- Loop detection threshold for `spawn_worker` with configurable `max_implement_cycles`
- Step regression finds nearest previous implement step
- Dream step template with `test_cmd` and spec fields
- Acceptance criteria and files surfaced inline in agent prompt
- Comprehensive unit and integration test suite added

### Web Dashboard
- **Catppuccin theming**: All 4 flavors (Latte, Frappé, Macchiato, Mocha), 14 accent colors, per-tenant sync
- **Per-tenant settings page**: Appearance and encryption configuration
- **Accordion views**: Tasks, goals, decisions, knowledge, and observations converted from table/card to accordion pattern
- **Goal management overhaul**: Inline-editable goal details, clickable statistics filters, task marking/bookmarking, create task from goal, visual status indicators (green/yellow bar), achieved goals sorted to bottom
- **Task detail improvements**: Inline editing for title/kind/priority/spec, clickable state badge lifecycle dropdown, playbook management, error detection in task updates
- **Step template library**: Browse, create, edit, fork, delete — integrated into playbook builder
- **Task picker**: Searchable multi-select modal for linking tasks to goals with unlinked-only filter
- **Scratchpad**: Notes (markdown) and todos with "Promote to Task" action
- **Agents & Team merged** into single tabbed page
- **Logs moved** under integrations (visible only with logging integration)
- **Source page**: Helpful empty state when no repo path configured
- **Mobile responsiveness** improvements across all feature pages
- **Nginx realip**: Accurate client IP resolution behind load balancers
- **i18n**: Expanded English and German translations
- **WebSocket client ping** for connection keepalive

### TUI
- Goals view updates

---

## v0.1.0 — 2026-03-03

Initial release.

### API
- Dual database backend: PostgreSQL (production) and SQLite (zero-config local dev)
- Task state machine with playbook-driven multi-step pipelines
- Project hierarchy with role-based access control (6 authority levels)
- Knowledge base, decision log, observations, goals, and milestones
- Integration registry and event/signal system
- Agent registration, heartbeat, and stale detection
- Webhook delivery with HMAC-SHA256 signatures and retry
- NATS JetStream event bus with audit-logger and webhook-dispatcher consumers
- JWT JWKS authentication with dev-mode bypass
- Rate limiting (100 req/60s per IP)
- Health probes (`/health/live`, `/health/ready`)
- OpenTelemetry metrics middleware
- 20 PostgreSQL migrations, 1 consolidated SQLite migration

### Orchestra
- Polls API for ready tasks and spawns Claude Code CLI workers
- Isolated git worktree per task — auto-creates branch, merges on completion
- Per-step configuration: model, budget, tool preset, MCP servers, sub-agents, env vars
- Automatic playbook step advancement
- `agent-cli` binary for manual agent interaction (claim, transition, progress, artifact, etc.)
- NATS chat listener for real-time communication
- Loki log shipping

### Web Dashboard
- Angular 21 SPA with Tailwind CSS 4 and Catppuccin theme
- Full project management UI: tasks, goals, knowledge, decisions, observations, playbooks
- Agent monitoring with health indicators
- Audit log viewer
- Chat interface (NATS-backed)
- OAuth2/OIDC authentication
- i18n support (English, German)

### TUI
- Ratatui terminal interface (experimental)
- Task, agent, playbook, and audit views
