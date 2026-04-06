# Changelog

## v20260406-2342 (2026-04-06)

### Fixed
- **Orchestra**: SQLite now bundled correctly for cross-compiled targets
- **CI/CD**: Various pipeline fixes

---


## v20260326-0929 (2026-03-26)

### Added
- **iOS**: Native iOS app with full feature coverage: dashboard, work items, tasks, agents, chat, events, reports, observations, decisions, integrations, webhooks, logs, source browser, search, git status, and settings
- **iOS**: OIDC/PKCE authentication with keychain token storage
- **Orchestra**: Repo-as-code support — sync playbooks, decisions, knowledge, and observations from `.diraigent/` directory
- **Orchestra**: Headless mode for CLI-driven pipeline execution without API
- **TUI**: Model selector in chat (cycle between sonnet/opus/haiku)
- **TUI**: Global keyboard shortcuts with help overlay
- **TUI**: API token management in account settings

### Changed
- **TUI**: Merged work and tasks into a single unified work view
- **TUI**: Work items sorted by status (active → paused → achieved → abandoned)
- **TUI**: Simplified navigation by removing standalone playbooks, tasks, verify, knowledge, and teams views

### Fixed
- **API**: Membership unique constraint now scoped to tenant, preventing onboarding conflicts
- **Orchestra**: Various pipeline resolution and task sourcing improvements

---

## v20260320-0042 (2026-03-20)

### Added
- **API**: Multi-provider support — configure OpenAI and Ollama alongside Anthropic
- **API**: Forgejo and GitHub CI integrations with webhook sync
- **API**: OpenAPI/Swagger documentation endpoint
- **API**: AI-powered work planning with task generation
- **Orchestra**: Codebase static analyzer with dependency graphs and API surface mapping
- **Orchestra**: Scheduled repository re-indexing via cron
- **Orchestra**: Composite task scoring for smarter prioritization
- **Web**: CI pipelines page with run details, filters, and auto-polling
- **Web**: Provider configuration UI in project settings
- **Web**: Execute and Plan & Execute actions on work items
- **Web**: Global keyboard shortcuts with help overlay
- **Web**: Onboarding wizard and API token management in account settings
- **Web**: Active Work dashboard section across projects
- **TUI**: Unified work + tasks split view with progress indicators

### Changed
- **All**: Goals renamed to Work across the entire stack
- **TUI**: Removed playbooks, verify, knowledge, and teams views
- **Web**: Streamlined work detail layout — inline title editing, movable type badge
- **CI/CD**: Binaries and containers are now signed with cosign

### Fixed
- **Web**: Mobile horizontal scrolling, dropdown clipping, and light-mode background
- **Orchestra**: Deterministic graph cycle detection, merge rollback on push failure
- Various UI and stability fixes across Web and TUI

---

## v20260320-0041 (2026-03-20)

### Added
- **Web**: CI pipelines page with Forgejo and GitHub integration setup wizards
- **Web**: Provider configuration UI in project settings (API keys, models, base URLs)
- **Web**: OpenAPI/Swagger documentation UI for the Diraigent API
- **Web**: Global keyboard shortcuts with help overlay
- **Web**: Onboarding modal and API token management in account settings
- **Web**: Active Work dashboard section with cross-project work items
- **API**: Forgejo webhook integration with HMAC-SHA256 validation
- **API**: GitHub CI integration backend with webhooks and sync
- **API**: Multi-provider support (OpenAI, Ollama) with encrypted API key storage
- **Orchestra**: Codebase static analyzer with dependency graphs and module summaries
- **Orchestra**: Composite task scoring for smarter task prioritization
- **TUI**: API token management for streamlined onboarding

### Changed
- **Web**: Merged work and tasks into a unified work view with integrated task detail
- **Web**: Streamlined work item UI: inline title editing, editable type badge, reorganized action buttons
- **Orchestra**: Work planning now routed through orchestra with configurable per-project providers
- **TUI**: Simplified navigation by removing playbooks, verify, knowledge, and teams views

### Fixed
- **Web**: Light mode background, dropdown clipping, mobile horizontal scrolling
- **Orchestra**: Deterministic graph cycle detection, merge rollback on push failure
- **API**: Authorization checks on global provider config endpoints
- Various UI and stability fixes

