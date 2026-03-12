-- Update default global playbooks with detailed step descriptions.
-- The short descriptions from 006/011 were too vague for the review step
-- to produce correct transitions. These descriptions include explicit
-- agent-cli workflow instructions and template variables.

UPDATE diraigent.playbook AS playbook
SET
    steps = updated.steps,
    trigger_description = updated.trigger_description,
    metadata = updated.metadata
FROM (
    VALUES
        (
            'Standard Lifecycle',
            'implement → review',
            '[
              {
                "name": "implement",
                "budget": 12.0,
                "allowed_tools": "full",
                "context_level": "full",
                "on_complete": "next",
                "description": "## Your Job: IMPLEMENT\n\n1. **Read the task**: Run `{{agent_cli}} task {{task_id}}` to get the spec, files, test_cmd, and acceptance criteria. Also check `## Task Discussion` above for any human instructions.\n2. **Claim the task**: Run `{{agent_cli}} claim {{task_id}}`\n3. **Assess complexity**: If this touches many files across modules, decompose:\n   - Create subtasks: `{{agent_cli}} create {{project_id}} ''<json>''`\n   - Wire dependencies: `{{agent_cli}} depend <id> <dep_id>`\n   - Transition subtasks to ready: `{{agent_cli}} transition <id> ready`\n   - Post progress and submit for review. Stop here.\n   Otherwise, continue.\n4. **Implement**: Write code as the spec requires.\n5. **Report progress**: `{{agent_cli}} progress {{task_id}} \"what was done\"`\n6. **Lint & format**: If a lint command is available, run it. Fix all issues before testing.\n7. **Test**: If a test command is specified, run it. Retry up to 3 times on failure.\n   - Still failing? `{{agent_cli}} blocker {{task_id}} \"error\"`, `{{agent_cli}} transition {{task_id}} ready`\n8. **Post artifacts**: `{{agent_cli}} artifact {{task_id}} \"test output\"`\n9. **Complete**: `{{agent_cli}} transition {{task_id}} done`\n10. **File observations** for code smells, risks, or improvements:\n    `{{agent_cli}} observation {{project_id}} ''{\"kind\":\"<insight|risk|smell|improvement>\",\"title\":\"...\",\"description\":\"...\",\"severity\":\"<info|low|medium|high>\"}''\n\n**Rules**: Do NOT run `git push`. Stay in your worktree.\n\n**Scope guardrail**: Before transitioning to done, run `git diff --stat {{project.default_branch}}...HEAD` and verify you only touched files listed in the task spec."
              },
              {
                "name": "review",
                "budget": 5.0,
                "model": "claude-sonnet-4-6",
                "allowed_tools": "readonly",
                "context_level": "minimal",
                "on_complete": "next",
                "description": "## Your Job: CODE REVIEW\n\n1. **Read the task**: Run `{{agent_cli}} task {{task_id}}` to get the spec and acceptance criteria.\n2. **Claim the task**: Run `{{agent_cli}} claim {{task_id}}`\n3. **Review the changes**: Run `git diff {{project.default_branch}}...HEAD` to see what changed.\n4. **Check quality**: Does it meet the spec? Acceptance criteria met? Any obvious bugs or security issues?\n5. **Post your review**: `{{agent_cli}} artifact {{task_id}} \"REVIEW: <findings>\"`\n6. **Decision**:\n   - APPROVED: `{{agent_cli}} transition {{task_id}} done`\n   - CHANGES NEEDED: `{{agent_cli}} blocker {{task_id}} \"what needs fixing\"`, then `{{agent_cli}} transition {{task_id}} ready`\n\n**Rules**: Do NOT modify code — only review. Be specific (file names, line numbers)."
              }
            ]'::jsonb,
            '{"git_strategy":"merge_to_default"}'::jsonb
        ),
        (
            'Standard (Backlog Start)',
            'backlog → ready → implement → review',
            '[
              {
                "name": "implement",
                "budget": 12.0,
                "allowed_tools": "full",
                "context_level": "full",
                "on_complete": "next",
                "description": "## Your Job: IMPLEMENT\n\n1. **Read the task**: Run `{{agent_cli}} task {{task_id}}` to get the spec, files, test_cmd, and acceptance criteria. Also check `## Task Discussion` above for any human instructions.\n2. **Claim the task**: Run `{{agent_cli}} claim {{task_id}}`\n3. **Assess complexity**: If this touches many files across modules, decompose:\n   - Create subtasks: `{{agent_cli}} create {{project_id}} ''<json>''`\n   - Wire dependencies: `{{agent_cli}} depend <id> <dep_id>`\n   - Transition subtasks to ready: `{{agent_cli}} transition <id> ready`\n   - Post progress and submit for review. Stop here.\n   Otherwise, continue.\n4. **Implement**: Write code as the spec requires.\n5. **Report progress**: `{{agent_cli}} progress {{task_id}} \"what was done\"`\n6. **Lint & format**: If a lint command is available, run it. Fix all issues before testing.\n7. **Test**: If a test command is specified, run it. Retry up to 3 times on failure.\n   - Still failing? `{{agent_cli}} blocker {{task_id}} \"error\"`, `{{agent_cli}} transition {{task_id}} ready`\n8. **Post artifacts**: `{{agent_cli}} artifact {{task_id}} \"test output\"`\n9. **Complete**: `{{agent_cli}} transition {{task_id}} done`\n\n**Rules**: Do NOT run `git push`. Stay in your worktree.\n\n**Scope guardrail**: Before transitioning to done, run `git diff --stat {{project.default_branch}}...HEAD` and verify you only touched files listed in the task spec."
              },
              {
                "name": "review",
                "budget": 5.0,
                "model": "claude-sonnet-4-6",
                "allowed_tools": "readonly",
                "context_level": "minimal",
                "on_complete": "next",
                "description": "## Your Job: CODE REVIEW\n\n1. **Read the task**: Run `{{agent_cli}} task {{task_id}}` to get the spec and acceptance criteria.\n2. **Claim the task**: Run `{{agent_cli}} claim {{task_id}}`\n3. **Review the changes**: Run `git diff {{project.default_branch}}...HEAD` to see what changed.\n4. **Check quality**: Does it meet the spec? Acceptance criteria met? Any obvious bugs or security issues?\n5. **Post your review**: `{{agent_cli}} artifact {{task_id}} \"REVIEW: <findings>\"`\n6. **Decision**:\n   - APPROVED: `{{agent_cli}} transition {{task_id}} done`\n   - CHANGES NEEDED: `{{agent_cli}} blocker {{task_id}} \"what needs fixing\"`, then `{{agent_cli}} transition {{task_id}} ready`\n\n**Rules**: Do NOT modify code — only review. Be specific (file names, line numbers)."
              }
            ]'::jsonb,
            '{"start_in_backlog":true,"git_strategy":"merge_to_default"}'::jsonb
        ),
        (
            'Dreamer',
            'implement → review → dream',
            '[
              {
                "name": "implement",
                "budget": 12.0,
                "allowed_tools": "full",
                "context_level": "full",
                "on_complete": "next",
                "description": "## Your Job: IMPLEMENT\n\n1. **Read the task**: Run `{{agent_cli}} task {{task_id}}` to get the spec, files, test_cmd, and acceptance criteria. Also check `## Task Discussion` above for any human instructions.\n2. **Claim the task**: Run `{{agent_cli}} claim {{task_id}}`\n3. **Assess complexity**: If this touches many files across modules, decompose:\n   - Create subtasks: `{{agent_cli}} create {{project_id}} ''<json>''`\n   - Wire dependencies: `{{agent_cli}} depend <id> <dep_id>`\n   - Transition subtasks to ready: `{{agent_cli}} transition <id> ready`\n   - Post progress and submit for review. Stop here.\n   Otherwise, continue.\n4. **Implement**: Write code as the spec requires.\n5. **Report progress**: `{{agent_cli}} progress {{task_id}} \"what was done\"`\n6. **Lint & format**: If a lint command is available, run it. Fix all issues before testing.\n7. **Test**: If a test command is specified, run it. Retry up to 3 times on failure.\n   - Still failing? `{{agent_cli}} blocker {{task_id}} \"error\"`, `{{agent_cli}} transition {{task_id}} ready`\n8. **Post artifacts**: `{{agent_cli}} artifact {{task_id}} \"test output\"`\n9. **Complete**: `{{agent_cli}} transition {{task_id}} done`\n\n**Rules**: Do NOT run `git push`. Stay in your worktree."
              },
              {
                "name": "review",
                "budget": 5.0,
                "model": "claude-sonnet-4-6",
                "allowed_tools": "readonly",
                "context_level": "minimal",
                "on_complete": "next",
                "description": "## Your Job: CODE REVIEW\n\n1. **Read the task**: Run `{{agent_cli}} task {{task_id}}` to get the spec and acceptance criteria.\n2. **Claim the task**: Run `{{agent_cli}} claim {{task_id}}`\n3. **Review the changes**: Run `git diff {{project.default_branch}}...HEAD` to see what changed.\n4. **Check quality**: Does it meet the spec? Acceptance criteria met? Any obvious bugs or security issues?\n5. **Post your review**: `{{agent_cli}} artifact {{task_id}} \"REVIEW: <findings>\"`\n6. **Decision**:\n   - APPROVED: `{{agent_cli}} transition {{task_id}} done`\n   - CHANGES NEEDED: `{{agent_cli}} blocker {{task_id}} \"what needs fixing\"`, then `{{agent_cli}} transition {{task_id}} ready`\n\n**Rules**: Do NOT modify code — only review. Be specific (file names, line numbers)."
              },
              {
                "name": "dream",
                "budget": 4.0,
                "model": "claude-sonnet-4-6",
                "allowed_tools": "readonly",
                "context_level": "dream",
                "on_complete": "done",
                "description": "## Your Job: DREAM\n\nYou just completed a task. Now step back and think about what is next.\n\n1. **Read the task**: Run `{{agent_cli}} task {{task_id}}` to understand what was just done.\n2. **Claim the task**: Run `{{agent_cli}} claim {{task_id}}`\n3. **Analyze the codebase**: Look at the files touched, the surrounding code, tests, and architecture.\n4. **Identify improvements**: Think about:\n   - Missing tests or edge cases\n   - Code that could be refactored or simplified\n   - New features that would naturally follow\n   - Technical debt worth addressing\n   - Documentation gaps\n5. **Post suggestions as new tasks** (quality over quantity — only things worth doing):\n   ```\n   {{agent_cli}} create {{project_id}} ''{\"title\": \"...\", \"kind\": \"<feature|refactor|docs|test|bug>\", \"priority\": 5, \"playbook_id\": \"{{playbook_id}}\", \"context\": {\"spec\": \"...\", \"files\": [\"...\"], \"acceptance_criteria\": [\"...\"]}}'' \n   ```\n6. **Post observations** for architectural insights or code smells:\n   `{{agent_cli}} observation {{project_id}} ''{\"kind\":\"<insight|risk|smell|improvement>\",\"title\":\"...\",\"description\":\"...\",\"severity\":\"<info|low|medium|high>\"}''\n7. **Complete**: `{{agent_cli}} transition {{task_id}} done`\n\n**Rules**: Do NOT modify code — only analyze and suggest. 2-5 suggestions is ideal."
              }
            ]'::jsonb,
            '{"git_strategy":"merge_to_default"}'::jsonb
        ),
        (
            'Researcher',
            'scope → gather → synthesize → document',
            '[
              {
                "name": "scope",
                "model": "claude-sonnet-4-6",
                "budget": 5.0,
                "allowed_tools": "readonly",
                "on_complete": "next",
                "description": "## Your Job: SCOPE THE RESEARCH\n\n1. **Read the task**: Run `{{agent_cli}} task {{task_id}}` to get the research question.\n2. **Claim the task**: Run `{{agent_cli}} claim {{task_id}}`\n3. **Read existing docs and codebase** to understand the current state.\n4. **Refine the scope**: Post the refined research question as a decision:\n   `{{agent_cli}} decision {{project_id}} ''{\"title\":\"Research scope: ...\",\"description\":\"...\",\"rationale\":\"...\",\"alternatives\":[\"...\"]}''\n5. **Report progress**: `{{agent_cli}} progress {{task_id}} \"scoped: <summary>\"`\n6. **Complete**: `{{agent_cli}} transition {{task_id}} done`"
              },
              {
                "name": "gather",
                "model": "claude-sonnet-4-6",
                "budget": 10.0,
                "allowed_tools": "full",
                "on_complete": "next",
                "description": "## Your Job: GATHER SOURCES\n\n1. **Read the task**: Run `{{agent_cli}} task {{task_id}}` to get the scoped research question.\n2. **Claim the task**: Run `{{agent_cli}} claim {{task_id}}`\n3. **Fetch and aggregate** relevant sources — code, docs, web references.\n4. **Post key findings as knowledge entries**:\n   `{{agent_cli}} knowledge {{project_id}} ''{\"title\":\"...\",\"category\":\"<pattern|convention|architecture|reference>\",\"content\":\"...\"}''`\n5. **Report progress**: `{{agent_cli}} progress {{task_id}} \"gathered N sources\"`\n6. **Complete**: `{{agent_cli}} transition {{task_id}} done`"
              },
              {
                "name": "synthesize",
                "model": "claude-opus-4-6",
                "budget": 15.0,
                "allowed_tools": "full",
                "on_complete": "next",
                "description": "## Your Job: SYNTHESIZE FINDINGS\n\n1. **Read the task**: Run `{{agent_cli}} task {{task_id}}` to review the gathered material.\n2. **Claim the task**: Run `{{agent_cli}} claim {{task_id}}`\n3. **Synthesize** findings into structured notes. Identify patterns, trade-offs, and recommendations.\n4. **Create a section with actionable tasks** that could follow from this research.\n5. **Post artifacts**: `{{agent_cli}} artifact {{task_id}} \"SYNTHESIS: <structured notes>\"`\n6. **Complete**: `{{agent_cli}} transition {{task_id}} done`"
              },
              {
                "name": "document",
                "budget": 8.0,
                "allowed_tools": "full",
                "on_complete": "done",
                "description": "## Your Job: WRITE DOCUMENTATION\n\n1. **Read the task**: Run `{{agent_cli}} task {{task_id}}` to review the synthesis.\n2. **Claim the task**: Run `{{agent_cli}} claim {{task_id}}`\n3. **Write the final documentation artifact** based on the synthesized findings.\n4. **Post artifacts**: `{{agent_cli}} artifact {{task_id}} \"<final document>\"`\n5. **Complete**: `{{agent_cli}} transition {{task_id}} done`"
              }
            ]'::jsonb,
            '{"git_strategy":"merge_to_default"}'::jsonb
        )
) AS updated(title, trigger_description, steps, metadata)
WHERE playbook.title = updated.title
  AND playbook.tenant_id IS NULL
  AND playbook.created_by = '00000000-0000-0000-0000-000000000000';

