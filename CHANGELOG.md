# Changelog

## v20260316-0006-developer (2026-03-16)

### Added
- **Orchestra**: Configurable per-project AI providers for plan and chat handlers via project metadata (`plan_provider`, `chat_provider`)
- **Orchestra**: Provider-based chat handler supporting anthropic, openai, ollama, and copilot as alternatives to claude-code CLI
- **Orchestra**: Provider-based plan handler for non-claude-code providers with stateless API calls
- **Web**: Plan Provider and Chat Provider selection dropdowns in Project Settings
- **Web**: Merge conflict indicator badge on work items showing count of tasks with branch conflicts
- **Web**: i18n keys for plan/chat provider settings (English and German)

### Changed
- **Orchestra**: Refactored plan handler into `handle_plan_via_cli` and `handle_plan_via_provider` code paths with shared `send_plan_result` helper
- **CI/CD**: Release script now pushes to all remotes for both release and non-release merges (tags only on release)
- **CI/CD**: Release script merges target back into source branch to keep changelog synchronized

### Fixed
- **Web**: Associated labels with form controls in provider config and event rule forms (accessibility)
- **Web**: Removed unused import in task-detail component

---


## v20260315-2323-developer (2026-03-15)

### Added
- **Web**: Forgejo CI onboarding UI with 3-step setup wizard (connect instance, configure webhook, verify integration)
- **Web**: CI pipelines page with table view, status filters, and auto-polling
- **Web**: Pipeline run detail page with job and step drilldown
- **Web**: CI API service for fetching pipeline data and registering Forgejo integrations
- **Web**: German and English i18n strings for pipelines and Forgejo setup flows

---


## v20260316-developer (2026-03-16)

### Added
- **Analyzer**: sync subcommand that persists analyzer outputs (module summaries, API surface maps) to the knowledge store
- **Analyzer**: API surface mapper subcommand for extracting public endpoint definitions
- **Orchestra**: indexer module with scheduled cron-based re-indexing of codebase knowledge
- **Orchestra**: inject codegen auto-docs from knowledge store into agent task context
- **CI/CD**: post-commit git hook (`scripts/post-commit-index.sh`) to trigger re-indexing after pushes

### Changed
- **Orchestra**: plan handler now invokes claude-code CLI instead of calling the Anthropic API directly

### Fixed
- **Web**: state change dropdown on task cards no longer clipped by overflow-hidden container

---


## v20260315-2151-developer (2026-03-15)

### Added
- **API**: Forgejo CI data model migration with integration, ci_run, ci_job, ci_step tables
- **API**: Forgejo webhook endpoint with HMAC-SHA256 validation and integration tests
- **API**: REST endpoints for CI pipeline data and Forgejo integration registration
- **API**: CI run ingestion and sync service
- **API**: Forgejo REST API v1 client library (`forgejo-client`)
- **API**: Provider configs table with CRUD endpoints, encrypted API key storage, and resolution
- **API**: Atomic apply-plan endpoint to prevent race conditions in plan task dependencies
- **API**: TaskScore with age, priority, goal-alignment, and dependency-graph scoring components
- **API**: `state_entered_at` timestamp on tasks table for staleness tracking
- **Orchestra**: StepProvider trait, ProviderFactory, and step executor routing by provider config
- **Orchestra**: OllamaProvider with streaming NDJSON and typed errors
- **Orchestra**: OpenAI provider with streaming SSE, error mapping, and integration tests
- **Orchestra**: Claude Code and Copilot provider implementations
- **Orchestra**: Plan handler for routing work item planning through orchestra via WebSocket
- **Orchestra**: New `apps/analyzer` tool with API surface mapper and AI-powered module summarizer
- **Web**: CI pipelines page with table view, filters, and auto-polling
- **Web**: Pipeline run detail page with job/step drilldown
- **Web**: Provider Configs UI in Settings with create/edit/delete form
- **Web**: Plan Tasks button and preview dialog on Work view
- **Web**: Execute button on work items; Create & Execute on creation form
- **Web**: Event rules and CI API services