### Removed
- **TUI**: Standalone tasks, playbooks, verify, knowledge, and teams views
- **Web**: Manual link-to-work functionality and redundant status filters

---

## v20260319-1103 (2026-03-19)

### Added
- **Web**: Logout page with session cleanup

### Changed
- **TUI**: Simplified view layout and reordered navigation
- **Web**: Streamlined dashboard view

### Removed
- **API**: Removed unused query methods from tasks and work repositories
- **TUI**: Removed dead code and redundant imports

---


## v20260318-2155 (2026-03-18)

### Changed
- **API**: Improved related-item query handling
- **Orchestra**: Refined chat handler response logic

### Fixed
- **Web**: Minor cleanup in work module

---

## v20260318-2146 (2026-03-18)

### Changed
- **CI/CD**: Unified deploy and release pipelines into single workflow per service

### Fixed
- **Web**: Fix minor build issue on work page

---

## v20260318-2117 (2026-03-18)

### Added
- **API**: OpenAPI/Swagger UI for browsing the API interactively
- **API**: Work status counts endpoint for section header aggregation
- **Web**: Inline title editing on work list via double-click
- **Web**: Editable work type badge in work item header
- **Web**: Status counts shown in work list section headers
- **TUI**: Create-task keybinding [t] on work page
- **TUI**: Work progress indicators (done/total) in work list
- **TUI**: Keybindings for execute, plan+execute, and save in work form
- **TUI**: API token management in account settings for onboarding

### Changed
- **Web**: Rename work status buttons to Done, Pause, Abandon
- **TUI**: Default view changed from Tasks to Work
- **TUI**: Work items sorted by status (active → paused → achieved → abandoned)
- **Orchestra**: Improved chat handler with better model routing

### Fixed
- **TUI**: Light mode background now fills frame with correct base color
- **TUI**: Work list scrolling keeps selected item visible

### Removed
- **Web**: Status filter dropdown from work page
- **Web**: Unused agents, scratchpad, and status-badge components

---


## v20260318-2000 (2026-03-18)

### Added
- **TUI**: Create-task keybinding [t] on work page
- **TUI**: Scrolling in work list keeps selected item visible
- **TUI**: Work progress indicator (done/total) on work items
- **TUI**: Keybindings for execute, plan+execute, and save in work form
- **Web**: Global keyboard shortcuts with navigation keys and help overlay
- **Web**: Inline title editing on work list header via double-click

### Changed
- **TUI**: Default view changed from Tasks to Work
- **TUI**: Work items sorted by status (active → paused → achieved → abandoned)
- **Web**: Status buttons renamed to Done, Pause, Abandon

### Removed
- **TUI**: Manual link-to-work functionality
- **Web**: Status filter dropdown from work page
- **Web**: Dead task picker code

### Fixed
- **TUI**: Light mode background now fills frame with correct base color

---

## v20260318-1810 (2026-03-18)

### Added
- **Web**: Global keyboard shortcuts with navigation keys and help overlay
- **Web**: Inline title editing on work list header via double-click
- **Web**: Editable work type badge in work item header
- **API**: Release tag endpoint for git operations

### Changed
- **Web**: Reorganized work detail layout — action buttons moved to status transitions row
- **Web**: Moved add task button to top action bar in work detail
- **Web**: Moved delete button next to status transition buttons
- **Web**: Create work modal buttons now match work detail style
- **Web**: Achieved status button uses green styling
- **Orchestra**: Improved release script with better tag and changelog handling

### Removed
- **Web**: Redundant title and type fields from work details
- **Web**: Standalone save button from work detail view

---

## v20260318-0719 (2026-03-18)

### Added
- **API**: OpenAPI/Swagger UI for interactive API documentation
- **API**: Tenant quota system with configurable resource limits
- **CI/CD**: Additional CI image build targets

### Changed
- **API**: Rate limiting now integrates with tenant quotas
- **API**: Route annotations updated for OpenAPI spec generation

### Fixed
- **API**: Deterministic output in dependency graph cycle detection
- **Web**: Remove unused imports causing dashboard warnings
- **API**: Various stability and model fixes

---

## v20260317 (2026-03-17)

