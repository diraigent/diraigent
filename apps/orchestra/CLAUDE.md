# Claude Code Agent Instructions

You are an AI agent registered with the Diraigent API. You pick up tasks, do real work, and report back.

## Identity

Your agent config is in `apps/orchestra/.env`:
- `AGENT_ID` — your registered agent ID
- `PROJECT_ID` — default project to work on
- `DIRAIGENT_API_URL` — API base URL

## CLI Tool

All API interactions go through `agent-cli`:

```bash
agent-cli ready <project_id>           # list tasks ready for work
agent-cli task <task_id>                # get task details
agent-cli context <project_id>          # full project context
agent-cli claim <task_id>               # claim a task
agent-cli transition <task_id> <state>  # move task state
agent-cli progress <task_id> "msg"      # post progress update
agent-cli artifact <task_id> "output"   # post artifact (code/output)
agent-cli blocker <task_id> "msg"       # post blocker
agent-cli comment <task_id> "msg"       # post discussion comment
agent-cli create <project_id> '<json>'  # create a new task (decompose or dream)
agent-cli depend <task_id> <dep_id>     # add a dependency between tasks
agent-cli observation <project_id> '<json>'  # file observation (insight/risk/smell/improvement)
agent-cli knowledge <project_id> '<json>'    # contribute knowledge (pattern/convention/etc.)
agent-cli decision <project_id> '<json>'     # propose decision (with rationale/alternatives)
agent-cli heartbeat                     # keep-alive
agent-cli setup                         # interactive setup wizard
```

## Workflow

When asked to "pick up a task" or "work on a task":

1. **Load config**: Read `apps/orchestra/.env` for AGENT_ID and PROJECT_ID
2. **Find work**: `agent-cli ready $PROJECT_ID` — pick the highest priority task
3. **Read details**: `agent-cli task $TASK_ID` — understand spec, files, test_cmd, acceptance
4. **Claim**: `agent-cli claim $TASK_ID` (sets state to the current playbook step name)
5. **Do the work**: Write code, create files, run commands as specified in the task context
6. **Report progress**: `agent-cli progress $TASK_ID "description of what was done"`
7. **Test**: Run the `test_cmd` from the task context
8. **Post artifacts**: `agent-cli artifact $TASK_ID "test output or code snippet"`
9. **Verify acceptance**: Check each acceptance criterion is met
10. **File observations**: If you encounter out-of-scope findings (architectural insights, code smells, risks, or improvement ideas), file them as observations: `agent-cli observation $PROJECT_ID '{"kind":"<insight|risk|smell|improvement>","title":"...","description":"...","severity":"<info|low|medium|high>"}'`
11. **Complete step**: `agent-cli transition $TASK_ID done`

## Task Context Fields

Tasks include structured context:
- `spec` — what to build/do
- `files` — which files to create/modify
- `test_cmd` — how to verify the work
- `acceptance_criteria` — conditions that must be met
- `notes` — additional guidance

## Workspace Convention

Each project gets its own directory: `apps/orchestra/{project-slug}/`

For example, a project with slug `hello-world` → work in `apps/orchestra/hello-world/`.

## Commit Message Convention

All commit messages MUST end with `agent(<short_task_id>)` where `<short_task_id>` is the first 12 characters of your task ID. This suffix is required for the revert system to identify task commits.

Example: if your task ID is `7956d757-cfda-4b2e-9a1f-...`, your commits should look like:
```
fix button styling on dashboard agent(7956d757-cfd)
add unit tests for auth service agent(7956d757-cfd)
```

Never omit this suffix. It applies to every commit you make, not just the final one.

## Safe Editing Rules — READ BEFORE TOUCHING ANY FILE

These rules prevent collateral damage. Violations cause regressions that waste multiple review cycles.

1. **Only modify files listed in `task.context.files`** (plus new test files for that same area).
   If a file is not in the task spec, do NOT touch it — even if you see an improvement opportunity.
2. **NEVER use the `Write` tool on a file that already exists.**
   `Write` replaces the ENTIRE file — you will silently delete every line not in your new content,
   including i18n keys, test cases, other functions, and unrelated features.
   **Always use `Edit` for existing files.** Use `Write` only to create brand-new files.
3. **Read the full file before editing it.**
   Use `Read` on any file you plan to change. You must see all existing sections before writing,
   or you will produce an `Edit` that conflicts with content you didn't know was there.
4. **Never remove existing code you weren't asked to remove.**
   Do not delete functions, i18n keys, imports, tests, or features that are unrelated to the task.
   Particularly at risk: `en.json`/`de.json` (i18n), `*.spec.ts` (tests), large Angular components.
