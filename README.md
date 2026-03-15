# Diraigent

**Your codebase deserves a system, not a suggestion.**

A self-hosted software factory — define goals, agents decompose and execute them through enforced pipelines, with humans in the loop where it matters.

![Diraigent demo](apps/landing/public/assets/images/demo.gif)

## Why Diraigent?

Most AI coding tools fall into one of three traps:

- **Black-box SaaS** — cloud agents you can't inspect, audit, or self-host. Your code leaves your network.
- **Single-agent copilots** — great for autocomplete, but they don't orchestrate work, manage releases, or enforce quality gates.
- **Unstructured swarms** — agents without enforced workflows, no state machines, no audit trail.

Diraigent is none of these. It's a structured, self-hosted platform where:

- **Playbook pipelines enforce every step** — multi-step workflows with a validated state machine. Agents can't skip steps, improvise, or bypass quality gates.
- **Goals decompose into parallel tasks** — describe what you want at any level, Diraigent breaks it into concrete tasks and dispatches agents in parallel.
- **Humans decide what matters** — merge conflicts, ambiguous requirements, and quality gate failures surface in a review queue. Agents handle the routine; you handle the judgment calls.
- **Your project gets smarter over time** — knowledge entries, architectural decisions, and observations accumulate as agents work. The next task starts with everything the last one learned.
- **Different agents, different permissions** — define roles with scoped authority. One agent writes code, another reviews, another handles releases.
- **Releases are a first-class concept** — built-in release workflows with configurable merge strategies, branch management, and release tagging.

## How Diraigent Compares

| Capability | IDE Copilots | SaaS Agents | Open-Source Agents | Agent Orchestrators | **Diraigent** |
|---|---|---|---|---|---|
| Multi-agent parallel execution | — | ✓ | — | ✓ | **✓** |
| Enforced pipeline state machine | — | — | — | — | **✓** |
| Goal-to-task decomposition | — | ~ | — | — | **✓** |
| Persistent project knowledge | — | — | — | — | **✓** |
| Human-in-the-loop review queue | — | — | — | — | **✓** |
| Role-based agent authority | — | — | — | — | **✓** |
| Release management | — | ~ | — | — | **✓** |
| Self-hosted / data sovereignty | — | — | ✓ | ✓ | **✓** |
| Full audit trail | — | — | ~ | ~ | **✓** |

*Categories represent tool archetypes, not specific products.*

## Quickstart

Prerequisites: Docker, Docker Compose, and [Claude Code](https://docs.anthropic.com/en/docs/claude-code).

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

1. **Create a project** — point it at a git repo with a main branch
2. **Chat with the assistant** — verify Claude answers
3. **Clone a playbook** — pick one of the seeded defaults and clone it into your project
4. **Create a task** — attach the playbook, fill in spec and acceptance criteria
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