### Changed
- **Orchestra**: Restructured into `engine/`, `git/`, `handlers/`, `project/`, `ws/` submodules
- **Orchestra**: Refactored Anthropic provider for cleaner streaming and error handling
- **Orchestra**: Replaced direct Anthropic API call with `claude -p` subprocess for chat compression
- **Orchestra**: Split monolithic `ws_client` into modular `ws/` with `git_dispatch`
- **API**: Encrypt Forgejo integration token and webhook secret via CryptoDb
- **API**: Authorization checks added to global provider config endpoints
- **Web**: Combined token usage charts into single multi-project graph
- **Web**: Sorted completed work items by `updated_at` descending
- **Web**: Added provider and base_url fields to playbook step schema

### Fixed
- **API**: Authorization gap in agents, members, and roles routes
- **Orchestra**: Merge rollback on push failure and provider routing tests
- **Orchestra**: Monorepo git provisioning using `git_root` instead of `working_dir`
- **Orchestra**: `plan_work` empty response handling with stderr diagnostics
- **Web**: Plan tasks URL now includes `projectId` in path
- **Web**: Clear selected item when opening create form to prevent visual overlap
- **Web**: Mobile horizontal scrolling across all pages
- **Web**: Git push error display showing `[object Object]`

### Removed
- **Orchestra**: Monolithic `ws_client.rs` (replaced by modular `ws/` structure)

---

## v20260315-1702-developer (2026-03-15)

### Added
- **Orchestra**: Step executor routing — dispatch to provider based on step config
- **Orchestra**: OllamaProvider with streaming NDJSON, typed errors, and tests
- **Orchestra**: OpenAI provider with streaming SSE, error mapping, and integration tests
- **Orchestra**: Modularized Anthropic provider with full streaming and error handling
- **API**: Provider configs table with CRUD endpoints, encrypted API key storage, and resolution function
- **API**: Authorization checks on global provider config endpoints
- **API**: Provider and base_url fields on playbook step schema
- **API**: Atomic apply-plan endpoint to prevent race conditions in plan task dependencies
- **API**: State_entered_at timestamp on tasks table for staleness tracking
- **API**: AI planning endpoint routed through orchestra via WebSocket
- **Web**: Plan Tasks button and preview dialog in Work view
- **Web**: Create & Execute button on work item creation form
- **Web**: Execute button on work items that creates a task from work item fields
- **Web**: Combined token usage charts into single multi-project graph
- **Web**: Dependency-graph and goal-alignment scoring components in TaskScore
- **Web**: Composite scoring integrated into ready-tasks query
- **TUI**: Score breakdown display in agent-cli ready command
- **CI/CD**: Version info section in Account Settings
- **CI/CD**: Playwright demo recording spec and mocks

### Changed
- **Orchestra**: Refactored worker to use provider-based routing, simplifying execution logic
- **Web**: Renamed Goals to Work across Angular frontend
- **API**: Renamed Goals to Work in DB migration and API backend
- **TUI**: Renamed Goal to Work in structs, views, API calls, and UI labels
- **Web**: Sort completed work items by updated_at descending (most recent first)
- **Orchestra**: Replaced direct Anthropic API call with claude subprocess for chat history compression
- **Orchestra**: Closed human review → agent rework loop

### Fixed
- **API**: Merge rollback on push failure and broken provider routing tests
- **API**: Plan_work empty response handling with stderr diagnostics
- **Web**: Plan tasks being linked to wrong work item when selection changes during planning
- **Web**: Mobile horizontal scrolling across all pages
- **Web**: Git push error display showing [object Object] in work items UI
- **Web**: Clear selected item when opening create form to prevent visual overlap
- **Orchestra**: ws_client monorepo git provisioning using git_root instead of working_dir

### Removed
- **Web**: Standalone 'new task' button from /work page
- **Web**: Link tasks button from work item detail view
- **Web**: Nested scrolling from task detail view
- **Web**: Lightweight expanded preview from task list