5. **Before completing, sanity-check your diff:**
   ```bash
   git diff --stat main...HEAD
   ```
   The three-dot syntax (`main...HEAD`) is critical — it shows only YOUR changes since the
   branch diverged from main. Plain `git diff main` includes changes merged to main by other
   concurrent tasks, which produces false positives.
   If you see deletions in files not listed in the task spec, restore them:
   ```bash
   git checkout main -- <file>
   ```
6. **Never use `git add -A` or `git add .`** — stage only the files you intentionally changed.

## If Blocked

- Post a blocker: `agent-cli blocker $TASK_ID "description of what's blocking"`
- Release the task back: `agent-cli transition $TASK_ID ready`
- Move on to the next ready task

## State Machine Reference

```
backlog → ready → <step_name> → ready (next step) or done (final)
                              ↘ cancelled
done → human_review → done | ready | backlog
```

Lifecycle states: `backlog`, `ready`, `done`, `cancelled`, `human_review`
Step states: playbook step names (e.g. `implement`, `review`, `dream`) or `working` for tasks without a playbook.

Claiming a task sets its state to the current playbook step name. Completing a step
transitions to `done` — but the API intercepts non-final steps and auto-advances to
`ready` with the next `playbook_step`. `done` is only reached on the final step.
`human_review` is an optional post-done state for human testing. From `human_review`, tasks can be approved (→ done), sent for rework (→ ready), or reopened (→ backlog).

## Playbook Step JSON Reference

Each step in a playbook's `steps` array is a JSON object. All fields except `name` are optional.

### Step Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Step name (e.g. `"implement"`, `"review"`, `"dream"`). Shown in UI and used as task state when claimed. |
| `description` | string | Prompt template for the agent. Supports `{{variable}}` placeholders (see below). This is the primary way to tell the agent what to do. |
| `on_complete` | string | Unused by orchestra — UI hint for what happens after completion. |
| `retriable` | bool | If `true`, this step is a regression target — when a later step is rejected, the pipeline regresses to this step. Default: inferred from name (implement-like → true, review/dream → false). |
| `max_cycles` | number | Maximum failed cycles before loop detection cancels the task. Overrides the project-level `max_implement_cycles` setting for this step. `0` disables loop detection. |
| `model` | string | Claude model to use (e.g. `"sonnet"`, `"opus"`). Overrides the task-level model. |
| `budget` | number | Max dollar budget for this step (e.g. `5.0`). Default depends on step type. |
| `allowed_tools` | string | Tool preset: `"full"` (all tools), `"readonly"` (no writes). Default depends on step type. |
| `context_level` | string | How much project context to include: `"full"`, `"minimal"`, `"dream"`. Default: inferred from step name. |
| `mcp_servers` | object | MCP server config passed to Claude Code via `--mcp-config`. |
| `agents` | object | Custom sub-agent definitions passed via `--agents`. |
| `agent` | string | Specific agent to activate via `--agent <name>`. |
| `settings` | object | Additional Claude Code settings (skills, etc.) passed via `--settings`. |
| `env` | object | Extra environment variables (string→string) exported before running the agent. |
| `vars` | object | Custom template variables (string→string) for `{{placeholder}}` substitution in `description`. |

### Template Variables

The `description` field supports `{{variable}}` placeholders that are replaced at runtime.

**Built-in variables** (always available):

| Variable | Description |
|----------|-------------|
| `{{agent_cli}}` | Path to the agent-cli binary |
| `{{task_id}}` | Full task UUID |
| `{{project_id}}` | Project UUID |
| `{{short_id}}` | Shortened task ID (e.g. `aaaaaaaa-bbb`) |
| `{{branch}}` | Task branch name (e.g. `agent/task-aaaaaaaa-bbb`) |
| `{{repo_root}}` | Absolute path to the git repository root |
| `{{api_base}}` | Diraigent API base URL |
| `{{auth_header}}` | Authorization header value for API calls |
| `{{agent_id}}` | Current agent's UUID |
| `{{playbook_id}}` | Current playbook's UUID |
| `{{review_feedback}}` | Review feedback from the previous cycle (empty if none) |

**Project variables** — any string field from the project record or its `metadata` JSONB:

| Variable | Description |
|----------|-------------|
| `{{project.<key>}}` | Top-level project field (e.g. `default_branch`, `slug`, `name`) or `metadata.<key>` |

Top-level fields take precedence over metadata fields with the same name.

Examples: `{{project.default_branch}}`, `{{project.slug}}`, `{{project.branch}}`, `{{project.slack_channel}}`

Top-level fields are set in the project record; metadata fields are set in project settings or via the API.

**Custom step variables** — defined in the step's `vars` object:

```json
{
  "name": "implement",
  "description": "Run {{lint_cmd}} before committing.",
  "vars": {
    "lint_cmd": "cargo clippy --all-targets"
  }
}
```

**Substitution order**: built-ins → project metadata → step vars. Step vars can override project metadata if they use the same key name.
