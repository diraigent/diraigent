# Changelog

## v20260315-1029-developer (2026-03-15)

### Orchestra
- **Git provisioning fix**: Use `git_root` instead of `working_dir` in ws_client monorepo git provisioning
- **Plan request verbosity**: Add missing `--verbose` flag to plan request CLI invocation

### Web Dashboard
- **Field label rename**: Rename "P" to "Priority" and "Todos" to "Checklist" in work item detail view
- **i18n updates**: Update English and German translations for renamed fields

---

## v20260314-2330 (2026-03-14)

### API
- **Goals → Work rename**: Renamed "Goals" to "Work" across all API endpoints and database schema
- **Fix merge failure handling**: Tasks now correctly transition to `human_review` on merge failure

### Orchestra
- **Chat compression**: Chat history compression now uses claude subprocess instead of direct API call
- **Planning via orchestra**: Work item planning routed through orchestra for improved reliability
- **Auto-generate success criteria**: Planning auto-generates success criteria for work items when empty

### Web Dashboard
- **Goals → Work rename**: Renamed "Goals" to "Work" across the entire UI
- **AI task planning**: Plan Tasks button on Work items generates tasks via orchestra with preview dialog
- **Version info**: Added version info section to Account Settings
- **Fix mobile scrolling**: Fixed horizontal scrolling issues on mobile
- **Fix error display**: Goals UI no longer shows `[object Object]` on git push errors
- **Streamlined Work page**: Removed standalone new-task and link-tasks buttons, removed target date field

---

## v20260314-1902 (2026-03-14)

### API
- **Remove Plan entity**: Plans removed entirely; task ordering moved to `position` column on `task_goal` join table
- **Goal task reorder endpoint**: `POST /{project_id}/goals/{goal_id}/tasks/reorder` for drag-and-drop ordering within goals
- **Token time series**: `tokens_per_day` added to ProjectMetrics endpoint
- **Merge failure → human_review**: Tasks now transition to `human_review` on merge failure, creating a review queue item

### Orchestra
- **Plan references removed**: Agent prompts and decompose mode no longer reference plans
- **Merge failure review**: Merge failures create observations and transition tasks to human_review for visibility

### Web Dashboard
- **Token usage chart**: Token usage over time chart on dashboard, fetched from metrics API
- **Fullscreen chat**: Toggle chat panel to full-screen mode
- **Collapsible chat panel**: Chat panel can be collapsed to header only
- **Logarithmic chart scale**: Token usage chart y-axis uses logarithmic scale
- **Plans removed**: Plans page, sidebar entry, and all plan references removed from UI
- **Fix git push error**: Goals UI no longer shows `[object Object]` on git push errors
- **Fix goal reorder**: Drag-and-drop snapping fixed

### TUI
- **Plans removed**: Plans view and all plan references removed

### CI/CD
- **Automated release**: `release.sh` generates commit messages and changelog via `claude -p`
- **Image retagging**: Release workflow retags images instead of rebuilding
- **Changelog in dev**: Release merges main back into dev so changelog is present in both branches

---

## v20260314-1320

### API
- **Goal activation endpoint**: `POST /{project_id}/goals/{goal_id}/activate` to trigger goal processing
- **Goal intent types**: `intent_type` column on goals; status lifecycle extended with `ready`/`processing` states
- **Related items endpoint**: `GET /tasks/{id}/related` returns contextually related items via text-based relevance matching
- **Event-observation rules**: Full CRUD API for rules that auto-create observations when matching events fire
- **Event trigger engine**: Automatically creates observations from matching event-observation rules
- **Enriched agent context**: Task context now includes relevant knowledge entries and decisions
- **Tasks page removed**: Tasks are now accessed through goals

### Orchestra
- **`.diraigent/` project config**: Repos can now include a `.diraigent/` folder with hook scripts (`release.sh`, etc.) and a `config.toml` for template selection
- **Configurable release strategy**: Release can use built-in templates (`squash-merge`, `merge-commit`, `tag-only`) or a custom script with env vars (`DIRAIGENT_PROJECT_PATH`, `DIRAIGENT_BRANCH`, `DIRAIGENT_TARGET_BRANCH`, `DIRAIGENT_VERSION`)
- **Goal processing**: Orchestra polls for ready goals and auto-creates tasks
- **Git event emission**: Merge, push, revert, and release operations now emit structured events to the API

### Web Dashboard
- **Show chat model** in AI assistant header
- **Decisions moved** from reference nav group to review queue tab
- **Fix week token count** including all in-progress tasks regardless of date
- **Fix goal drag-and-drop** snapping back

### CI/CD
- **Split release workflows**: Monolith `release-diraigent.yml` replaced with per-app workflows (`release-api.yml`, `release-orchestra.yml`, `release-web.yml`) with change detection — unchanged apps skip builds entirely
- **Reduced runner contention**: Max 2 runner slots per app instead of 6+ simultaneous builds

---

## v202603014-03 — 2026-03-14

### API
- **Plan entity**: Full CRUD API with ordered task sequences
- **Task parent-child hierarchy**: `parent_id` column, child listing, subtask count
- **File scope**: `file_scope` column on tasks for branch overlap detection
- **Sequential task queuing**: Overlapping file scopes are queued instead of running in parallel
- **Priority → urgent toggle**: Replaced numeric priority with boolean `urgent` flag
- **Auto-delete old observations**: Configurable retention period
- **Release feature**: Squash-merge dev → main with tagging and multi-remote push

### Orchestra
- **File lock acquisition/release** for safe parallel task execution
- **Priority → urgent flag** in CLI and agent prompt
- **Propagate parent_id and plan_id** when agents decompose tasks into subtasks
- **Post comment on UnexpectedState** outcome for better debugging

### Web Dashboard
- **3-panel layout redesign**: Chat panel converted from floating overlay to always-visible inline panel with collapsible animation
- **Plan management**: Plan list page, detail page with task ordering
- **Task hierarchy views**: Parent-child relationships and plan membership displayed
- **Priority → urgent toggle** across all task views
- **Token usage stats**: Today/week/total on dashboard
- **Mobile improvements**: Jump-to-chat FAB, CDK drag-drop disabled on touch devices, playbook builder responsive grid
- **Chat UX**: Tool messages collapsed into single spinner indicator
- **Goal drag-and-drop fixes**

### TUI
- **Plans view** with task list and progress display
- **Task parent-child hierarchy** rendering
- **Priority → urgent toggle**

### CI/CD
- **Release workflow**: Added release button with squash-merge and tagging support
- Forgejo release jobs split per architecture
- Deploy workflows switched to buildx

---

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