### Added
- **Web**: CI pipelines page with filters, auto-polling, and run detail drilldown
- **Web**: Forgejo and GitHub CI onboarding setup wizards
- **Web**: Provider configuration UI in project settings
- **Web**: Active Work section on dashboard with cross-project items
- **Web**: Plan Tasks button and preview dialog for work items
- **Web**: Acceptance criteria field on work items
- **Web**: Deep-link support for work items via query params
- **Web**: Version info in account settings
- **Web**: Multi-step agent onboarding wizard with provider setup
- **API**: Multi-provider support (OpenAI, Ollama) with per-project configuration
- **API**: Forgejo CI integration with webhook ingestion and sync
- **API**: GitHub CI integration with webhooks and registration
- **API**: AI planning endpoint for work items with auto-generated success criteria
- **API**: Composite task scoring (age, priority, goal-alignment, dependency-graph)
- **API**: Account deletion and full user data download
- **API**: User registration flow
- **API**: Auto-unlock tenant encryption using stored wrapped keys
- **Analyzer**: Static code analyzer with dependency graphs, API surface mapping, and module summaries
- **Orchestra**: Configurable per-project AI providers with step executor routing
- **Orchestra**: Scheduled re-indexing via cron and git hooks
- **TUI**: Standalone TUI and API binary builds
- **CI/CD**: Container and binary signing with cosign and GPG
- **CI/CD**: GitHub Actions release workflow for binaries

### Changed
- **All**: Renamed "Goals" to "Work" across entire stack (Web, API, Orchestra, TUI)
- **Orchestra**: Plan and chat handlers configurable per project via metadata
- **Orchestra**: Switched to model-agnostic architecture with claude-code CLI
- **Web**: Combined token usage into single multi-project chart
- **API**: Agent registration now auto-assigns a default role

### Fixed
- **Web**: Mobile horizontal scrolling and dropdown clipping issues
- **Orchestra**: Merge rollback on push failure
- **API**: Authorization checks on provider config endpoints
- **API**: Encrypted integration tokens and fixed authorization gaps
- **Forgejo Client**: Correct actions API endpoint URLs

### Removed
- **API**: Unused subtask and work-task count endpoints
- **Orchestra**: Dead code cleanup

---

## v20260316 (2026-03-16)

### Added
- **Web**: Active Work dashboard section with cross-project work items
- **Web**: "Plan & Execute" option in work item creation
- **Web**: Deep-link support for work items via query parameter
- **Web**: Lazy loading for completed and archived work sections
- **Web**: Unmerged branch and merge conflict indicators on work items
- **Web**: Acceptance criteria field on tasks
- **Web**: GitHub CI integration setup wizard
- **Web**: Provider brand icons in CI pipeline views
- **Web**: Configurable release button in project settings
- **API**: Work status counts endpoint for section aggregation
- **API**: Multi-provider CI support (Forgejo + GitHub Actions)
- **Orchestra**: Configurable per-project AI providers (Anthropic, OpenAI, Ollama, Copilot)
- **Orchestra**: Codebase knowledge indexer with scheduled re-indexing

### Changed
- **Orchestra**: Planning routed through chat handler with per-project model selection
- **CI/CD**: Release script pushes to all remotes and merges back into source branch

### Fixed
- **Web**: State dropdown clipping near viewport bottom
- **Web**: Security vulnerabilities in undici dependencies
- **Web**: Various form accessibility improvements

### Removed
- **Web**: Inline AI planning and manual ready/processing status transitions
- **API**: Dedicated work planning endpoint and WebSocket plan protocol

---

## v20260315 (2026-03-15)

### Added
- **API**: CI data model (integrations, runs, jobs, steps) with Forgejo webhook ingestion
- **API**: Provider configs with encrypted API key storage
- **API**: Composite task scoring (age, priority, dependency-graph, goal-alignment)
- **API**: AI planning endpoint for work items
- **Orchestra**: Multi-provider step execution (Anthropic, OpenAI, Ollama, Copilot)
- **Orchestra**: Codebase analyzer with API surface mapper and module summarizer
- **Web**: CI pipelines page with status filters and auto-polling
- **Web**: Pipeline run detail with job/step drilldown
- **Web**: Forgejo CI onboarding wizard
- **Web**: Provider config management in Settings
- **Web**: Plan Tasks button with preview dialog
- **Web**: Execute button on work items
- **Web**: Combined multi-project token usage chart

### Changed
- **API**: Renamed "Goals" to "Work" across all endpoints and schema
- **Web**: Renamed "Goals" to "Work" across the entire UI
- **Web**: Streamlined Work page layout
- **API**: Observation promotion now creates work item + task

