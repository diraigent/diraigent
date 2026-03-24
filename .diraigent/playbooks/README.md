# Repo-Based Playbooks

Playbooks define the step-by-step pipeline that tasks follow in Diraigent. Instead of
managing playbooks exclusively through the API, you can store them as YAML files in your
repository under `.diraigent/playbooks/`.

## Directory Layout

```
.diraigent/
  playbooks/
    README.md            # this file
    standard.yaml        # implement -> review
    dreamer.yaml         # implement -> review -> dream
    my-custom.yaml       # any custom playbook
```

The filename (without `.yaml`) becomes the playbook identifier for repo-based references.
For example, `standard.yaml` is referenced as `standard`.

## File Format

Each `.yaml` file defines one playbook. The structure mirrors the API Playbook model:

```yaml
title: My Playbook
trigger_description: "implement -> review"
initial_state: ready        # "ready" or "backlog"
tags: [default]
metadata:
  git_strategy: merge_to_default
steps:
  - name: implement
    description: |
      Instructions for this step...
    budget: 12.0
    allowed_tools: full
    context_level: full
    on_complete: next
  - name: review
    description: |
      Review instructions...
    model: claude-sonnet-4-6
    budget: 5.0
    allowed_tools: readonly
    context_level: minimal
    on_complete: next
```

## Playbook Fields

| Field                  | Type     | Required | Default   | Description                                                    |
|------------------------|----------|----------|-----------|----------------------------------------------------------------|
| `title`                | string   | yes      | —         | Human-readable name for the playbook                           |
| `trigger_description`  | string   | no       | —         | Summary of the pipeline flow (e.g. "implement -> review")      |
| `initial_state`        | string   | no       | `ready`   | Starting state for tasks: `ready` or `backlog`                 |
| `tags`                 | string[] | no       | `[]`      | Tags for categorization and filtering                          |
| `metadata`             | object   | no       | `{}`      | Arbitrary metadata; commonly includes `git_strategy`           |
| `steps`                | array    | yes      | —         | Ordered list of pipeline steps (see below)                     |

### Metadata

The `metadata` object supports these conventional keys:

| Key                | Values                          | Description                                           |
|--------------------|---------------------------------|-------------------------------------------------------|
| `git_strategy`     | `merge_to_default`, `pr`, `none`| How completed work is merged back                     |
| `start_in_backlog` | `true` / `false`                | Whether tasks start in backlog (used with `initial_state: backlog`) |

## Step Fields

Each entry in the `steps` array is an object. Only `name` is required; all other fields
have sensible defaults inferred by the orchestra engine.

| Field              | Type     | Default              | Description                                                           |
|--------------------|----------|----------------------|-----------------------------------------------------------------------|
| `name`             | string   | *(required)*         | Step name (e.g. `implement`, `review`, `dream`). Used as task state when claimed. |
| `description`      | string   | —                    | Prompt/instructions for the agent. Supports `{{variable}}` placeholders (see below). |
| `budget`           | number   | varies by step type  | Max dollar budget for this step (e.g. `12.0`).                        |
| `model`            | string   | inherited            | Claude model override (e.g. `claude-sonnet-4-6`, `claude-opus-4-6`).  |
| `allowed_tools`    | string   | inferred             | Tool access: `full` (all tools) or `readonly` (no writes).           |
| `context_level`    | string   | inferred             | How much project context to include: `full`, `minimal`, or `dream`.  |
| `retriable`        | boolean  | inferred from name   | If `true`, regressions target this step. Implement-like steps default to `true`. |
| `max_cycles`       | number   | project default      | Max failed cycles before loop detection cancels the task. `0` disables. |
| `on_complete`      | string   | `next`               | What happens after completion: `next` (advance to next step) or `done` (finish). |
| `env`              | object   | `{}`                 | Extra environment variables (string -> string) exported before running. |
| `vars`             | object   | `{}`                 | Custom template variables for `{{placeholder}}` substitution in `description`. |
| `provider`         | string   | `anthropic`          | AI provider: `anthropic`, `openai`, `ollama`.                        |
| `base_url`         | string   | —                    | Override the default API endpoint for the chosen provider.           |
| `mcp_servers`      | object   | —                    | MCP server config passed to Claude Code via `--mcp-config`.         |
| `agents`           | object   | —                    | Custom sub-agent definitions passed via `--agents`.                  |
| `agent`            | string   | —                    | Specific agent to activate via `--agent <name>`.                     |
| `settings`         | object   | —                    | Additional Claude Code settings (skills, etc.) passed via `--settings`. |