---


## v20260315-1615-developer (2026-03-15)

### Added
- **Web**: "Create & Execute" button on work item creation form — creates the work item and immediately spawns a task in one step

### Changed
- **Orchestra**: Plan request now uses `--json-schema` with a strict schema instead of `--max-tokens` with free-form prompt instructions for task decomposition output

---


## v20260315-1556-developer (2026-03-15)

### Changed
- **Orchestra**: Improved git merge rollback handling on push failure
- **Orchestra**: Enhanced worker provider routing with proper API endpoint mocking
- **Orchestra**: Fixed max tokens configuration

### Fixed
- **API**: Added missing authorization checks to global provider config endpoints
- **Orchestra**: Fixed broken provider routing tests to mock actual provider API endpoints
- **Orchestra**: Fixed git push error handling and rollback logic

---

## v20260315-1437-developer (2026-03-15)

### Added
- **API**: Provider configs table with CRUD endpoints and encrypted API key storage
- **API**: Provider and base_url fields on playbook step schema
- **API**: Atomic apply-plan endpoint to prevent race conditions in plan task dependencies
- **Orchestra**: StepProvider trait with shared types and ProviderFactory
- **Orchestra**: Anthropic provider implementation
- **Orchestra**: OpenAI provider with streaming SSE, error mapping, and integration tests
- **Orchestra**: Ollama provider with streaming NDJSON, typed errors, and tests
- **Orchestra**: Step executor routing — dispatch to provider based on step config
- **Web**: Execute button on work items that creates a task from work item fields
- **Web**: Combined multi-project token usage chart on dashboard

### Changed
- **API**: Observation promotion now creates work item + task instead of just task
- **Web**: Completed work items sorted by updated_at descending (most recent first)
- **Web**: Renamed 'Active Goals' to 'Active Work' in UI labels
- **Web**: Human review → agent rework loop closed in review UI

### Fixed
- **Web**: Plan tasks being linked to wrong work item when selection changes during planning

---

## v20260315-1236 (2026-03-15)

### Added
- **API**: Composite task scoring system (TaskScore) with age, priority, dependency-graph, and goal-alignment scoring components
- **API**: `state_entered_at` timestamp on tasks table for staleness tracking
- **API**: AI planning endpoint `POST /{project_id}/work/{work_id}/plan`
- **API**: Integration tests for scoring module
- **Web**: Execute button on work items to create tasks from work item fields
- **Web**: Plan Tasks button with preview dialog in Work view
- **Orchestra**: Score breakdown display in agent-cli `ready` command
- **Orchestra**: `--verbose` flag for plan request CLI invocation

### Changed
- **Web**: Renamed Goals to Work across the frontend
- **TUI**: Renamed Goal to Work in structs, views, API calls, and UI labels
- **Web**: Moved task statistics section above linked tasks in work item view
- **Web**: Renamed 'Create Task' to 'Add Task', 'P' to 'Priority', 'Todos' to 'Checklist'
- **Orchestra**: Route work item planning through orchestra via WebSocket instead of direct Anthropic API
- **Orchestra**: Replace direct Anthropic API call with `claude -p` subprocess for chat history compression
- **API**: Integrate composite scoring into ready-tasks query
- **API**: Return 502 Bad Gateway for Anthropic API errors per spec
- **CI/CD**: Updated release script

### Fixed
- **Orchestra**: ws_client monorepo git provisioning uses `git_root` instead of `working_dir`
- **Orchestra**: Handle empty Claude CLI output in `plan_work` with stderr diagnostics
- **Web**: Mobile horizontal scrolling across all pages
- **Web**: `goals.type.undefined` field name alignment with API rename
- **Web**: Git push error display showing `[object Object]`
- **Web**: Clear selected item when opening create form to prevent visual overlap
- **API**: Transition task to `human_review` on merge failure

