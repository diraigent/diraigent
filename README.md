# Diraigent

**A self-hosted platform for coordinating AI-assisted software development.**

Diraigent brings projects, tasks, project knowledge, chat and human review into one shared workspace. Orchestra workers run coding agents in isolated Git worktrees and manage task execution and Git delivery. OpenCode is the default execution provider; Codex and Claude Code are optional alternatives.

The API is the authenticated control plane. With full Orchestra storage enabled, durable workers also hold detailed project content, repositories and execution output, and perform content search. Web and iOS clients access that content through the API under the same project permissions.

![Diraigent demo](apps/web/public/landing/demo.gif)

## What Diraigent provides

- **Shared workspaces** — people collaborate on projects with viewer, editor or manager access. Read-only demo accounts use these permissions.
- **Task execution and Git delivery** — Orchestra claims ready tasks, runs the selected provider in isolated worktrees, and follows the project's merge or branch policy.
- **Human review** — tasks can enter `human_review` when intervention is needed. Repository instructions and skills guide agents; configurable playbook pipelines have been retired.
- **Persistent project context** — knowledge, decisions, observations, comments and evidence stay associated with the project across tasks.
- **Worker-owned storage** — an assigned Orchestra can store detailed content and run search while PostgreSQL retains access control, task state and routing references.
- **Tools and operations** — MCP integration, release support, audit history and optional telemetry support the development workflow.

Self-hosting controls where Diraigent stores its data. The chosen AI provider and MCP tools determine where prompts and tool data are processed.

## Quickstart

Prerequisites: Git, Docker with Compose, Bash, curl and Python 3 on Linux or macOS. OpenCode is the default; Claude Code and Codex are optional. See [onboarding](docs/onboarding.md) for authentication and persistent worker storage.

```bash
git clone https://github.com/diraigent/diraigent.git
cd diraigent/startup
cp .env.example .env
chmod +x start.sh
```

Edit `.env` before starting:

```bash
# Choose a model your provider account can use.
OPENCODE_MODEL=openai/your-model-id
OPENAI_API_KEY=your-provider-api-key
```

For the first run, build the images with `./start.sh --build`.
Alternatively, run `./start.sh --build --login` for interactive OpenCode authentication.
Run `./start.sh --models` to find model IDs and set `OPENCODE_MODEL`.

`start.sh` checks the provider connection, registers an agent, privately saves its
API key, and starts the containers. Repositories, Orchestra content and OpenCode
accounts use persistent volumes:

```bash
./start.sh --build
```

Once running, open the dashboard at **http://localhost:4200**.

The quickstart builds local images from this checkout. Run `./start.sh --build` after updating the source; restart normally with `./start.sh`.

### First steps after startup

1. **Create a project** — in the dashboard, create a new project and point it at your git repo's default branch
2. **Chat with the assistant** — open the project chat and verify the configured provider responds
3. **Create a task** — fill in spec and acceptance criteria
4. **Make the task ready** — an authorized, connected Orchestra picks it up and starts working

### Git credentials

The orchestra pushes branches and merges results back to your remote. For this to work, git must be able to authenticate inside the container. Common options:

- **HTTPS + PAT** — mount a `.netrc` file with `machine github.com login <user> password <token>`
- **SSH** — mount your SSH key and use an `ssh://` remote URL
- **Git credential helper** — configure `GIT_ASKPASS` or a store-based helper

Without credentials, agents can still work locally but push/merge to the remote will fail.

## Architecture

```mermaid
flowchart LR
    Web[Web dashboard] --> API[API: authentication, permissions and coordination]
    iOS[iOS app] --> API
    API --> DB[(PostgreSQL: shared state and content references)]
    Orchestra[Orchestra worker] <-->|Authenticated WebSocket| API
    Orchestra --> Store[(Local durable content store)]
    Orchestra --> Repos[Repositories and task worktrees]
    Orchestra --> Agent[OpenCode / Codex / Claude Code]
```

| Component | Description |
|-----------|-------------|
| **API** | Rust/Axum REST API with PostgreSQL (sqlx), JWT/JWKS authentication, project authorization and authenticated worker communication. Relays authorized content requests to the assigned storage owner. |
| **Orchestra** | Runs on a host or in a container. Executes tasks, manages repositories and worktrees, and can own durable project content and content search. |
| **Web** | Angular 21 + Tailwind CSS 4 + Catppuccin themes. Project management dashboard. |
| **iOS** | Native client for projects, tasks and chat. Uses the same API permissions; workspace switching and access-management screens currently live in the web app. |
| **TUI** | Ratatui terminal interface (experimental). |

### Durable project storage

Full storage is opt-in per project and requires a protocol-2 Orchestra owner. Detailed content includes task specifications, comments, knowledge and embeddings, decisions, evidence, audit snapshots and webhook history. Logs, diffs, artifacts, personal chat and repository files also live on Orchestra. The API retains authentication, authorization, shared state, verified references and small operational metadata.

The owner's `DATA_DIR` must persist across restarts and upgrades. Detailed reads require that owner to be online; automatic storage failover is not implemented. Back up both PostgreSQL and the worker content database. Encrypted workspaces cannot enable this storage mode yet.

See [Orchestra project storage](docs/orchestra-project-storage.md) for migration, backup and recovery details.