### Fixed
- **API**: Authorization gaps in provider config, agents, and roles routes
- **Orchestra**: Git provisioning in monorepo setups
- **Web**: Mobile horizontal scrolling
- **Web**: Git push errors displaying as `[object Object]`

### Removed
- **Web**: Standalone new-task and link-tasks buttons
- **Web**: Target date field from work items
- **Web**: Standalone new-task and link-tasks buttons
- **Web**: Target date field from work items

---

## v20260314 (2026-03-14)

### Added
- **API**: Token usage time series in project metrics
- **API**: Task reordering within work items (drag-and-drop)
- **API**: Work item activation endpoint with ready/processing lifecycle
- **API**: Related items endpoint for contextual task linking
- **API**: Event-observation rules with automatic trigger engine
- **API**: Task subtrees with parent-child relationships
- **API**: File scope tracking for branch overlap detection
- **Orchestra**: Per-project config via `.diraigent/` directory
- **Orchestra**: Configurable release strategy (templates or custom script)
- **Orchestra**: Structured event emission for git operations
- **Web**: Token usage chart on dashboard with logarithmic scale
- **Web**: Collapsible and full-screen chat panel
- **Web**: Task hierarchy views
- **Web**: Chat model displayed in AI assistant header
- **CI/CD**: Automated changelog generation via Claude in release script

### Changed
- **API**: Replaced numeric priority with boolean urgent flag
- **API**: Tasks accessed through work items (standalone tasks page removed)
- **Web**: Chat panel converted to inline collapsible panel
- **Web**: Decisions moved to review queue tab
- **CI/CD**: Per-app release workflows with change detection

### Fixed
- **API**: Tasks transition to human_review on merge failure
- **Web**: Drag-and-drop snapping issues

### Removed
- **API**: Plan entity (task ordering moved to position column)

---

## v20260313 (2026-03-13)

### Added
- **Orchestra**: Goal-based feature branch git strategy
- **Orchestra**: Subtasks inherit goal associations automatically
- **Web**: Blocker surfacing in review queue with badge and details
- **Web**: Merge conflict resolution UI
- **Web**: Feature branch option in playbook builder
- **Web**: Work item drag-and-drop reordering
- **CI/CD**: GitHub Actions release workflow

---

## v0.2.0 (2026-03-12)

### Added
- **API**: Goals as first-class containers with hierarchy, stats, and comments
- **API**: Reusable step template library with playbook integration
- **API**: Playbook versioning with copy-on-write defaults
- **API**: Merge conflict detection and resolution
- **API**: SSE push for real-time agent status
- **API**: Configurable project settings (retention, logging, auto-push)
- **Orchestra**: Auto-push after merge, retry logic, loop detection
- **Web**: Catppuccin theming (4 flavors, 14 accent colors)
- **Web**: Per-tenant settings for appearance and encryption
- **Web**: Goal management with inline editing and statistics
- **Web**: Task detail inline editing with lifecycle controls
- **Web**: Step template library in playbook builder
- **Web**: Scratchpad with "Promote to Task" action

### Changed
- **Web**: Logs moved under integrations
- **Web**: Mobile responsiveness improvements

### Fixed
- **API**: Playbook step bounds validation
- **API**: Authorization bypass via empty agent_id

---

## v0.1.0 (2026-03-03)

Initial release.

### Added
- **API**: Dual database backend (PostgreSQL + SQLite)
- **API**: Task state machine with playbook-driven pipelines
- **API**: Project hierarchy with role-based access control
- **API**: Knowledge base, decision log, observations, and goals
- **API**: Webhook delivery with HMAC-SHA256 and retry
- **API**: NATS JetStream event bus
- **API**: JWT JWKS auth, rate limiting, health probes, OpenTelemetry
- **Orchestra**: Polls for ready tasks and spawns Claude Code workers
- **Orchestra**: Isolated git worktree per task with auto-branching
- **Orchestra**: Per-step config (model, budget, tools, MCP servers)
- **Web**: Angular SPA with Tailwind and Catppuccin theme
- **Web**: Full project management UI (tasks, goals, knowledge, playbooks)
- **Web**: Agent monitoring, audit log, chat, OAuth2/OIDC, i18n
- **TUI**: Ratatui terminal interface (experimental)