### Removed
- **Web**: Nested scrolling from task detail view
- **Web**: Lightweight expanded preview from task list
- **Web**: Standalone 'new task' button from /work page
- **Web**: `target_date` from work entity
- **Web**: 'Link tasks' button from work detail view

---

## v20260315-1029-developer (2026-03-15)

### Fixed
- **Orchestra**: Use `git_root` instead of `working_dir` in ws_client monorepo git provisioning
- **Orchestra**: Add missing `--verbose` flag to plan request CLI invocation

### Changed
- **Web**: Rename "P" to "Priority" and "Todos" to "Checklist" in work item detail view
- **Web**: Update English and German translations for renamed fields

---

## v20260314-2330 (2026-03-14)

### Added
- **Web**: Plan Tasks button on Work items generates tasks via orchestra with preview dialog
- **Web**: Version info section in Account Settings
- **Orchestra**: Work item planning routed through orchestra for improved reliability
- **Orchestra**: Auto-generate success criteria for work items when empty

### Changed
- **API**: Renamed "Goals" to "Work" across all API endpoints and database schema
- **Web**: Renamed "Goals" to "Work" across the entire UI
- **Web**: Streamlined Work page — removed standalone new-task and link-tasks buttons, removed target date field
- **Orchestra**: Chat history compression now uses claude subprocess instead of direct API call

### Fixed
- **API**: Tasks now correctly transition to `human_review` on merge failure
- **Web**: Fixed horizontal scrolling issues on mobile
- **Web**: Goals UI no longer shows `[object Object]` on git push errors

---

## v20260314-1902 (2026-03-14)

### Added
- **API**: `tokens_per_day` time series added to ProjectMetrics endpoint
- **API**: `POST /{project_id}/goals/{goal_id}/tasks/reorder` for drag-and-drop ordering within goals
- **Web**: Token usage over time chart on dashboard, fetched from metrics API
- **Web**: Toggle chat panel to full-screen mode
- **Web**: Collapsible chat panel (header only)
- **Web**: Logarithmic scale for token usage chart y-axis
- **CI/CD**: `release.sh` generates commit messages and changelog via `claude -p`
- **CI/CD**: Release merges main back into dev so changelog is present in both branches

### Changed
- **CI/CD**: Release workflow retags images instead of rebuilding

### Fixed
- **API**: Tasks now transition to `human_review` on merge failure, creating a review queue item
- **Web**: Goals UI no longer shows `[object Object]` on git push errors
- **Web**: Drag-and-drop snapping fixed for goal reorder

### Removed
- **API**: Plan entity removed entirely; task ordering moved to `position` column on `task_goal` join table
- **Orchestra**: Plan references removed from agent prompts and decompose mode
- **Web**: Plans page, sidebar entry, and all plan references removed from UI
- **TUI**: Plans view and all plan references removed

---

## v20260314-1320 (2026-03-14)

### Added
- **API**: `POST /{project_id}/goals/{goal_id}/activate` to trigger goal processing
- **API**: `intent_type` column on goals; status lifecycle extended with `ready`/`processing` states
- **API**: `GET /tasks/{id}/related` returns contextually related items via text-based relevance matching
- **API**: Full CRUD API for event-observation rules that auto-create observations when matching events fire
- **API**: Event trigger engine automatically creates observations from matching rules
- **API**: Task context now includes relevant knowledge entries and decisions
- **Orchestra**: `.diraigent/` project config with hook scripts and `config.toml` for template selection
- **Orchestra**: Configurable release strategy (built-in templates or custom script)
- **Orchestra**: Orchestra polls for ready goals and auto-creates tasks
- **Orchestra**: Merge, push, revert, and release operations now emit structured events to the API
- **Web**: Show chat model in AI assistant header

### Changed
- **API**: Tasks are now accessed through goals (standalone tasks page removed)
- **Web**: Decisions moved from reference nav group to review queue tab
- **CI/CD**: Monolith release workflow replaced with per-app workflows with change detection
- **CI/CD**: Max 2 runner slots per app instead of 6+ simultaneous builds

