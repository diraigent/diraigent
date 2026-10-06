# Diraigent

**Your codebase deserves a system, not a suggestion.**

A self-hosted software factory — define goals, agents decompose and execute them in isolated worktrees, with humans in the loop where it matters.

![Diraigent demo](apps/landing/public/assets/images/demo.gif)

## Why Diraigent?

Most AI coding tools fall into one of three traps:

- **Black-box SaaS** — cloud agents you can't inspect, audit, or self-host. Your code leaves your network.
- **Single-agent copilots** — great for autocomplete, but they don't orchestrate work, manage releases, or enforce quality gates.
- **Unstructured swarms** — agents without enforced workflows, no state machines, no audit trail.

Diraigent is none of these. It's a structured, self-hosted platform where:

- **Agents execute tasks directly** — repository guidance and skills guide the work, with isolated worktrees and optional human review.
- **Goals decompose into parallel tasks** — describe what you want at any level, Diraigent breaks it into concrete tasks and dispatches agents in parallel.
- **Humans decide what matters** — merge conflicts, ambiguous requirements, and quality gate failures surface in a review queue. Agents handle the routine; you handle the judgment calls.
- **Your project gets smarter over time** — knowledge entries, architectural decisions, and observations accumulate as agents work. The next task starts with everything the last one learned.
- **Different agents, different permissions** — define roles with scoped authority. One agent writes code, another reviews, another handles releases.
- **Releases are a first-class concept** — built-in release workflows with configurable merge strategies, branch management, and release tagging.

## How Diraigent Compares

| Capability | IDE Copilots | SaaS Agents | Open-Source Agents | Agent Orchestrators | **Diraigent** |
|---|---|---|---|---|---|
| Multi-agent parallel execution | — | ✓ | — | ✓ | **✓** |
| Task ownership and Git delivery | — | — | — | — | **✓** |
| Goal-to-task decomposition | — | ~ | — | — | **✓** |
| Persistent project knowledge | — | — | — | — | **✓** |
| Human-in-the-loop review queue | — | — | — | — | **✓** |
| Role-based agent authority | — | — | — | — | **✓** |
| Release management | — | ~ | — | — | **✓** |
| Self-hosted / data sovereignty | — | — | ✓ | ✓ | **✓** |
| Full audit trail | — | — | ~ | ~ | **✓** |

*Categories represent tool archetypes, not specific products.*

## Quickstart

Prerequisites: Docker, Docker Compose, and model-provider credentials for OpenCode (the default agent). Claude Code and Codex are optional providers.

```bash
curl -LO https://raw.githubusercontent.com/diraigent/diraigent/main/startup/docker-compose.yml
curl -LO https://raw.githubusercontent.com/diraigent/diraigent/main/startup/start.sh
curl -LO https://raw.githubusercontent.com/diraigent/diraigent/main/startup/.env.example
cp .env.example .env
chmod +x start.sh
```

Edit `.env` before starting:

```bash
# Required — the git repo the orchestra will clone and work on
GIT_REPO_URL=https://github.com/your-org/your-repo.git
```

`start.sh` reads your Claude Code credentials from the macOS Keychain, registers an agent with the API, and brings up all containers:

```bash
./start.sh
```

Once running, open the dashboard at **http://localhost:4200**.

Images are published on Docker Hub: [`diraigent/api`](https://hub.docker.com/r/diraigent/api), [`diraigent/web`](https://hub.docker.com/r/diraigent/web), [`diraigent/orchestra`](https://hub.docker.com/r/diraigent/orchestra).

### First steps after startup

1. **Create a project** — in the dashboard, create a new project and point it at your git repo's default branch
2. **Chat with the assistant** — open the project chat and verify Claude responds
3. **Create a task** — fill in spec and acceptance criteria
5. The orchestra picks it up and starts working

### Git credentials

The orchestra pushes branches and merges results back to your remote. For this to work, git must be able to authenticate inside the container. Common options:

- **HTTPS + PAT** — mount a `.netrc` file with `machine github.com login <user> password <token>`
- **SSH** — mount your SSH key and use an `ssh://` remote URL
- **Git credential helper** — configure `GIT_ASKPASS` or a store-based helper

Without credentials, agents can still work locally but push/merge to the remote will fail.

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Web (4200) │────▶│  API (8082) │◀────│  Orchestra  │
│  Angular 21 │     │  Rust/Axum  │     │  Rust + CC  │
└─────────────┘     └──────┬──────┘     └─────────────┘
                           │
                    ┌──────┴──────┐
                    │  PostgreSQL │
                    │    (5433)   │
                    └─────────────┘
```

| Component | Description |
|-----------|-------------|
| **API** | Rust/Axum REST API. PostgreSQL backend (sqlx). JWT JWKS auth. WebSocket agent communication. |
| **Orchestra** | Polls API for ready tasks, runs OpenCode by default in isolated git worktrees, coordinates task completion and Git delivery. |
| **Web** | Angular 21 + Tailwind CSS 4 + Catppuccin themes. Full project management dashboard. |
| **TUI** | Ratatui terminal interface (experimental). |

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
| `GIT_REPO_URL` | Orchestra | Git repo URL cloned into the worker volume |
| `MAX_WORKERS` | No | Concurrent Claude Code workers (default: `3`) |

## API Reference

The OpenAPI spec is served at runtime: `GET /v1/openapi.json`

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
