# Diraigent API

AI-agent-first project management API. Built with Rust/Axum.

## Architecture

- **Port**: 8082
- **Database**: `diraigent` (PostgreSQL)
- **Auth**: JWKS JWT (same as health API)
- **Routes**: nested under `/v1`

## Key Concepts

- **Projects** group tasks. Each has a unique slug. Projects can be nested via `parent_id` (e.g. platform → API, Health, iOS).
- **Tasks** are the atomic unit of work. They have structured context (files, spec, test_cmd, acceptance criteria, notes). Tasks can be delegated between agents.
- **Agents** are AI workers that claim and execute tasks.
- **Roles** define explicit agent authorities within a workspace (tenant).
- **Membership** links agents to workspace roles. Human workspace membership and project viewer/editor/manager grants are separate.
- **Task Updates** are structured progress reports from agents or humans.
- **Dependencies** form a DAG between tasks.

### Authorities (Role-based)
- `execute` — can claim and work on tasks
- `delegate` — can assign tasks to other agents
- `review` — can approve/reject other agents' work
- `create` — can create tasks, decompose goals into tasks
- `decide` — can approve decisions, set priority, resolve observations
- `manage` — can modify roles, add/remove team members, modify project

### Project Hierarchy
Projects support nesting via `parent_id`. Agent roles apply to projects within their workspace; hierarchy does not imply additional authorities. `manage` does not imply `create` or `execute`. Human workspace owners/admins manage all workspace projects; ordinary members need explicit project grants. Workspace viewers remain read-only.

## Task State Machine

```
backlog → ready → working → human_review → done
                     ↘ cancelled
```

Lifecycle states: `backlog`, `ready`, `done`, `cancelled`, `human_review`
Task states are `backlog`, `ready`, `working`, `human_review`, `done`, and `cancelled`. Claiming atomically moves a ready task to working. Completion is terminal; no stage counters or playbook endpoints exist. Repository guidance and skills are interpreted by the worker's selected agent.

## Key Endpoints

### For Agents
- `GET /v1/{id}/tasks/ready` — tasks ready for work (all deps satisfied)
- `POST /v1/tasks/{id}/claim` — atomically claim a task
- `POST /v1/tasks/{id}/transition` — move task through states
- `POST /v1/tasks/{id}/updates` — report progress
- `POST /v1/agents/{id}/heartbeat` — keep-alive

### Roles & Membership
- `POST /v1/roles` — create role
- `GET /v1/roles` — list roles
- `GET/PUT/DELETE /v1/roles/{id}` — role CRUD
- `POST /v1/members` — add member (assign agent to role)
- `GET /v1/members` — list workspace agent memberships
- `GET/PUT/DELETE /v1/members/{id}` — membership CRUD
- `GET /v1/agents/{id}/memberships` — agent's workspace memberships

### Delegation & Hierarchy
- `POST /v1/tasks/{id}/delegate` — delegate task to another agent
- `GET /v1/{id}/children` — list sub-projects
- `GET /v1/{id}/tree` — full project tree (recursive)

### For Humans
- CRUD on projects, tasks, agents
- Task dependency management
- Task update timeline

## Source Structure

```
src/
  main.rs        — AppState, server setup, migrations
  auth.rs        — JWKS JWT auth (same as health API)
  error.rs       — AppError enum
  models.rs      — Domain structs, enums, DTOs
  repository.rs  — All database operations
  routes/
    mod.rs       — Router wiring
    projects.rs  — Project CRUD
    tasks.rs     — Task operations + state transitions
    agents.rs    — Agent registry + heartbeat
    roles.rs     — Role CRUD
    members.rs   — Membership management
```

## WebSocket Agent Communication

- Orchestra agents connect via `wss://.../v1/agents/{agent_id}/ws`
- Chat and git requests are sent over WS; responses flow back the same channel
- `ws_protocol.rs` — shared message types, `ws_registry.rs` — connection registry
- Events (audit + webhooks) are dispatched inline via `AppState::fire_event()`

## Running Locally

```bash
# Create the database
createdb -h localhost -p 5433 -U zivue diraigent

# Run
DATABASE_URL=postgres://zivue:@localhost:5433/diraigent \
PORT=8082 \
cargo run -p diraigent-api
```