### Fixed
- **Web**: Week token count now includes all in-progress tasks regardless of date
- **Web**: Goal drag-and-drop snapping back

---

## v20260314-03 (2026-03-14)

### Added
- **API**: Full CRUD API for plans with ordered task sequences
- **API**: `parent_id` column on tasks with child listing and subtask count
- **API**: `file_scope` column on tasks for branch overlap detection
- **API**: Sequential task queuing for overlapping file scopes
- **API**: Squash-merge dev → main with tagging and multi-remote push
- **API**: Configurable retention period for auto-deleting old observations
- **Orchestra**: File lock acquisition/release for safe parallel task execution
- **Orchestra**: Propagate `parent_id` and `plan_id` when agents decompose subtasks
- **Web**: Plan list page and detail page with task ordering
- **Web**: Task hierarchy views showing parent-child relationships and plan membership
- **Web**: Token usage stats (today/week/total) on dashboard
- **Web**: Jump-to-chat FAB, CDK drag-drop disabled on touch devices, playbook builder responsive grid
- **TUI**: Plans view with task list and progress display
- **TUI**: Task parent-child hierarchy rendering
- **CI/CD**: Release workflow with squash-merge and tagging support

### Changed
- **API**: Replaced numeric priority with boolean `urgent` flag
- **Orchestra**: Priority → urgent flag in CLI and agent prompt
- **Web**: Chat panel converted from floating overlay to always-visible inline panel with collapsible animation
- **Web**: Priority → urgent toggle across all task views
- **Web**: Tool messages collapsed into single spinner indicator
- **TUI**: Priority → urgent toggle
- **CI/CD**: Forgejo release jobs split per architecture
- **CI/CD**: Deploy workflows switched to buildx

### Fixed
- **Orchestra**: Post comment on UnexpectedState outcome for better debugging
- **Web**: Goal drag-and-drop fixes

---

## v20260313-03 (2026-03-13)

### Added
- **API**: `GET /{project_id}/tasks/with-blockers` returns active tasks with blocker updates
- **API**: Per-task review authority check in `bulk_transition_tasks` for review steps
- **API**: Retry backoff unit tests for `retry_api_call`
- **Orchestra**: Goal-based git strategy (`feature_branch`) — tasks branch from goal branch and merge back
- **Orchestra**: Subtasks created by agents automatically inherit goal associations
- **Orchestra**: Observation guidance added to agent CLAUDE.md workflow
- **Web**: Blocker surfacing in review queue with red "Blocked" badge and details in expanded view
- **Web**: Merge conflict resolution with "Merge Conflict" badge and "Resolve Conflict" button
- **Web**: "Feature branch (per goal)" option in playbook builder git strategy dropdown
- **Web**: Goal drag-and-drop reordering with sort_order field and reorder API endpoint
- **Web**: i18n keys for blocker/conflict UI (English + German)
- **CI/CD**: GitHub Actions release workflow with Docker Buildx, GHA layer cache, and GHCR push

### Changed
- **Orchestra**: UPX removed from Containerfiles for faster builds
- **Orchestra**: Updated blocker handling

---

## v0.2.0 (2026-03-12)

