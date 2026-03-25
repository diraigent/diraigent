use crate::crypto::Dek;
use crate::engine::step_profile::StepProfile;
use crate::engine::task_source::TaskSource;
use crate::task_id::TaskId;
use std::path::Path;
use tracing::{debug, info};

/// Build the static system prompt for Claude Code's `--system-prompt` flag.
///
/// This content is stable across tasks and benefits from Anthropic's prompt
/// caching. Claude Code marks system prompt blocks with `cache_control`,
/// so identical content across invocations gets a server-side cache hit
/// (5-min TTL).
///
/// NOTE: CLAUDE.md and AGENTS.md are NOT included here — Claude Code
/// auto-loads them from the worktree directory with its own caching.
pub fn build_static_system_prompt(repo_root: &Path) -> String {
    let apps_claude_md = read_file_or_empty(&repo_root.join("apps/orchestra/CLAUDE.md"));

    format!(
        "You are an autonomous AI agent working on a specific task in a software project.\n\n\
         ## Agent Instructions\n\
         {apps_claude_md}"
    )
}

/// Build the dynamic user prompt for Claude Code's `-p` flag.
///
/// This content changes per task: identity, project context, active work,
/// workflow steps, and task discussion. Kept separate from the static
/// system prompt so that the system prompt can be cached across tasks.
///
/// Context is trimmed based on the step type to reduce input tokens:
/// - implement/rework: full context (observations, knowledge, events, playbooks)
/// - review: task-focused (no observations, no events, minimal context)
/// - merge: minimal context (just task info)
/// - dream: full context needed for analysis
///
/// Prompt structure (order matters — earlier sections get more attention):
/// 1. Identity
/// 2. Project Context
/// 3. Active Work
/// 4. Task Discussion — human comments & task spec shown BEFORE the workflow
///    so the agent reads instructions before forming an implementation plan
/// 5. Workflow (## Your Job: IMPLEMENT / REVIEW / etc.)
/// 6. General Rules
#[allow(clippy::too_many_arguments)]
pub async fn build_user_prompt(
    api: &dyn TaskSource,
    task_id: &str,
    project_id: &str,
    worktree_path: &Path,
    repo_root: &Path,
    agent_cli: &str,
    step_name: &str,
    step_json: Option<&serde_json::Value>,
    dek: Option<&Dek>,
) -> String {
    let tid = TaskId::new(task_id);

    let resolved_step = step_name.to_string();
    // Determine what context to include: step JSONB > hardcoded by step name.
    // Computed before API calls so we can conditionally skip work items in the parallel block.
    let context_level = match step_json.and_then(|s| s["context_level"].as_str()) {
        Some("full") => ContextLevel::full(),
        Some("minimal") => ContextLevel::minimal(),
        Some("dream") => ContextLevel::dream(),
        _ => ContextLevel::for_step(&resolved_step),
    };

    // Fire all independent API calls in parallel using tokio::join!.
    // Previously these ran sequentially (~200-500ms of serial latency);
    // now they overlap and complete in ~one round-trip (~30ms).
    let include_work = context_level.include_work;
    let include_related = context_level.include_knowledge;
    let include_autodocs = context_level.include_knowledge;
    let (
        project_res,
        task_res,
        raw_context,
        work_section,
        comments_res,
        updates_res,
        verifications_res,
        task_work_ids,
        related_items_res,
        autodocs_res,
    ) = tokio::join!(
        api.get_project(project_id),
        api.get_task(task_id),
        api.get_context_for_task(project_id, task_id),
        async {
            if include_work {
                build_work_section(api, project_id).await
            } else {
                String::new()
            }
        },
        api.get_task_comments(task_id),
        api.get_task_updates(task_id),
        api.get_verifications(project_id, task_id),
        api.get_task_work_items(task_id),
        async {
            if include_related {
                Some(api.get_related_items(task_id).await)
            } else {
                None
            }
        },
        async {
            if include_autodocs {
                api.list_knowledge(project_id, Some("source:codegen"), Some(100))
                    .await
                    .ok()
            } else {
                None
            }
        },
    );

    let project_json = project_res.ok();
    let task_json = task_res.ok();

    // Build related context section (task-relevant knowledge/decisions/observations).
    // Only included for full-context steps (implement/dream), not minimal (review).
    let related_context = match related_items_res {
        Some(Ok(ref related)) => {
            let section = build_related_context_section(related);
            if section.is_empty() {
                debug!("task {tid}: related items returned empty");
            }
            section
        }
        Some(Err(ref e)) => {
            debug!("task {tid}: related items fetch failed: {e}");
            String::new()
        }
        None => String::new(),
    };
    // First work item ID linked to this task (if any) — used for work item
    // inheritance when the agent creates subtasks. Empty string when the task
    // has no work item.
    let first_work_id = task_work_ids
        .unwrap_or_default()
        .into_iter()
        .next()
        .and_then(|v| v["id"].as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    let task_comments = comments_res.unwrap_or_default();
    let task_updates = updates_res.unwrap_or_default();
    let verifications = verifications_res.unwrap_or_default();

    let current_playbook_id = task_json
        .as_ref()
        .and_then(|t| t["playbook_id"].as_str())
        .unwrap_or("")
        .to_string();

    // Process raw project context: decrypt and optionally trim based on step type.
    let project_context = if context_level.include_full_context {
        raw_context
            .map(|mut v| {
                // Decrypt any encrypted fields if DEK is available
                if let Some(dek) = dek {
                    crate::crypto::decrypt_json_recursive(dek, &mut v, "context");
                }
                serde_json::to_string_pretty(&v).unwrap_or_default()
            })
            .unwrap_or_default()
    } else {
        // Trimmed context: strip heavy sections before including
        raw_context
            .map(|mut v| {
                // Decrypt before trimming so trimming sees real content
                if let Some(dek) = dek {
                    crate::crypto::decrypt_json_recursive(dek, &mut v, "context");
                }
                trim_context(&mut v, &context_level);
                serde_json::to_string_pretty(&v).unwrap_or_default()
            })
            .unwrap_or_default()
    };

    // Task comments and updates — always needed for discussion.
    //
    // ROOT CAUSE FIX: Previously these were assembled into a single `discussion`
    // block placed AFTER the workflow section. This meant agents would read the
    // numbered workflow steps, form a plan, and only then encounter human
    // instructions buried at the bottom. Now:
    //   1. Human comments (most important) are separated from agent update logs
    //   2. The entire Task Discussion section is rendered BEFORE ## Your Job
    //   3. Task spec/notes from task.context are surfaced inline so agents see
    //      the full task description without needing to run `agent-cli task` first

    // Human comments -- discussion thread, treated as instructions (highest priority)
    let mut human_comments = String::new();
    for c in &task_comments {
        let author = c["author_name"].as_str().unwrap_or("human");
        let content = c["content"].as_str().unwrap_or("");
        let time = c["created_at"].as_str().unwrap_or("");
        if !content.is_empty() {
            human_comments.push_str(&format!("- [{time}] **{author}**: {content}\n"));
        }
    }

    // Agent updates (progress, blocker, artifact, etc.) -- audit trail
    let mut agent_updates_log = String::new();
    for u in &task_updates {
        let kind = u["kind"].as_str().unwrap_or("note");
        let content = u["content"].as_str().unwrap_or("");
        let time = u["created_at"].as_str().unwrap_or("");
        if !content.is_empty() {
            agent_updates_log.push_str(&format!("- [{time}] [{kind}]: {content}\n"));
        }
    }

    let discussion = if human_comments.is_empty() && agent_updates_log.is_empty() {
        "(none)\n".to_string()
    } else {
        let mut d = human_comments.clone();
        if !agent_updates_log.is_empty() {
            if !human_comments.is_empty() {
                d.push('\n');
            }
            d.push_str("**Previous agent activity:**\n");
            d.push_str(&agent_updates_log);
        }
        d
    };

    // Extract task spec and notes for inline display -- surfaced directly in the prompt
    // so the agent sees the full task description without needing to run `agent-cli task`.
    // Falls back through: spec -> description -> (empty)
    let task_context_obj = task_json.as_ref().and_then(|t| t["context"].as_object());
    let inline_spec = task_context_obj
        .and_then(|c| c.get("spec").or_else(|| c.get("description")))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let inline_notes = task_context_obj
        .and_then(|c| c.get("notes"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    // Extract acceptance_criteria and files for inline display
    let inline_acceptance: Vec<&str> = task_context_obj
        .and_then(|c| c.get("acceptance_criteria"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<&str>>()
        })
        .unwrap_or_default();
    let inline_files: Vec<&str> = task_context_obj
        .and_then(|c| c.get("files"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<&str>>()
        })
        .unwrap_or_default();

    // Detect decompose mode: when context.decompose is true, the agent should
    // split the task into subtasks instead of implementing it directly.
    let decompose_mode = task_context_obj
        .and_then(|c| c.get("decompose"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Build inline task spec block shown before the workflow
    let task_spec_inline = {
        let mut s = String::new();
        if !inline_spec.is_empty() {
            s.push_str(&format!("**Spec**: {inline_spec}\n\n"));
        }
        if !inline_notes.is_empty() {
            s.push_str(&format!("**Notes**: {inline_notes}\n\n"));
        }
        if !inline_files.is_empty() {
            s.push_str("**Files**:\n");
            for f in &inline_files {
                s.push_str(&format!("- `{f}`\n"));
            }
            s.push('\n');
        }
        if !inline_acceptance.is_empty() {
            s.push_str("**Acceptance Criteria**:\n");
            for ac in &inline_acceptance {
                s.push_str(&format!("- [ ] {ac}\n"));
            }
            s.push('\n');
        }
        s
    };

    // Build generated context from auto-docs (source:codegen knowledge entries).
    // Matches knowledge entries to task files by module prefix and injects relevant
    // module summaries so agents start with architectural understanding.
    let generated_context = build_generated_context(&inline_files, autodocs_res.as_deref());

    // Detect rework: extract review feedback from updates and verifications.
    // Only consider updates from the current pipeline cycle (after claimed_at)
    // to avoid stale REVIEW: artifacts from previous cycles triggering false REWORK.
    let claimed_at = task_json
        .as_ref()
        .and_then(|t| t["claimed_at"].as_str())
        .unwrap_or("");
    let review_feedback =
        extract_review_feedback(&task_updates, &verifications, &task_comments, claimed_at);

    let task_title = task_json
        .as_ref()
        .and_then(|t| t["title"].as_str())
        .unwrap_or("unknown");
    let rework_tag = if review_feedback.is_empty() {
        ""
    } else {
        " [REWORK]"
    };
    info!("task {tid}: step={resolved_step}{rework_tag} \"{task_title}\"");

    // Determine auth mode for identity section documentation
    let auth_mode = if std::env::var("DIRAIGENT_API_TOKEN").is_ok_and(|v| !v.is_empty()) {
        "Token (`Authorization: Bearer`)".to_string()
    } else if std::env::var("DIRAIGENT_DEV_USER_ID").is_ok_and(|v| !v.is_empty()) {
        "Dev header (`X-Dev-User-Id`)".to_string()
    } else {
        "None (no auth configured)".to_string()
    };

    // Resolve API base URL for verification endpoints
    let api_base = std::env::var("DIRAIGENT_API_URL")
        .unwrap_or_else(|_| "http://localhost:8082".into())
        .trim_end_matches("/v1")
        .to_string();

    // Extract step description from playbook step JSON (shown in prompt so
    // the agent understands the intent of the current step).
    let step_description = step_json
        .and_then(|s| s["description"].as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    // Build step-specific workflow
    let workflow = build_workflow(&WorkflowParams {
        step_name: &resolved_step,
        step_description: &step_description,
        step_json,
        agent_cli,
        task_id,
        project_id,
        short_id: tid.short(),
        repo_root,
        api_base: &api_base,
        playbook_id: &current_playbook_id,
        review_feedback: &review_feedback,
        decompose_mode,
        project_json: project_json.as_ref(),
        work_id: &first_work_id,
    });

    let step_desc_line = if step_description.is_empty() {
        String::new()
    } else {
        format!("\n- Step description: {step_description}")
    };

    let wt = worktree_path.display();
    format!(
        r#"## Identity
- AGENT_ID: {agent_id}
- PROJECT_ID: {project_id}
- TASK_ID: {task_id}
- Agent CLI: {agent_cli}
- Working directory: {wt}
- Pipeline step: {resolved_step}{step_desc_line}
- Auth mode: {auth_mode}

## Project Context
```json
{project_context}
```

## Active Work
{work_section}
{related_context}{generated_context}## Task Discussion (comments & feedback from humans — follow these instructions)
{task_spec_inline}{discussion}
{workflow}

## General Rules
- Do NOT run `git push` — the human pushes.
- Stay within your worktree: `{wt}`. Use absolute paths.
- Use the agent CLI (`{agent_cli}`) for ALL interactions with the Projects API.
- If blocked, post a blocker and release the task (transition to ready).
- Be concise in progress updates."#,
        agent_id = api.agent_id(),
    )
}

/// Controls what context sections to include in the prompt.
/// Heavier steps (implement) get full context; lighter steps (merge) get minimal.
struct ContextLevel {
    /// Include full project context (observations, knowledge, events, playbooks)
    include_full_context: bool,
    /// Include active work section
    include_work: bool,
    /// Include observations in trimmed context
    include_observations: bool,
    /// Include knowledge entries in trimmed context
    include_knowledge: bool,
    /// Include recent events in trimmed context
    include_events: bool,
    /// Include playbook definitions in trimmed context
    include_playbooks: bool,
    /// Max number of recent events to include (0 = all)
    max_events: usize,
}

impl ContextLevel {
    fn full() -> Self {
        ContextLevel {
            include_full_context: true,
            include_work: true,
            include_observations: true,
            include_knowledge: true,
            include_events: true,
            include_playbooks: true,
            max_events: 0,
        }
    }

    fn minimal() -> Self {
        ContextLevel {
            include_full_context: false,
            include_work: false,
            include_observations: false,
            include_knowledge: false,
            include_events: false,
            include_playbooks: false,
            max_events: 0,
        }
    }

    fn dream() -> Self {
        ContextLevel {
            include_full_context: false,
            include_work: true,
            include_observations: true,
            include_knowledge: true,
            include_events: true,
            include_playbooks: true,
            max_events: 5,
        }
    }

    fn for_step(step_name: &str) -> Self {
        match StepProfile::for_step(step_name) {
            // Review: needs task details, agent info, decisions. Skip heavy lists.
            StepProfile::Review => ContextLevel {
                include_full_context: false,
                include_work: false,
                include_observations: false,
                include_knowledge: false,
                include_events: false,
                include_playbooks: false,
                max_events: 0,
            },
            // Merge: minimal -- just needs task and project basics.
            StepProfile::Merge => ContextLevel {
                include_full_context: false,
                include_work: false,
                include_observations: false,
                include_knowledge: false,
                include_events: false,
                include_playbooks: false,
                max_events: 0,
            },
            // Dream: needs observations and knowledge to avoid duplicates.
            StepProfile::Dream => ContextLevel {
                include_full_context: false,
                include_work: true,
                include_observations: true,
                include_knowledge: true,
                include_events: true,
                include_playbooks: true,
                max_events: 5,
            },
            // Implement / rework: full context.
            StepProfile::Implement => ContextLevel {
                include_full_context: true,
                include_work: true,
                include_observations: true,
                include_knowledge: true,
                include_events: true,
                include_playbooks: true,
                max_events: 0,
            },
        }
    }
}

/// Trim heavy sections from project context JSON to reduce token count.
fn trim_context(context: &mut serde_json::Value, level: &ContextLevel) {
    if let Some(obj) = context.as_object_mut() {
        if !level.include_observations {
            obj.remove("open_observations");
        }
        if !level.include_knowledge {
            obj.remove("knowledge");
        }
        if !level.include_events {
            obj.remove("recent_events");
        } else if level.max_events > 0 {
            // Trim events to max_events most recent
            if let Some(serde_json::Value::Array(events)) = obj.get_mut("recent_events") {
                events.truncate(level.max_events);
            }
        }
        if !level.include_playbooks {
            obj.remove("playbooks");
        }
        // Always keep: project, agent, role, membership, ready_tasks, my_tasks, decisions, integrations
    }
}

/// Build the related context section from the related items API response.
///
/// Formats knowledge, decisions, and observations with title, relevance score,
/// and snippet. Returns an empty string if no related items are found.
fn build_related_context_section(related: &serde_json::Value) -> String {
    let knowledge = related["knowledge"].as_array();
    let decisions = related["decisions"].as_array();
    let observations = related["observations"].as_array();

    let has_knowledge = knowledge.is_some_and(|arr| !arr.is_empty());
    let has_decisions = decisions.is_some_and(|arr| !arr.is_empty());
    let has_observations = observations.is_some_and(|arr| !arr.is_empty());

    if !has_knowledge && !has_decisions && !has_observations {
        return String::new();
    }

    let mut section = String::from(
        "## Relevant Context for This Task\n\n\
         The following knowledge entries and decisions are related to this task:\n\n",
    );

    if let Some(items) = knowledge.filter(|arr| !arr.is_empty()) {
        section.push_str("### Related Knowledge\n");
        for item in items {
            let title = item["title"].as_str().unwrap_or("untitled");
            let score = item["relevance_score"].as_f64().unwrap_or(0.0);
            let snippet = item["snippet"].as_str().unwrap_or("");
            section.push_str(&format!(
                "- **{title}** (relevance: {score:.1}): {snippet}\n"
            ));
        }
        section.push('\n');
    }

    if let Some(items) = decisions.filter(|arr| !arr.is_empty()) {
        section.push_str("### Related Decisions\n");
        for item in items {
            let title = item["title"].as_str().unwrap_or("untitled");
            let score = item["relevance_score"].as_f64().unwrap_or(0.0);
            let snippet = item["snippet"].as_str().unwrap_or("");
            section.push_str(&format!(
                "- **{title}** (relevance: {score:.1}): {snippet}\n"
            ));
        }
        section.push('\n');
    }

    if let Some(items) = observations.filter(|arr| !arr.is_empty()) {
        section.push_str("### Related Observations\n");
        for item in items {
            let title = item["title"].as_str().unwrap_or("untitled");
            let score = item["relevance_score"].as_f64().unwrap_or(0.0);
            let snippet = item["snippet"].as_str().unwrap_or("");
            section.push_str(&format!(
                "- **{title}** (relevance: {score:.1}): {snippet}\n"
            ));
        }
        section.push('\n');
    }

    section
}

/// Build a generated context section from auto-docs (knowledge entries tagged `source:codegen`).
///
/// Matches entries to the task's `context.files` by module prefix: a knowledge entry
/// titled "Module: apps/api" matches any file starting with `apps/api/`. Generic
/// entries (e.g. "Codebase Dependency Graph") are included when at least one module-specific
/// entry matches. The output is capped at ~4000 tokens (~16000 chars) to avoid
/// bloating the prompt. Returns an empty string if no entries match or none exist.
fn build_generated_context(task_files: &[&str], autodocs: Option<&[serde_json::Value]>) -> String {
    let entries = match autodocs {
        Some(e) if !e.is_empty() => e,
        _ => return String::new(),
    };

    if task_files.is_empty() {
        return String::new();
    }

    const MAX_CHARS: usize = 16000; // ~4000 tokens at ~4 chars/token

    let mut matched = Vec::new();
    let mut generic = Vec::new();

    for entry in entries {
        let title = entry["title"].as_str().unwrap_or("");
        let content = entry["content"].as_str().unwrap_or("");
        if title.is_empty() || content.is_empty() {
            continue;
        }

        // Module entries: title starts with "Module: <prefix>"
        if let Some(module_prefix) = title.strip_prefix("Module: ") {
            // Check if any task file belongs to this module
            let matches = task_files.iter().any(|f| f.starts_with(module_prefix));
            if matches {
                matched.push((title, content));
            }
        } else {
            // Generic entries (e.g. "Codebase Dependency Graph", "API Surface Map")
            generic.push((title, content));
        }
    }

    // Only include generic entries if at least one module-specific entry matched
    if matched.is_empty() {
        return String::new();
    }

    let mut section = String::from(
        "## Generated Context (auto-docs for files in scope)\n\n\
         The following auto-generated documentation is relevant to the files this task modifies.\n\n",
    );
    let mut total_chars = section.len();

    // Add module-specific entries first (most relevant)
    for (title, content) in &matched {
        let block = format!("### {title}\n{content}\n\n");
        if total_chars + block.len() > MAX_CHARS {
            // Truncate content to fit within budget
            let remaining = MAX_CHARS.saturating_sub(total_chars);
            if remaining > 100 {
                let header = format!("### {title}\n");
                let content_budget = remaining.saturating_sub(header.len() + 20);
                // Truncate at a line boundary to avoid partial lines
                let truncated: String = content.chars().take(content_budget).collect();
                let last_newline = truncated.rfind('\n').unwrap_or(truncated.len());
                section.push_str(&header);
                section.push_str(&truncated[..last_newline]);
                section.push_str("\n... (truncated)\n\n");
            }
            break;
        }
        section.push_str(&block);
        total_chars += block.len();
    }

    // Add generic entries if there's room
    for (title, content) in &generic {
        let block = format!("### {title}\n{content}\n\n");
        if total_chars + block.len() > MAX_CHARS {
            break;
        }
        section.push_str(&block);
        total_chars += block.len();
    }

    section
}

/// Build the work section with progress info.
///
/// Work item progress calls are parallelized using `join_all` to avoid the N+1
/// sequential API call problem (one call per active work item).
async fn build_work_section(api: &dyn TaskSource, project_id: &str) -> String {
    let work_items = api.get_work_items(project_id).await.unwrap_or_default();
    let active_work: Vec<_> = work_items
        .iter()
        .filter(|w| w["status"].as_str().unwrap_or("active") == "active")
        .collect();

    if active_work.is_empty() {
        return String::new();
    }

    // Fetch all work item progress in parallel (fixes N+1 sequential calls)
    let progress_futures: Vec<_> = active_work
        .iter()
        .map(|w| {
            let work_id = w["id"].as_str().unwrap_or("").to_string();
            async move {
                if !work_id.is_empty() {
                    api.get_work_item_progress(&work_id).await.ok()
                } else {
                    None
                }
            }
        })
        .collect();

    let progress_results = futures_util::future::join_all(progress_futures).await;

    let mut work_section = String::new();
    for (w, progress) in active_work.iter().zip(progress_results) {
        let title = w["title"].as_str().unwrap_or("untitled");
        let desc = w["description"].as_str().unwrap_or("");
        let progress_str = progress
            .map(|p| {
                let done = p["done_tasks"].as_u64().unwrap_or(0);
                let total = p["total_tasks"].as_u64().unwrap_or(0);
                format!("{done}/{total} tasks")
            })
            .unwrap_or_else(|| "progress unknown".to_string());
        work_section.push_str(&format!(
            "- **{title}**: {desc} (progress: {progress_str})\n"
        ));
    }
    work_section
}

struct WorkflowParams<'a> {
    step_name: &'a str,
    step_description: &'a str,
    step_json: Option<&'a serde_json::Value>,
    agent_cli: &'a str,
    task_id: &'a str,
    project_id: &'a str,
    short_id: &'a str,
    repo_root: &'a Path,
    api_base: &'a str,
    playbook_id: &'a str,
    review_feedback: &'a str,
    /// When true, the agent should decompose this task into subtasks
    /// instead of implementing it directly (set via `context.decompose`).
    decompose_mode: bool,
    /// Full project JSON — used for `{{project.<key>}}` substitution.
    /// Top-level fields (default_branch, slug, etc.) and metadata fields are both available.
    project_json: Option<&'a serde_json::Value>,
    /// First work item ID linked to this task (empty string if none).
    /// Used for work item inheritance: subtasks inherit the parent's work item.
    work_id: &'a str,
}

/// Substitute `{{variable}}` placeholders in a step description template.
///
/// Built-in variables (from runtime context): agent_cli, task_id, project_id,
/// short_id, repo_root, api_base, auth_header, agent_id, playbook_id, branch,
/// review_feedback.
///
/// Project variables: `{{project.<key>}}` — any string field from the project
/// record (e.g. `{{project.default_branch}}`, `{{project.slug}}`), plus any
/// string field from the project's metadata JSONB (e.g. `{{project.branch}}`,
/// `{{project.slack_channel}}`). Top-level fields take precedence over metadata.
///
/// Custom variables from `step_json["vars"]` (a string→string object) are
/// substituted after project metadata, so playbook authors can define
/// step-specific placeholders like `{{lint_cmd}}` or `{{test_cmd}}`.
fn substitute_description(
    template: &str,
    p: &WorkflowParams<'_>,
    auth_header: &str,
    agent_id: &str,
) -> String {
    let branch = TaskId::new(p.short_id).branch_name();
    let mut result = template
        .replace("{{agent_cli}}", p.agent_cli)
        .replace("{{task_id}}", p.task_id)
        .replace("{{project_id}}", p.project_id)
        .replace("{{short_id}}", p.short_id)
        .replace("{{repo_root}}", &p.repo_root.display().to_string())
        .replace("{{api_base}}", p.api_base)
        .replace("{{auth_header}}", auth_header)
        .replace("{{agent_id}}", agent_id)
        .replace("{{playbook_id}}", p.playbook_id)
        .replace("{{branch}}", &branch)
        .replace("{{review_feedback}}", p.review_feedback)
        .replace("{{work_id}}", p.work_id);

    // Apply project vars: {{project.<key>}}
    // Resolution order: top-level project fields first, then metadata fields.
    // This makes {{project.default_branch}}, {{project.slug}} etc. work alongside
    // metadata keys like {{project.branch}}, {{project.slack_channel}}.
    if let Some(proj) = p.project_json.and_then(|j| j.as_object()) {
        for (key, val) in proj {
            if let Some(v) = val.as_str() {
                result = result.replace(&format!("{{{{project.{key}}}}}"), v);
            }
        }
        // Also expand metadata sub-keys as {{project.<key>}}
        if let Some(meta) = proj.get("metadata").and_then(|m| m.as_object()) {
            for (key, val) in meta {
                if let Some(v) = val.as_str() {
                    let placeholder = format!("{{{{project.{key}}}}}");
                    // Only apply if not already substituted by a top-level field
                    if result.contains(&placeholder) {
                        result = result.replace(&placeholder, v);
                    }
                }
            }
        }
    }

    // Apply custom vars from step JSON
    if let Some(vars) = p.step_json.and_then(|s| s["vars"].as_object()) {
        for (key, val) in vars {
            if let Some(v) = val.as_str() {
                result = result.replace(&format!("{{{{{key}}}}}"), v);
            }
        }
    }

    result
}

fn build_workflow(p: &WorkflowParams<'_>) -> String {
    let agent_id = std::env::var("AGENT_ID").unwrap_or_default();
    let dev_user_id = std::env::var("DIRAIGENT_DEV_USER_ID").unwrap_or_default();
    let api_token = std::env::var("DIRAIGENT_API_TOKEN").unwrap_or_default();
    // Use Bearer token auth when configured; fall back to X-Dev-User-Id for local/dev mode.
    let auth_header = if !api_token.is_empty() {
        format!("Authorization: Bearer {api_token}")
    } else {
        format!("X-Dev-User-Id: {dev_user_id}")
    };

    // Decompose mode: override the normal step workflow with decomposition instructions.
    // This is triggered by context.decompose=true, set via the "Spawn subtasks" checkbox
    // in the create task dialog. The agent splits the task into subtasks instead of
    // implementing it directly.
    if p.decompose_mode {
        let agent_cli = p.agent_cli;
        let task_id = p.task_id;
        let project_id = p.project_id;
        let playbook_id = p.playbook_id;
        // Include work_id in subtask creation so subtasks inherit the parent's work item.
        let work_id_field = if p.work_id.is_empty() {
            String::new()
        } else {
            format!(r#""work_id": "{}", "#, p.work_id)
        };
        return format!(
            r#"## Your Job: DECOMPOSE INTO SUBTASKS

**This task is marked for decomposition.** Do NOT implement it directly.
Instead, analyze the spec and break it into smaller, well-scoped subtasks.

1. **Read the task**: Run `{agent_cli} task {task_id}` to get the full spec.
2. **Claim the task**: Run `{agent_cli} claim {task_id}`
3. **Analyze the spec**: Identify logical units of work that can be implemented independently.
4. **Create subtasks**: For each unit, create a task with a clear spec, files, test_cmd, and acceptance_criteria:
   ```
   {agent_cli} create {project_id} '{{{work_id_field}"parent_id": "{task_id}", "title": "...", "kind": "feature", "urgent": false, "playbook_id": "{playbook_id}", "context": {{"spec": "...", "files": ["..."], "test_cmd": "...", "acceptance_criteria": ["..."]}}}}'
   ```
5. **Wire dependencies**: If subtask B depends on subtask A:
   ```
   {agent_cli} depend <B_id> <A_id>
   ```
6. **Transition subtasks to ready**: For each subtask:
   ```
   {agent_cli} transition <subtask_id> ready
   ```
7. **Report progress**: `{agent_cli} progress {task_id} "Decomposed into N subtasks"`
8. **Complete**: `{agent_cli} transition {task_id} done`

**Guidelines**:
- Each subtask should be small enough for a single agent to implement in one session
- Include concrete file paths, test commands, and acceptance criteria in each subtask
- Set dependencies so subtasks that build on each other run in the right order
- Use the same playbook_id as the parent task so subtasks follow the same pipeline
- Always include `"parent_id": "{task_id}"` so subtasks are linked to this parent task
- Do NOT write any code — only create and wire subtasks
- Do NOT run `git push`"#
        );
    }

    // If the step has a description, use it (with variable substitution if
    // it contains {{}} markers). This makes playbooks fully data-driven.
    if !p.step_description.is_empty() {
        let mut result = substitute_description(p.step_description, p, &auth_header, &agent_id);
        // For rework: prepend review feedback if present and not already in template
        if !p.review_feedback.is_empty() && !p.step_description.contains("{{review_feedback}}") {
            result = format!(
                "## Review Feedback — Issues to Fix\n{}\n\n{}",
                p.review_feedback, result
            );
        }
        return result;
    }

    // Generic fallback for steps without a description.
    let agent_cli = p.agent_cli;
    let task_id = p.task_id;
    let step_label = p.step_name.to_uppercase();
    let mut fallback = format!(
        r#"## Your Job: {step_label}

1. **Read the task**: Run `{agent_cli} task {task_id}` to understand what needs to be done.
2. **Claim the task**: Run `{agent_cli} claim {task_id}`
3. **Do the work** as described in the task spec and discussion above.
4. **Report progress**: `{agent_cli} progress {task_id} "what was done"`
5. **Complete**: `{agent_cli} transition {task_id} done`
   - If blocked: `{agent_cli} blocker {task_id} "what's blocking"`, then `{agent_cli} transition {task_id} ready`

**Rules**: Do NOT run `git push`. Stay in your worktree. Be concise."#
    );

    // Prepend review feedback for rework
    if !p.review_feedback.is_empty() {
        fallback = format!(
            "## Review Feedback — Issues to Fix\n{}\n\n{}",
            p.review_feedback, fallback
        );
    }

    fallback
}

/// Extract review feedback from task updates, verifications, and human comments.
///
/// Returns a formatted string of review issues when the task has been through
/// a review cycle, or an empty string for first-time implementation.
///
/// Signals rework when any of:
/// - A "blocker" update exists (reviewer flagged issues)
/// - An "artifact" update contains "REVIEW:" (reviewer findings)
/// - A verification has status "fail" (failed checks)
/// - Human comments exist that are newer than the latest agent update
///   (e.g. posted during human_review)
///
/// `claimed_at` restricts the scan of agent updates/verifications to the current
/// pipeline cycle. Human comments are NOT filtered by `claimed_at` because they
/// are posted BEFORE the agent re-claims the task; instead they are filtered by
/// comparing against the latest agent update timestamp.
fn extract_review_feedback(
    task_updates: &[serde_json::Value],
    verifications: &[serde_json::Value],
    task_comments: &[serde_json::Value],
    claimed_at: &str,
) -> String {
    let mut feedback_items = Vec::new();

    // Filter updates to only those from the current cycle (created_at >= claimed_at).
    // This prevents stale REVIEW: artifacts from previous completed cycles from
    // triggering false REWORK when a task is reopened for a new implement cycle.
    let current_cycle_updates: Vec<&serde_json::Value> = if claimed_at.is_empty() {
        task_updates.iter().collect()
    } else {
        task_updates
            .iter()
            .filter(|u| u["created_at"].as_str().is_some_and(|ts| ts >= claimed_at))
            .collect()
    };

    // Check if a review artifact exists -- only blockers posted alongside a
    // REVIEW: artifact are genuine review rejections. Implement-step agents
    // also post blockers (e.g. "test failed") but never post REVIEW: artifacts,
    // so this filter prevents false REWORK triggers on retry cycles.
    let has_review_artifact = current_cycle_updates.iter().any(|u| {
        u["kind"].as_str() == Some("artifact")
            && u["content"].as_str().is_some_and(|c| c.contains("REVIEW:"))
    });

    // 1. Collect review feedback from agent updates
    for u in &current_cycle_updates {
        let kind = u["kind"].as_str().unwrap_or("");
        let content = u["content"].as_str().unwrap_or("");
        if content.is_empty() {
            continue;
        }
        match kind {
            // Only include blockers as review feedback if a REVIEW: artifact
            // confirms that a review step actually ran and rejected the work.
            "blocker" if has_review_artifact => {
                feedback_items.push(format!("**Blocker**: {content}"));
            }
            "artifact" if content.contains("REVIEW:") => {
                feedback_items.push(format!("**Review**: {content}"));
            }
            _ => {}
        }
    }

    // 2. Failed verifications (filtered by claimed_at to exclude stale cycles)
    let current_cycle_verifications: Vec<&serde_json::Value> = if claimed_at.is_empty() {
        verifications.iter().collect()
    } else {
        verifications
            .iter()
            .filter(|v| v["created_at"].as_str().is_some_and(|ts| ts >= claimed_at))
            .collect()
    };
    for v in &current_cycle_verifications {
        let status = v["status"].as_str().unwrap_or("");
        if status == "fail" {
            let title = v["title"].as_str().unwrap_or("check failed");
            let detail = v["detail"].as_str().unwrap_or("");
            if detail.is_empty() {
                feedback_items.push(format!("**Failed check**: {title}"));
            } else {
                feedback_items.push(format!("**Failed check**: {title} -- {detail}"));
            }
        }
    }

    // 3. Human review comments: include comments posted AFTER the latest agent
    // update. This captures human feedback from the human_review phase without
    // including initial task discussion comments.
    //
    // We don't filter by claimed_at because human comments are posted before the
    // agent re-claims the task (during human_review → ready transition).
    let latest_agent_update = task_updates
        .iter()
        .filter_map(|u| u["created_at"].as_str())
        .max()
        .unwrap_or("");

    if !latest_agent_update.is_empty() {
        for c in task_comments {
            let comment_time = c["created_at"].as_str().unwrap_or("");
            let content = c["content"].as_str().unwrap_or("");
            if content.is_empty() || comment_time <= latest_agent_update {
                continue;
            }
            let author = c["author_name"].as_str().unwrap_or("human");
            feedback_items.push(format!("**Human review** ({author}): {content}"));
        }
    }

    if feedback_items.is_empty() {
        String::new()
    } else {
        feedback_items
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn read_file_or_empty(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_params<'a>(
        step_name: &'a str,
        review_feedback: &'a str,
        repo_root: &'a Path,
    ) -> WorkflowParams<'a> {
        WorkflowParams {
            step_name,
            step_description: "",
            step_json: None,
            agent_cli: "/usr/bin/agent-cli",
            task_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            project_id: "11111111-2222-3333-4444-555555555555",
            short_id: "aaaaaaaa-bbb",
            repo_root,
            api_base: "http://localhost:8082",
            playbook_id: "pppppppp-qqqq-rrrr-ssss-tttttttttttt",
            review_feedback,
            decompose_mode: false,
            project_json: None,
            work_id: "",
        }
    }

    #[test]
    fn generic_fallback_contains_step_name() {
        let root = PathBuf::from("/tmp/test-repo");
        for step in &["review", "implement", "merge", "dream"] {
            let params = make_params(step, "", &root);
            let output = build_workflow(&params);
            let heading = format!("## Your Job: {}", step.to_uppercase());
            assert!(
                output.contains(&heading),
                "{step} step should contain generic heading '{heading}'"
            );
        }
    }

    #[test]
    fn generic_fallback_contains_required_commands() {
        let root = PathBuf::from("/tmp/test-repo");
        let params = make_params("implement", "", &root);
        let output = build_workflow(&params);
        assert!(
            output.contains("agent-cli task"),
            "generic fallback should contain 'agent-cli task' command"
        );
        assert!(
            output.contains("agent-cli claim"),
            "generic fallback should contain 'agent-cli claim' command"
        );
        assert!(
            output.contains("agent-cli transition"),
            "generic fallback should contain 'agent-cli transition' command"
        );
    }

    #[test]
    fn review_feedback_prepended_when_present() {
        let root = PathBuf::from("/tmp/test-repo");
        let params = make_params("implement", "- **Blocker**: Missing error handling", &root);
        let output = build_workflow(&params);
        assert!(
            output.contains("Review Feedback"),
            "implement step with review_feedback should contain Review Feedback heading"
        );
        assert!(
            output.contains("Missing error handling"),
            "output should include the review feedback content"
        );
    }

    #[test]
    fn review_feedback_not_present_when_empty() {
        let root = PathBuf::from("/tmp/test-repo");
        let params = make_params("implement", "", &root);
        let output = build_workflow(&params);
        assert!(
            !output.contains("Review Feedback"),
            "implement step without review feedback should NOT contain Review Feedback"
        );
    }

    // ── extract_review_feedback tests ──────────────────────────

    fn make_update(kind: &str, content: &str) -> serde_json::Value {
        make_update_at(kind, content, "2026-01-01T00:00:00Z")
    }

    fn make_update_at(kind: &str, content: &str, created_at: &str) -> serde_json::Value {
        serde_json::json!({
            "kind": kind,
            "content": content,
            "created_at": created_at
        })
    }

    #[test]
    fn implement_blocker_without_review_artifact_does_not_trigger_rework() {
        // Implement agent posted a blocker (test failure), but no REVIEW: artifact
        let updates = vec![make_update("blocker", "test failed: 2 assertions")];
        let feedback = extract_review_feedback(&updates, &[], &[], "");
        assert!(
            feedback.is_empty(),
            "implement-step blockers without a REVIEW: artifact should NOT trigger rework, got: {feedback}"
        );
    }

    #[test]
    fn review_rejection_with_artifact_and_blocker_triggers_rework() {
        // Review step posted both a REVIEW: artifact and a blocker
        let updates = vec![
            make_update("artifact", "REVIEW: Missing error handling in foo.rs"),
            make_update("blocker", "Fix error handling before re-submitting"),
        ];
        let feedback = extract_review_feedback(&updates, &[], &[], "");
        assert!(
            feedback.contains("Blocker"),
            "blockers should be included when REVIEW: artifact exists"
        );
        assert!(
            feedback.contains("REVIEW:"),
            "REVIEW: artifact should be included as feedback"
        );
    }

    #[test]
    fn review_artifact_alone_triggers_rework() {
        // Review step posted a REVIEW: artifact but no blocker (approved with comments)
        let updates = vec![make_update(
            "artifact",
            "REVIEW: Code looks good but minor style issue",
        )];
        let feedback = extract_review_feedback(&updates, &[], &[], "");
        assert!(
            !feedback.is_empty(),
            "REVIEW: artifact alone should trigger rework"
        );
    }

    #[test]
    fn no_updates_means_no_rework() {
        let updates: Vec<serde_json::Value> = vec![];
        let feedback = extract_review_feedback(&updates, &[], &[], "");
        assert!(feedback.is_empty(), "no updates should mean no rework");
    }

    #[test]
    fn stale_review_artifact_does_not_trigger_rework_on_reopened_task() {
        // Cycle 1: review posted REVIEW: artifact + blocker at T1
        // Task was merged, then reopened. New claimed_at is T2.
        // Cycle 2: implement agent posts a blocker at T3.
        // The old REVIEW: artifact (T1) should NOT cause REWORK.
        let updates = vec![
            make_update_at("artifact", "REVIEW: Missing tests", "2026-01-01T10:00:00Z"),
            make_update_at("blocker", "Review rejection", "2026-01-01T10:05:00Z"),
            make_update_at(
                "blocker",
                "test failed: 3 assertions",
                "2026-01-02T14:00:00Z",
            ),
        ];
        // claimed_at = T2 (after cycle 1 completed, before cycle 2 blocker)
        let feedback = extract_review_feedback(&updates, &[], &[], "2026-01-02T12:00:00Z");
        assert!(
            feedback.is_empty(),
            "stale REVIEW: artifact from previous cycle should NOT trigger rework, got: {feedback}"
        );
    }

    #[test]
    fn current_cycle_review_rejection_triggers_rework() {
        // Cycle 1: old review artifact (before re-claim)
        // Cycle 2: new review artifact + blocker (after claimed_at)
        let updates = vec![
            make_update_at(
                "artifact",
                "REVIEW: Old cycle feedback",
                "2026-01-01T10:00:00Z",
            ),
            make_update_at(
                "artifact",
                "REVIEW: Current cycle rejection",
                "2026-01-02T15:00:00Z",
            ),
            make_update_at("blocker", "Fix the bug", "2026-01-02T15:05:00Z"),
        ];
        // claimed_at is after cycle 1 but before cycle 2 updates
        let feedback = extract_review_feedback(&updates, &[], &[], "2026-01-02T12:00:00Z");
        assert!(
            feedback.contains("Blocker"),
            "current-cycle blocker with REVIEW: artifact should trigger rework"
        );
        assert!(
            feedback.contains("Current cycle rejection"),
            "current-cycle REVIEW: artifact should be included"
        );
    }

    #[test]
    fn progress_updates_do_not_trigger_rework() {
        let updates = vec![make_update("progress", "implemented the feature")];
        let feedback = extract_review_feedback(&updates, &[], &[], "");
        assert!(
            feedback.is_empty(),
            "progress updates should not trigger rework"
        );
    }

    #[test]
    fn stale_failed_verification_does_not_trigger_rework_on_reopened_task() {
        let updates: Vec<serde_json::Value> = vec![];
        let verifications = vec![serde_json::json!({
            "id": "v1",
            "task_id": "task1",
            "status": "fail",
            "title": "Old CI failure",
            "detail": "from previous cycle",
            "created_at": "2026-01-01T08:00:00Z"
        })];
        // claimed_at is AFTER the stale verification
        let feedback =
            extract_review_feedback(&updates, &verifications, &[], "2026-01-02T12:00:00Z");
        assert!(
            feedback.is_empty(),
            "stale failed verification from previous cycle should NOT trigger rework, got: {feedback}"
        );
    }

    #[test]
    fn current_cycle_failed_verification_triggers_rework() {
        let updates: Vec<serde_json::Value> = vec![];
        let verifications = vec![
            serde_json::json!({
                "id": "v1",
                "task_id": "task1",
                "status": "fail",
                "title": "Old CI failure",
                "detail": "from previous cycle",
                "created_at": "2026-01-01T08:00:00Z"
            }),
            serde_json::json!({
                "id": "v2",
                "task_id": "task1",
                "status": "fail",
                "title": "Current CI failure",
                "detail": "from current cycle",
                "created_at": "2026-01-02T15:00:00Z"
            }),
        ];
        // claimed_at filters out the old verification but keeps the current one
        let feedback =
            extract_review_feedback(&updates, &verifications, &[], "2026-01-02T12:00:00Z");
        assert!(
            feedback.contains("Current CI failure"),
            "current-cycle failed verification should trigger rework, got: {feedback}"
        );
        assert!(
            !feedback.contains("Old CI failure"),
            "stale failed verification should be filtered out, got: {feedback}"
        );
    }

    #[test]
    fn empty_claimed_at_includes_all_verifications() {
        let updates: Vec<serde_json::Value> = vec![];
        let verifications = vec![serde_json::json!({
            "id": "v1",
            "task_id": "task-1",
            "status": "fail",
            "title": "Old failure",
            "detail": "",
            "created_at": "2026-01-01T10:00:00Z"
        })];

        // Empty claimed_at should include all verifications (backwards compatible)
        let feedback = extract_review_feedback(&updates, &verifications, &[], "");

        assert!(
            feedback.contains("Old failure"),
            "empty claimed_at should include all verifications, got: {feedback}"
        );
    }

    #[test]
    fn failed_verification_with_detail_formats_correctly() {
        let updates: Vec<serde_json::Value> = vec![];
        let verifications = vec![serde_json::json!({
            "id": "v1",
            "task_id": "task1",
            "status": "fail",
            "title": "CI check",
            "detail": "3 tests failed in module foo",
            "created_at": "2026-01-01T10:00:00Z"
        })];
        let feedback = extract_review_feedback(&updates, &verifications, &[], "");
        assert!(
            feedback.contains("**Failed check**: CI check -- 3 tests failed in module foo"),
            "failed verification with detail should format as 'title -- detail', got: {feedback}"
        );
    }

    #[test]
    fn passed_verification_does_not_trigger_rework() {
        let updates: Vec<serde_json::Value> = vec![];
        let verifications = vec![serde_json::json!({
            "id": "v1",
            "task_id": "task1",
            "status": "pass",
            "title": "CI check",
            "detail": "all green",
            "created_at": "2026-01-01T10:00:00Z"
        })];
        let feedback = extract_review_feedback(&updates, &verifications, &[], "");
        assert!(
            feedback.is_empty(),
            "passed verifications should not trigger rework, got: {feedback}"
        );
    }

    // ── human review comment tests ──────────────────────────

    fn make_comment(content: &str, created_at: &str) -> serde_json::Value {
        serde_json::json!({
            "content": content,
            "author_name": "human",
            "created_at": created_at
        })
    }

    #[test]
    fn human_comment_after_agent_work_triggers_rework() {
        // Agent posted progress at T1, human posted review comment at T2
        let updates = vec![make_update_at(
            "progress",
            "implemented feature",
            "2026-01-01T10:00:00Z",
        )];
        let comments = vec![make_comment(
            "The button should be blue not red",
            "2026-01-01T12:00:00Z",
        )];
        let feedback = extract_review_feedback(&updates, &[], &comments, "");
        assert!(
            feedback.contains("Human review"),
            "human comment after agent work should trigger rework, got: {feedback}"
        );
        assert!(
            feedback.contains("The button should be blue not red"),
            "human comment content should be included, got: {feedback}"
        );
    }

    #[test]
    fn human_comment_before_agent_work_does_not_trigger_rework() {
        // Human posted discussion comment at T1, agent posted progress at T2
        let updates = vec![make_update_at(
            "progress",
            "implemented feature",
            "2026-01-01T12:00:00Z",
        )];
        let comments = vec![make_comment(
            "Initial instructions for the task",
            "2026-01-01T08:00:00Z",
        )];
        let feedback = extract_review_feedback(&updates, &[], &comments, "");
        assert!(
            feedback.is_empty(),
            "human comment before agent work should NOT trigger rework, got: {feedback}"
        );
    }

    #[test]
    fn human_comment_without_any_agent_updates_does_not_trigger_rework() {
        // No agent updates exist — task hasn't been worked on yet
        let updates: Vec<serde_json::Value> = vec![];
        let comments = vec![make_comment(
            "Initial task discussion",
            "2026-01-01T10:00:00Z",
        )];
        let feedback = extract_review_feedback(&updates, &[], &comments, "");
        assert!(
            feedback.is_empty(),
            "human comment with no prior agent work should NOT trigger rework, got: {feedback}"
        );
    }

    #[test]
    fn human_comment_combined_with_agent_review_feedback() {
        // Agent review step rejected + human also posted comment during human_review
        let updates = vec![
            make_update_at(
                "artifact",
                "REVIEW: Missing error handling in foo.rs",
                "2026-01-01T10:00:00Z",
            ),
            make_update_at("blocker", "Fix error handling", "2026-01-01T10:05:00Z"),
        ];
        let comments = vec![make_comment(
            "Also please fix the color scheme",
            "2026-01-01T12:00:00Z",
        )];
        let feedback = extract_review_feedback(&updates, &[], &comments, "");
        assert!(
            feedback.contains("Blocker"),
            "agent blocker should be in feedback"
        );
        assert!(
            feedback.contains("REVIEW:"),
            "agent review artifact should be in feedback"
        );
        assert!(
            feedback.contains("Also please fix the color scheme"),
            "human review comment should also be in feedback"
        );
    }

    #[test]
    fn human_comment_includes_author_name() {
        let updates = vec![make_update_at("progress", "done", "2026-01-01T10:00:00Z")];
        let comments = vec![serde_json::json!({
            "content": "Fix the spacing",
            "author_name": "Roman",
            "created_at": "2026-01-01T12:00:00Z"
        })];
        let feedback = extract_review_feedback(&updates, &[], &comments, "");
        assert!(
            feedback.contains("(Roman)"),
            "feedback should include the author name, got: {feedback}"
        );
    }

    #[test]
    fn dream_step_uses_generic_fallback() {
        let root = PathBuf::from("/tmp/test-repo");
        let params = make_params("dream", "", &root);
        let output = build_workflow(&params);
        assert!(
            output.contains("## Your Job: DREAM"),
            "dream step should contain generic heading"
        );
        assert!(
            output.contains("agent-cli transition"),
            "dream step should contain 'agent-cli transition' command"
        );
    }

    // ── ContextLevel::for_step tests ────────────────────────

    #[test]
    fn context_level_implement_has_full_context() {
        let level = ContextLevel::for_step("implement");
        assert!(level.include_full_context);
        assert!(level.include_work);
        assert!(level.include_observations);
        assert!(level.include_knowledge);
        assert!(level.include_events);
        assert!(level.include_playbooks);
        assert_eq!(level.max_events, 0);
    }

    #[test]
    fn context_level_review_has_minimal_context() {
        let level = ContextLevel::for_step("review");
        assert!(!level.include_full_context);
        assert!(!level.include_work);
        assert!(!level.include_observations);
        assert!(!level.include_knowledge);
        assert!(!level.include_events);
        assert!(!level.include_playbooks);
    }

    #[test]
    fn context_level_merge_has_minimal_context() {
        let level = ContextLevel::for_step("merge");
        assert!(!level.include_full_context);
        assert!(!level.include_work);
        assert!(!level.include_observations);
        assert!(!level.include_knowledge);
        assert!(!level.include_events);
        assert!(!level.include_playbooks);
    }

    #[test]
    fn context_level_dream_has_observations_but_not_full() {
        let level = ContextLevel::for_step("dream");
        assert!(!level.include_full_context);
        assert!(level.include_work);
        assert!(level.include_observations);
        assert!(level.include_knowledge);
        assert!(level.include_events);
        assert!(level.include_playbooks);
        assert_eq!(level.max_events, 5);
    }

    // ── trim_context tests ───────────────────────────────────

    fn sample_context() -> serde_json::Value {
        serde_json::json!({
            "project": {"id": "p1", "name": "test"},
            "agent": {"id": "a1"},
            "role": {"id": "r1"},
            "membership": {"id": "m1"},
            "ready_tasks": [{"id": "t1"}],
            "my_tasks": [{"id": "t2"}],
            "decisions": [{"id": "d1"}],
            "integrations": [],
            "open_observations": [{"id": "o1"}, {"id": "o2"}],
            "knowledge": [{"id": "k1"}],
            "recent_events": [
                {"id": "e1"}, {"id": "e2"}, {"id": "e3"},
                {"id": "e4"}, {"id": "e5"}, {"id": "e6"},
                {"id": "e7"}
            ],
            "playbooks": [{"id": "pb1"}]
        })
    }

    #[test]
    fn trim_context_removes_observations_when_not_included() {
        let mut ctx = sample_context();
        let level = ContextLevel::minimal();
        trim_context(&mut ctx, &level);
        assert!(ctx.get("open_observations").is_none());
    }

    #[test]
    fn trim_context_keeps_observations_when_included() {
        let mut ctx = sample_context();
        let level = ContextLevel::dream();
        trim_context(&mut ctx, &level);
        assert!(ctx.get("open_observations").is_some());
        assert_eq!(ctx["open_observations"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn trim_context_removes_knowledge_when_not_included() {
        let mut ctx = sample_context();
        let level = ContextLevel::minimal();
        trim_context(&mut ctx, &level);
        assert!(ctx.get("knowledge").is_none());
    }

    #[test]
    fn trim_context_removes_playbooks_when_not_included() {
        let mut ctx = sample_context();
        let level = ContextLevel::minimal();
        trim_context(&mut ctx, &level);
        assert!(ctx.get("playbooks").is_none());
    }

    #[test]
    fn trim_context_removes_events_when_not_included() {
        let mut ctx = sample_context();
        let level = ContextLevel::minimal();
        trim_context(&mut ctx, &level);
        assert!(ctx.get("recent_events").is_none());
    }

    #[test]
    fn trim_context_truncates_events_to_max() {
        let mut ctx = sample_context();
        let level = ContextLevel::dream(); // max_events = 5
        trim_context(&mut ctx, &level);
        let events = ctx["recent_events"].as_array().unwrap();
        assert_eq!(events.len(), 5);
        // First 5 are kept (truncate keeps front)
        assert_eq!(events[0]["id"], "e1");
        assert_eq!(events[4]["id"], "e5");
    }

    #[test]
    fn trim_context_preserves_ready_tasks_and_my_tasks() {
        // Minimal level — most aggressive trimming
        let mut ctx = sample_context();
        let level = ContextLevel::minimal();
        trim_context(&mut ctx, &level);
        assert!(
            ctx.get("ready_tasks").is_some(),
            "ready_tasks must survive trim"
        );
        assert!(ctx.get("my_tasks").is_some(), "my_tasks must survive trim");
        assert_eq!(ctx["ready_tasks"].as_array().unwrap().len(), 1);
        assert_eq!(ctx["my_tasks"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn trim_context_preserves_core_fields() {
        let mut ctx = sample_context();
        let level = ContextLevel::minimal();
        trim_context(&mut ctx, &level);
        assert!(ctx.get("project").is_some());
        assert!(ctx.get("agent").is_some());
        assert!(ctx.get("role").is_some());
        assert!(ctx.get("membership").is_some());
        assert!(ctx.get("decisions").is_some());
        assert!(ctx.get("integrations").is_some());
    }

    #[test]
    fn implement_step_without_feedback_has_generic_header() {
        let root = PathBuf::from("/tmp/test-repo");
        let params = make_params("implement", "", &root);
        let output = build_workflow(&params);
        assert!(
            !output.contains("Review Feedback"),
            "implement step without review feedback should NOT produce Review Feedback heading"
        );
        assert!(
            output.contains("## Your Job: IMPLEMENT"),
            "implement step should have generic IMPLEMENT header"
        );
    }

    // ── decompose mode tests ──────────────────────────────────

    #[test]
    fn decompose_mode_overrides_normal_workflow() {
        let root = PathBuf::from("/tmp/test-repo");
        let mut params = make_params("implement", "", &root);
        params.decompose_mode = true;
        let output = build_workflow(&params);
        assert!(
            output.contains("## Your Job: DECOMPOSE INTO SUBTASKS"),
            "decompose mode should produce DECOMPOSE heading, got: {output}"
        );
        assert!(
            output.contains("Do NOT implement it directly"),
            "decompose mode should instruct agent not to implement directly"
        );
        assert!(
            output.contains("agent-cli create"),
            "decompose mode should contain subtask creation command"
        );
        assert!(
            output.contains("agent-cli depend"),
            "decompose mode should contain dependency wiring command"
        );
    }

    #[test]
    fn decompose_mode_false_uses_normal_workflow() {
        let root = PathBuf::from("/tmp/test-repo");
        let mut params = make_params("implement", "", &root);
        params.decompose_mode = false;
        let output = build_workflow(&params);
        assert!(
            !output.contains("DECOMPOSE INTO SUBTASKS"),
            "decompose_mode=false should NOT produce DECOMPOSE heading"
        );
        assert!(
            output.contains("## Your Job: IMPLEMENT"),
            "decompose_mode=false should produce normal IMPLEMENT heading"
        );
    }

    #[test]
    fn decompose_mode_overrides_step_description() {
        let root = PathBuf::from("/tmp/test-repo");
        let step_json = serde_json::json!({
            "description": "Custom step description that should be overridden"
        });
        let params = WorkflowParams {
            step_name: "implement",
            step_description: "Custom step description that should be overridden",
            step_json: Some(&step_json),
            agent_cli: "/usr/bin/agent-cli",
            task_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            project_id: "11111111-2222-3333-4444-555555555555",
            short_id: "aaaaaaaa-bbb",
            repo_root: &root,
            api_base: "http://localhost:8082",
            playbook_id: "pppppppp-qqqq-rrrr-ssss-tttttttttttt",
            review_feedback: "",
            decompose_mode: true,
            project_json: None,
            work_id: "",
        };
        let output = build_workflow(&params);
        assert!(
            output.contains("DECOMPOSE INTO SUBTASKS"),
            "decompose mode should override step description, got: {output}"
        );
        assert!(
            !output.contains("Custom step description"),
            "decompose mode should NOT contain the original step description"
        );
    }

    #[test]
    fn decompose_mode_includes_work_id_when_present() {
        let root = PathBuf::from("/tmp/test-repo");
        let mut params = make_params("implement", "", &root);
        params.decompose_mode = true;
        params.work_id = "gggggggg-hhhh-iiii-jjjj-kkkkkkkkkkkk";
        let output = build_workflow(&params);
        assert!(
            output.contains(r#""work_id": "gggggggg-hhhh-iiii-jjjj-kkkkkkkkkkkk""#),
            "decompose mode with work item should include work_id in create command, got: {output}"
        );
    }

    #[test]
    fn decompose_mode_omits_work_id_when_absent() {
        let root = PathBuf::from("/tmp/test-repo");
        let mut params = make_params("implement", "", &root);
        params.decompose_mode = true;
        params.work_id = "";
        let output = build_workflow(&params);
        assert!(
            !output.contains("work_id"),
            "decompose mode without work item should NOT include work_id, got: {output}"
        );
    }

    #[test]
    fn decompose_mode_always_includes_parent_id() {
        let root = PathBuf::from("/tmp/test-repo");
        let mut params = make_params("implement", "", &root);
        params.decompose_mode = true;
        let output = build_workflow(&params);
        assert!(
            output.contains(r#""parent_id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee""#),
            "decompose mode should always include parent_id set to current task_id, got: {output}"
        );
    }

    #[test]
    fn decompose_mode_guideline_mentions_parent_id() {
        let root = PathBuf::from("/tmp/test-repo");
        let mut params = make_params("implement", "", &root);
        params.decompose_mode = true;
        let output = build_workflow(&params);
        assert!(
            output.contains("parent_id"),
            "decompose mode guidelines should mention parent_id, got: {output}"
        );
    }

    // ── build_related_context_section tests ──────────────────

    #[test]
    fn related_context_with_all_three_types() {
        let related = serde_json::json!({
            "knowledge": [
                {"title": "Auth pattern", "relevance_score": 0.9, "snippet": "JWT RS256 auth…"}
            ],
            "decisions": [
                {"title": "Use metadata JSON", "relevance_score": 0.7, "snippet": "Store in jsonb…"}
            ],
            "observations": [
                {"title": "Missing test", "relevance_score": 0.5, "snippet": "No coverage for…"}
            ]
        });
        let section = build_related_context_section(&related);
        assert!(section.contains("## Relevant Context for This Task"));
        assert!(section.contains("### Related Knowledge"));
        assert!(section.contains("**Auth pattern** (relevance: 0.9)"));
        assert!(section.contains("### Related Decisions"));
        assert!(section.contains("**Use metadata JSON** (relevance: 0.7)"));
        assert!(section.contains("### Related Observations"));
        assert!(section.contains("**Missing test** (relevance: 0.5)"));
    }

    #[test]
    fn related_context_empty_when_no_items() {
        let related = serde_json::json!({
            "knowledge": [],
            "decisions": [],
            "observations": []
        });
        let section = build_related_context_section(&related);
        assert!(
            section.is_empty(),
            "empty related items should produce empty section, got: {section}"
        );
    }

    #[test]
    fn related_context_empty_when_missing_arrays() {
        let related = serde_json::json!({});
        let section = build_related_context_section(&related);
        assert!(
            section.is_empty(),
            "missing arrays should produce empty section, got: {section}"
        );
    }

    #[test]
    fn related_context_partial_only_knowledge() {
        let related = serde_json::json!({
            "knowledge": [
                {"title": "SQL injection safe", "relevance_score": 1.0, "snippet": "Parameterized queries…"}
            ],
            "decisions": [],
            "observations": []
        });
        let section = build_related_context_section(&related);
        assert!(section.contains("### Related Knowledge"));
        assert!(!section.contains("### Related Decisions"));
        assert!(!section.contains("### Related Observations"));
    }

    #[test]
    fn related_context_handles_null_snippet() {
        let related = serde_json::json!({
            "knowledge": [
                {"title": "No snippet item", "relevance_score": 0.6, "snippet": null}
            ],
            "decisions": [],
            "observations": []
        });
        let section = build_related_context_section(&related);
        assert!(section.contains("**No snippet item** (relevance: 0.6): \n"));
    }

    #[test]
    fn related_context_included_for_implement_context_level() {
        let level = ContextLevel::for_step("implement");
        assert!(
            level.include_knowledge,
            "implement step should have include_knowledge=true for related items"
        );
    }

    #[test]
    fn related_context_included_for_dream_context_level() {
        let level = ContextLevel::for_step("dream");
        assert!(
            level.include_knowledge,
            "dream step should have include_knowledge=true for related items"
        );
    }

    #[test]
    fn related_context_excluded_for_review_context_level() {
        let level = ContextLevel::for_step("review");
        assert!(
            !level.include_knowledge,
            "review step should have include_knowledge=false (no related items)"
        );
    }

    #[test]
    fn work_id_template_variable_substituted() {
        let root = PathBuf::from("/tmp/test-repo");
        let step_json = serde_json::json!({
            "description": "Create subtask with work: {{work_id}}"
        });
        let params = WorkflowParams {
            step_name: "implement",
            step_description: "Create subtask with work: {{work_id}}",
            step_json: Some(&step_json),
            agent_cli: "/usr/bin/agent-cli",
            task_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            project_id: "11111111-2222-3333-4444-555555555555",
            short_id: "aaaaaaaa-bbb",
            repo_root: &root,
            api_base: "http://localhost:8082",
            playbook_id: "pppppppp-qqqq-rrrr-ssss-tttttttttttt",
            review_feedback: "",
            decompose_mode: false,
            project_json: None,
            work_id: "gggggggg-hhhh-iiii-jjjj-kkkkkkkkkkkk",
        };
        let output = build_workflow(&params);
        assert!(
            output.contains("Create subtask with work: gggggggg-hhhh-iiii-jjjj-kkkkkkkkkkkk"),
            "{{{{work_id}}}} template variable should be substituted, got: {output}"
        );
    }
}
