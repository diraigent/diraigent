# Repository playbooks

Playbooks are optional policies for stages and handoffs. Use a single task without a playbook for ordinary work; use a playbook when the project needs an explicit review or research pipeline. The selected AI agent decides how to plan, edit and validate each stage.

## Definitions and defaults

Store YAML in `.diraigent/playbooks/<name>.yaml`. A task's `playbook_name` selects that definition. Use lowercase kebab-case names; an explicit `name` overrides the filename stem. JSON is also valid YAML.

The bundled catalog contains `standard-lifecycle`, `standard-backlog-start`, `dreamer` and `researcher`. Their source definitions are in `libs/common-rust/diraigent-types/resources/playbooks/`; this repository includes matching YAML files. `standard.yaml` retains the older `standard` name as a compatibility definition. Update both bundled and repository copies when maintaining these defaults, not committed migrations.

The API exposes the bundled catalog without an online worker. Reading or editing a project's custom repository files requires a connected worker supporting the playbook protocol. An older worker can execute repository YAML without upgrading, but needs the files in its own checkout. Bundled execution fallback requires a worker containing that feature. An API/web deployment does not update another machine's checkout or worker binary.

## Minimal example

```yaml
name: implementation-review
title: Implementation and review
initial_state: ready
metadata:
  git_strategy: merge
steps:
  - name: implement
    allowed_tools: full
    description: Implement the requested behavior and verify it with relevant checks.
  - name: review
    allowed_tools: readonly
    description: Review the changes and validation evidence; report findings without editing code.
```

Leave the model and provider unset to inherit the configured worker. Do not encode a fixed number of retries, mandatory decomposition, tool-brand names, or file-by-file approval requirements in a general-purpose stage description. Missing task file lists and test commands should trigger repository discovery, not an authorization blocker. Explicit task exclusions still apply.

## Schema

Required fields are `title` and a nonempty `steps` array. Each step needs a string `name`. Optional playbook fields are `name`, `trigger_description`, `initial_state` (`ready` or `backlog`), `tags`, and `metadata`.

Useful step fields:

| Field | Purpose |
|---|---|
| `description` | Stage-specific instructions; leave general editing policy in shared worker guidance. |
| `description_file` | Repository-local file containing the description, relative to the playbook directory. It takes precedence over inline text when readable. API saves use inline descriptions. |
| `context_level` | Context selection (`full`, `minimal`, `dream`); inferred when omitted. |
| `allowed_tools` | Provider-adapter preset (`full`, `readonly`, `merge`); actual enforcement depends on the adapter. |
| `retriable` | Whether rejection can regress to this step; inferred from the name. |
| `max_cycles` | Failure-cycle limit overriding the project default; `0` disables it. |
| `provider`, `model` | Optional provider/model overrides. Use IDs supported by that provider; omit for inheritance. |
| `budget`, `timeout_minutes` | Optional limits interpreted by the selected adapter/runtime. Do not assume every provider enforces a dollar budget. |
| `git_action` | Optional post-stage Git action (`none`, `merge`, `push`) performed by Orchestra. |
| `env`, `vars` | Provider environment and description substitutions. Keep secrets out of version-controlled definitions. |
| `on_complete` | Legacy UI hint; runtime progression follows the ordered steps, not this field. |

Options such as `mcp_servers`, `agents`, `agent`, `settings`, and `base_url` are provider-specific. Verify adapter support before using them; they are not a common capabilities contract.

`metadata.git_strategy` supports `merge`, `branch_only`, `feature_branch`, and `no_git`. `merge_to_default` and `branch_to_target` are compatibility values. Orchestra owns branches, merges and pushes; task workers do not perform those operations themselves.

## Description variables

Supported built-ins include `{{agent_cli}}`, `{{task_id}}`, `{{project_id}}`, `{{short_id}}`, `{{branch}}`, `{{repo_root}}`, `{{api_base}}`, `{{agent_id}}`, `{{playbook_name}}`, `{{work_id}}`, and `{{review_feedback}}`. `{{project.<key>}}` reads string project fields or metadata; `vars` defines custom substitutions. Prefer `agent-cli` for authenticated operations rather than embedding authorization headers in prompts or artifacts.

## Execution and blocked work

Orchestra resolves the playbook locally and manages stage advancement through its task source. Completing a stage is not necessarily completing the whole task. Validate the state returned by the runtime; do not assume the API can inspect YAML on a remote worker.

Review stages report findings without changing implementation files. Research/dream stages produce useful evidence or follow-ups, not a quota of new tasks. A failed check requires diagnosis before another attempt. Releasing genuinely blocked work to `ready` immediately makes it runnable again; unresolved blockers need a project-level deferral or dependency, not repeated identical retries.