-- Also update the step templates with richer descriptions.
UPDATE diraigent.step_template SET
    description = 'Implement the feature or fix described in the task spec. Write tests where appropriate.

1. Read the task: `{{agent_cli}} task {{task_id}}`
2. Claim: `{{agent_cli}} claim {{task_id}}`
3. Implement the spec
4. Report progress: `{{agent_cli}} progress {{task_id}} "what was done"`
5. Complete: `{{agent_cli}} transition {{task_id}} done`

Rules: Do NOT run `git push`. Stay in your worktree.',
    context_level = 'full',
    on_complete = 'next'
WHERE name = 'implement' AND tenant_id IS NULL;

UPDATE diraigent.step_template SET
    description = 'Review the implementation for correctness, style, and test coverage.

1. Read the task: `{{agent_cli}} task {{task_id}}`
2. Claim: `{{agent_cli}} claim {{task_id}}`
3. Review changes: `git diff {{project.default_branch}}...HEAD`
4. APPROVED: `{{agent_cli}} transition {{task_id}} done`
5. CHANGES NEEDED: `{{agent_cli}} blocker {{task_id}} "what needs fixing"`, then `{{agent_cli}} transition {{task_id}} ready`

Rules: Do NOT modify code — only review.',
    context_level = 'minimal',
    on_complete = 'next'
WHERE name = 'review' AND tenant_id IS NULL;

UPDATE diraigent.step_template SET
    description = 'Explore the codebase around the completed work. Post new task suggestions as observations.

1. Read the task: `{{agent_cli}} task {{task_id}}`
2. Claim: `{{agent_cli}} claim {{task_id}}`
3. Analyze surrounding code, tests, architecture
4. Post suggestions: `{{agent_cli}} create {{project_id}} ''<json>''`
5. Post observations: `{{agent_cli}} observation {{project_id}} ''<json>''`
6. Complete: `{{agent_cli}} transition {{task_id}} done`

Rules: Do NOT modify code. 2-5 suggestions is ideal.',
    context_level = 'dream',
    on_complete = 'done'
WHERE name = 'dream' AND tenant_id IS NULL;
