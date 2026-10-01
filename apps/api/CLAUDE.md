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
- **Roles** are project-defined positions with specific authorities. Each project decides what roles it needs.
- **Membership** links agents to projects via roles. An agent can have multiple memberships across projects.
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
Projects support nesting via `parent_id`. Authority inheritance: an agent with `manage` authority on a parent project inherits that authority on all child projects.

## Task State Machine

```
backlog → ready → <step_name> → ready (next step) or done (final)
                              ↘ cancelled
```

Lifecycle states: `backlog`, `ready`, `done`, `cancelled`, `human_review`
Step states: playbook step names (e.g. `implement`, `review`, `dream`) or `working` for tasks without a playbook.

Valid transitions:
- backlog → ready, cancelled
- ready → \<step_name\>, backlog, cancelled
- \<step_name\> → done (final step only), ready (release/rejection), cancelled
- done → ready (reopen), backlog (reopen), human_review
- human_review → done, ready, backlog
- cancelled → backlog (reopen)

Orchestra resolves repository YAML and manages pipeline advancement through its
task source. The API validates requested state transitions; do not assume it can
inspect playbooks stored on another worker machine. Stage completion and terminal
task completion are distinct.

Claiming a task (`POST /tasks/:id/claim`) atomically transitions it from `ready` to the current playbook step name.

## Key Endpoints

### For Agents
- `GET /v1/{id}/tasks/ready` — tasks ready for work (all deps satisfied)
- `POST /v1/tasks/{id}/claim` — atomically claim a task
- `POST /v1/tasks/{id}/transition` — move task through states
- `POST /v1/tasks/{id}/updates` — report progress
- `POST /v1/agents/{id}/heartbeat` — keep-alive

### Roles & Membership
- `POST /v1/{id}/roles` — create role
- `GET /v1/{id}/roles` — list roles
- `GET/PUT/DELETE /v1/roles/{id}` — role CRUD
- `POST /v1/{id}/members` — add member (assign agent to role)
- `GET /v1/{id}/members` — list project members
- `GET/PUT/DELETE /v1/members/{id}` — membership CRUD
- `GET /v1/agents/{id}/memberships` — agent's project memberships

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
