# Diraigent

**The conductor for your software orchestra.**

A self-hosted software factory — define what you want built, agents handle the rest. Structured, auditable pipelines with full control over every step.

## Why Diraigent?

Most AI coding tools are either unstructured ("just let the AI go") or black-box SaaS you can't inspect. Diraigent gives you:

- **Control** — runs on your infra, your repos, your rules
- **Structure** — playbooks define repeatable multi-step workflows with a validated state machine
- **Auditability** — full trail of what every agent did, why, and what it produced

## Quickstart

Prerequisites: 

- Docker and Docker Compose.
- Claude Code

```bash
curl -LO https://github.com/diraigent/diraigent/blob/main/startup/docker-compose.yml
curl -LO https://github.com/diraigent/diraigent/blob/main/startup/start.sh
curl -LO https://github.com/diraigent/diraigent/blob/main/startup/.env.example
cp .env.example .env    # edit .env for your setup
chmod +x start.sh
./start.sh              # registers agent, seeds playbooks, starts everything
```

Images are published on Docker Hub: [`diraigent/api`](https://hub.docker.com/r/diraigent/api), [`diraigent/web`](https://hub.docker.com/r/diraigent/web), [`diraigent/orchestra`](https://hub.docker.com/r/diraigent/orchestra).

### First steps after startup

1. **Create a project** — via the dashboard or `POST /v1/projects`. Point it at a git repo with a main branch.
2. **Clone a playbook** — pick one of the seeded defaults and clone it into your project
3. **Create a task** — attach your playbook, fill in `spec` and `acceptance_criteria`
4. **Register an agent** — `POST /v1/agents`, then copy the returned UUID into `.env` as `AGENT_ID`
5. **Start the orchestra** — `docker compose --profile agent up -d` — it claims the task and begins working

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
| **Orchestra** | Polls API for ready tasks, spawns Claude Code workers in isolated git worktrees, auto-advances playbook pipelines. |
| **Web** | Angular 21 + Tailwind CSS 4 + Catppuccin themes. Full project management dashboard. |
| **TUI** | Ratatui terminal interface (experimental). |

## Core Concepts

### Tasks and the State Machine

Tasks advance through playbook steps automatically. Each step is a full claim → work → done cycle.

```
backlog → ready → <step_name> → done
                             ↘ cancelled
done → ready (pipeline advance to next step)
done → human_review → done | ready | backlog
```

Step names come from the task's playbook (e.g. `implement`, `review`, `dream`). Tasks carry structured context: `spec`, `files`, `test_cmd`, `acceptance_criteria`, `notes`. Transitions are validated — agents can't skip steps.

### Playbooks

Reusable multi-step workflows attached to tasks. The orchestra auto-advances tasks through pipeline steps. Each step can configure: model, budget, tool preset (`full`/`readonly`), MCP servers, sub-agents, and environment variables.

Playbooks use a `git_strategy` metadata field (e.g. `merge_to_default`) to control how completed work is integrated.

### Projects, Roles, and Knowledge

Projects nest hierarchically — agents at a parent level inherit authority over all children. Agents are assigned to projects through roles, each granting a combination of six authorities: `execute`, `delegate`, `review`, `create`, `decide`, `manage`.

The platform also tracks structured knowledge (architecture docs, conventions, patterns), ADR-style decisions, observations (things agents notice that may become tasks), integrations (external tools with per-agent access control), and events (CI results, deploys, errors).

## Configuration

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

### Running with PostgreSQL

```bash
DATABASE_URL=postgres://diraigent:diraigent@localhost:5433/diraigent cargo run -p diraigent-api
```

## License

SSPL. See [LICENSE](LICENSE) for terms.