### Added
- **API**: Goals promoted to first-class containers with `goal_type`, `priority`, `parent_goal_id` hierarchy, and `auto_status`
- **API**: Goal stats endpoint with task state breakdown, cost, token usage, and completion metrics
- **API**: Goal children endpoint for navigating goal hierarchies
- **API**: Goal comments with CRUD endpoints for discussion threads
- **API**: Bulk link/unlink tasks to goals, atomic `goal_id` on task creation, searchable task picker
- **API**: Reusable step template library with CRUD, fork, and playbook builder integration
- **API**: Playbook versioning with parent tracking and sync endpoint
- **API**: Copy-on-write default playbooks (editing auto-clones)
- **API**: Task tracking/flagging with flagged endpoint
- **API**: Merge conflict detection and resolve action
- **API**: Task `reverted_at` field and visual indicator
- **API**: SSE push for real-time agent status updates
- **API**: Configurable settings (done task retention, upload logs, auto-push after merge)
- **API**: 9 new PostgreSQL migrations (007–019)
- **Orchestra**: Auto-push after `merge_to_main`
- **Orchestra**: Retry logic for `transition_task` and `get_task`/`get_playbook`
- **Orchestra**: Loop detection with configurable `max_implement_cycles`
- **Orchestra**: Dream step template with `test_cmd` and spec fields
- **Orchestra**: Acceptance criteria and files surfaced inline in agent prompt
- **Orchestra**: Comprehensive unit and integration test suite
- **Web**: Catppuccin theming — all 4 flavors, 14 accent colors, per-tenant sync
- **Web**: Per-tenant settings page for appearance and encryption
- **Web**: Accordion views for tasks, goals, decisions, knowledge, and observations
- **Web**: Goal management overhaul with inline editing, statistics filters, task marking, visual status indicators
- **Web**: Task detail inline editing for title/kind/priority/spec, lifecycle dropdown, playbook management
- **Web**: Step template library integrated into playbook builder
- **Web**: Searchable task picker modal for linking tasks to goals
- **Web**: Scratchpad with notes and todos ("Promote to Task" action)
- **Web**: Agents & Team merged into single tabbed page
- **Web**: Source page with helpful empty state
- **Web**: WebSocket client ping for connection keepalive
- **TUI**: Goals view updates

### Changed
- **Web**: Logs moved under integrations (visible only with logging integration)
- **Web**: Mobile responsiveness improvements across all pages
- **Web**: Nginx realip for accurate client IP behind load balancers
- **Web**: Expanded English and German translations

### Fixed
- **API**: Playbook step bounds validation prevents out-of-bounds transitions
- **API**: Atomic pipeline advancement — `done` is now terminal-only
- **API**: Fixed `agent_id=None` authorization bypass, added scope guardrails
- **Orchestra**: Step regression finds nearest previous implement step

---

## v0.1.0 (2026-03-03)

Initial release.

### Added
- **API**: Dual database backend — PostgreSQL (production) and SQLite (zero-config local dev)
- **API**: Task state machine with playbook-driven multi-step pipelines
- **API**: Project hierarchy with role-based access control (6 authority levels)
- **API**: Knowledge base, decision log, observations, goals, and milestones
- **API**: Integration registry and event/signal system
- **API**: Agent registration, heartbeat, and stale detection
- **API**: Webhook delivery with HMAC-SHA256 signatures and retry
- **API**: NATS JetStream event bus with audit-logger and webhook-dispatcher consumers
- **API**: JWT JWKS authentication with dev-mode bypass
- **API**: Rate limiting (100 req/60s per IP)
- **API**: Health probes (`/health/live`, `/health/ready`)
- **API**: OpenTelemetry metrics middleware
- **API**: 20 PostgreSQL migrations, 1 consolidated SQLite migration
- **Orchestra**: Polls API for ready tasks and spawns Claude Code CLI workers
- **Orchestra**: Isolated git worktree per task — auto-creates branch, merges on completion
- **Orchestra**: Per-step configuration — model, budget, tool preset, MCP servers, sub-agents, env vars
- **Orchestra**: Automatic playbook step advancement
- **Orchestra**: `agent-cli` binary for manual agent interaction
- **Orchestra**: NATS chat listener for real-time communication
- **Orchestra**: Loki log shipping
- **Web**: Angular 21 SPA with Tailwind CSS 4 and Catppuccin theme
- **Web**: Full project management UI — tasks, goals, knowledge, decisions, observations, playbooks
- **Web**: Agent monitoring with health indicators
- **Web**: Audit log viewer
- **Web**: Chat interface (NATS-backed)
- **Web**: OAuth2/OIDC authentication
- **Web**: i18n support (English, German)
- **TUI**: Ratatui terminal interface (experimental)
- **TUI**: Task, agent, playbook, and audit views