## Template Variables

The `description` field supports `{{variable}}` placeholders that are replaced at runtime.

### Built-in Variables

These are always available in every step:

| Variable             | Description                                             |
|----------------------|---------------------------------------------------------|
| `{{agent_cli}}`      | Path to the agent-cli binary                            |
| `{{task_id}}`        | Full task UUID                                          |
| `{{project_id}}`     | Project UUID                                            |
| `{{short_id}}`       | Shortened task ID (e.g. `aaaaaaaa-bbb`)                 |
| `{{branch}}`         | Task branch name (e.g. `agent/task-aaaaaaaa-bbb`)       |
| `{{repo_root}}`      | Absolute path to the git repository root                |
| `{{api_base}}`       | Diraigent API base URL                                  |
| `{{auth_header}}`    | Authorization header value for API calls                |
| `{{agent_id}}`       | Current agent's UUID                                    |
| `{{playbook_id}}`    | Current playbook's UUID                                 |
| `{{review_feedback}}`| Review feedback from the previous cycle (empty if none) |

### Project Variables

Any string field from the project record or its `metadata` JSONB can be referenced:

| Variable                       | Description                                      |
|--------------------------------|--------------------------------------------------|
| `{{project.default_branch}}`   | The project's default branch (e.g. `main`, `dev`)|
| `{{project.slug}}`             | Project slug                                     |
| `{{project.name}}`             | Project name                                     |
| `{{project.<key>}}`            | Any top-level or metadata field                  |

Top-level project fields take precedence over metadata fields with the same name.

### Custom Step Variables

Define custom variables in a step's `vars` object:

```yaml
steps:
  - name: implement
    description: |
      Run {{lint_cmd}} before committing.
    vars:
      lint_cmd: "cargo clippy --all-targets"
```

**Substitution order**: built-ins -> project metadata -> step vars. Step vars can override
project metadata if they use the same key name.

## Relationship to API Playbooks

- **API playbooks** are stored in the database, managed via REST endpoints, and
  associated with tenants. They are the canonical runtime representation.
- **Repo playbooks** are YAML files checked into the repository. Orchestra discovers
  them at startup or when processing a project, and can use them as templates or
  overrides for API playbooks.

When orchestra discovers a `.diraigent/playbooks/` directory in a project repo, it:
1. Scans for `*.yaml` files
2. Parses each into the internal playbook representation
3. Makes them available for task assignment alongside API-managed playbooks

The filename stem (e.g. `standard` from `standard.yaml`) serves as the repo-local
playbook identifier.

## Conventions

- Use lowercase kebab-case for filenames: `my-playbook.yaml`
- Keep step names short and descriptive: `implement`, `review`, `dream`, `deploy`
- Set `on_complete: done` only on the **last** step; use `next` for all others
- Use `readonly` allowed_tools for review/dream steps that should not modify code
- Set `context_level: minimal` for review steps (they only need the diff)
- Set `context_level: dream` for dream steps (creative exploration)
- Include `git_strategy: merge_to_default` in metadata for standard workflows

## Examples

See the included example playbooks:
- [`standard.yaml`](./standard.yaml) — Standard two-step lifecycle (implement -> review)
- [`dreamer.yaml`](./dreamer.yaml) — Three-step lifecycle with creative exploration (implement -> review -> dream)