## Core Concepts

### Tasks and the State Machine

Tasks use a fixed lifecycle:

```
backlog → ready → working → done
                    ↘ human_review → ready | done | backlog
                    ↘ cancelled
```

Orchestra claims ready tasks, runs the selected agent in a worktree, and integrates successfully completed work. Repository instructions and skills guide the agent. Optional review can use a separate dependent task or `human_review`; configurable stage pipelines have been retired.

Task context can include `spec`, `files`, `test_cmd`, `acceptance_criteria`, and `notes`. File paths are discovery hints unless explicitly restricted by the user. Optional `context.mode` selects a review or research task, and `context.worker` can override provider/model/tool settings. OpenCode remains the default, with Codex and Claude Code optional.

Git delivery policy lives in project `metadata.git_strategy` (`merge_to_default`, `branch_only`, or `feature_branch`), with the target from `metadata.git_target_branch` or the project's default branch. Missing Git roots retain the work for human review.

### Upgrading from playbooks

Stop all workers before upgrading the API and worker together. Migration 047 removes playbook references and step templates and holds unfinished legacy playbook tasks in `human_review`. Review those tasks and any former YAML Git overrides before releasing them to `ready`. Existing committed migrations remain unchanged; fresh databases apply the historical migrations followed by retirement.

### Projects, Roles, and Knowledge

Projects nest hierarchically — agents at a parent level inherit authority over all children. Agents are assigned to projects through roles, each granting a combination of six authorities: `execute`, `delegate`, `review`, `create`, `decide`, `manage`.

The platform also tracks structured knowledge (architecture docs, conventions, patterns), ADR-style decisions, observations (things agents notice that may become tasks), integrations (external tools with per-agent access control), and events (CI results, deploys, errors).

Human access is separate from agent roles. Workspace members receive explicit project viewer, editor or manager grants; human project grants do not inherit from parent projects. See [shared workspace access](docs/workspace-access.md) and [demo accounts](docs/demo-account.md).

## Configuration

Keep deployment manifests, credentials, host addresses, and machine-specific
settings outside version control. The tracked `startup/docker-compose.yml` is
a generic example; `startup/unraid/` and runtime `.env` files are ignored.
Ignoring a file does not remove copies already committed to Git history.

The web container requires runtime `AUTH_PROVIDER_BASE`, `AUTH_ISSUER`, and
`AUTH_CLIENT_ID` values for your identity provider. Set `AUTH_ENROLLMENT_URL`
if registration is enabled. These settings are visible to browser users;
never place a client secret or an access token in frontend configuration.

For iOS, copy `apps/ios/Diraigent/LocalConfig.example.plist` to
`apps/ios/Diraigent/Diraigent/LocalConfig.plist` and configure the API and OAuth
client. Xcode bundles this ignored file. Copy `Local.example.xcconfig` to
`Local.xcconfig` for your signing team. If your LAN requires an HTTP exception,
put it in an ignored `Info.local.plist` and set `INFOPLIST_FILE` in
`Local.xcconfig`. Without local settings, Debug uses localhost and Release
requires authentication configuration before sign-in can work.

| Variable | Required | Description |
|----------|----------|-------------|
| `DEV_USER_ID` | No | Bypass JWT auth in dev (set to a UUID) |
| `AUTH_ISSUER` | Prod | OIDC issuer URL |
| `AUTH_JWKS_URL` | Prod | JWKS endpoint for JWT validation |
| `CORS_ORIGINS` | No | Comma-separated allowed origins |
| `LOKI_URL` | No | Loki push endpoint for log shipping |
| `LOKI_ENV` | No | Environment label for Loki (default: `dev`) |
| `AGENT_ID` | Orchestra | Agent UUID (register via `POST /agents`) |
| `OPENCODE_MODEL` | Startup | Explicit `provider/model` ID for the default OpenCode worker |
| `MAX_WORKERS` | No | Concurrent task workers (default: `3`) |
| `DATA_DIR` | Orchestra | Durable local runtime and content storage directory; persist and back it up |

## API Reference

The OpenAPI spec is served at runtime: `GET /v1/openapi.json`

## Further documentation

- [OpenCode onboarding](docs/onboarding.md)
- [Orchestra project storage](docs/orchestra-project-storage.md)
- [Shared workspaces and project permissions](docs/workspace-access.md)
- [Read-only demo accounts](docs/demo-account.md)
- [MCP worker integration](docs/mcp-workers.md)
- [Public release checks](docs/public-release-readiness.md)

## Development

### Building from source

```bash
# API
cargo check -p diraigent-api
cargo test -p diraigent-api
cargo run -p diraigent-api

# Orchestra
cargo run --bin orchestra

# Web
cd apps/web
npm install
ng serve    # http://localhost:4200

# Lint
cargo fmt && cargo clippy --all --quiet
cd apps/web && npm run lint
```

For macOS SDK/linker errors during Rust builds, see
[macOS Rust build troubleshooting](docs/macos-rust-builds.md).

### Running with PostgreSQL

```bash
DATABASE_URL=postgres://diraigent:diraigent@localhost:5433/diraigent cargo run -p diraigent-api
```

## License

SSPL. See [LICENSE](LICENSE.md) for terms.
