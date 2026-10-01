# Orchestra task worker guidance

These instructions apply to OpenCode, Codex, Claude Code and other supported providers. Orchestra supplies the task identity, context, step, repository and working directory. Use the configured `agent-cli` for task operations; it handles authentication. Do not print environment files, tokens or authorization headers.

## Perform the assigned step

- Read the supplied task and discussion; fetch `agent-cli task <task_id>` when current details are needed. Do not claim a task again if Orchestra already claimed it for you. Claim it only when manually picking up a ready task.
- Work in the supplied task worktree or working directory. Do not assume a fixed checkout layout or move into a different project.
- Infer the implementation files from the requested behavior. `context.files` and `file_scope` are hints for discovery and overlap detection, not mandatory authorization lists. Honor explicit exclusions and exclusive scope instructions.
- Choose the plan and tools that fit the task. Decompose only when explicitly requested or when independent work genuinely needs separate tasks; touching several files alone is not a reason to abandon implementation.
- Preserve unrelated code and other contributors' changes. Use focused edits with whichever tools the provider supports, then inspect the diff. Do not blindly replace a file or restore it to a branch snapshot.
- Run the supplied validation when relevant. If no test command is supplied, find the repository's normal checks and choose those that verify the changed behavior. Diagnose failures before retrying; distinguish failures introduced by this task from baseline problems.
- Report meaningful progress and final evidence through `agent-cli`. Create observations or follow-up tasks when they are actionable; do not manufacture a quota of suggestions.
- Complete the assigned step with `agent-cli transition <task_id> done` only after its actual work and validation are complete. Orchestra handles playbook advancement. A review step may report findings through the API but must not edit implementation files.

## Git and data integrity

Orchestra owns worktree creation, merge and push behavior according to the project's Git strategy. Task workers commit relevant changes but do not run `git push` or merge into the target branch themselves. An explicit user request to change that behavior must be handled by the controlling session/runtime.

For commits made for a claimed Orchestra task, append `agent(<short_task_id>)`, using the first 12 characters of the task UUID. The revert system searches for this marker. This convention does not apply to ordinary human or interactive-agent commits without a claimed task.

Inspect `git status`, staged changes, unstaged changes, and the task branch diff against its actual base/target. Stage only intended files. Do not assume every project targets `main`, or that a branch diff alone includes uncommitted work.

Never edit a committed migration. Add a new migration when required. Do not commit secrets, local deployment manifests, private connection details or signing overrides. Use configured credentials through the CLI without copying them into comments, logs, prompts or task artifacts.

## Genuine blockers

A missing file list or test command is not a blocker. An explicit contradictory restriction, unavailable required credential, unresolved dependency, or unrecoverable validation failure can be.

Post a specific blocker with the evidence and the missing information or dependency. Do not mark unfinished work done. Releasing a task to `ready` makes it eligible for immediate retry; do not repeatedly release and retry the same failure without a change that can resolve it. Use the available project controls to defer unresolved work, or report that a human needs to defer it. Do not invent an unsupported `blocked` state.

## Playbooks and providers

Playbooks are optional stage policies, not scripts for the agent's internal reasoning. Definitions live in `.diraigent/playbooks/`; tasks refer to `playbook_name`. Repository YAML may override a bundled default. See `.diraigent/playbooks/README.md` for the schema and examples.

Leave `provider` and `model` unset to inherit the worker configuration. Tool presets, budget limits and provider-specific options are applied by the selected adapter; support differs between providers. Do not assume Claude flag names or a dollar budget are portable enforcement mechanisms.

Lifecycle states include `backlog`, `ready`, `done`, `cancelled` and `human_review`; active step names come from the selected playbook. Use the CLI/runtime's supported transitions and report the state returned, rather than assuming a transition completed a whole pipeline.
