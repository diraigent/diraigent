# Changelog

## v20260317-1857 (2026-03-17)

### Fixed
- **CI/CD**: Fix container image signing in release pipelines

---

## v20260317-1841 (2026-03-17)

### Added
- **Web**: Multi-step agent onboarding wizard with provider setup
- **Web**: Sidebar prompt to create first agent for new users
- **API**: Auto-unlock tenant encryption using stored wrapped keys

### Changed
- **API**: Agent registration now auto-assigns a default role
- **Web**: Agent creation flow split into details, provider config, and credentials steps

### Removed
- **API**: Unused subtask and work-task count endpoints
- **Orchestra**: Dead code cleanup (unused polling, worktree cleanup, context helpers)

---

## v20250317-1345 (2026-03-17)

### Added
- **API**: Multi-provider support (OpenAI, Ollama, Anthropic) with encrypted config storage
- **API**: Forgejo CI integration with webhook validation and run ingestion
- **API**: GitHub CI integration with webhooks and sync
- **API**: AI-powered work item planning with auto-generated success criteria
- **API**: Composite task scoring (age, priority, goal-alignment, dependency-graph)
- **API**: User registration and account deletion endpoints
- **Orchestra**: Configurable per-project AI providers with step executor routing
- **Orchestra**: Scheduled re-indexing via cron and git hooks
- **Analyzer**: Static analyzer with dependency graph, API surface mapper, and module summarizer
- **Web**: CI pipelines page with run detail drilldown
- **Web**: Forgejo and GitHub CI onboarding wizards
- **Web**: Provider configuration UI in project settings
- **Web**: AI planning dialog for work items with task preview
- **Web**: Active Work dashboard section with cross-project items
- **CI/CD**: Container and binary signing with cosign and GPG

### Changed
- **Web**: Renamed Goals to Work across entire UI
- **TUI**: Renamed Goals to Work; renamed Description to Spec
- **Orchestra**: Switched to model-agnostic architecture with claude-code CLI
- **Web**: Combined token usage into single multi-project chart

### Fixed
- **API**: Encrypted integration tokens and fixed authorization gaps
- **Web**: Mobile horizontal scrolling, dropdown clipping, and various UI fixes
- **API**: Clippy lint fixes for let-chain patterns and unused imports

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
