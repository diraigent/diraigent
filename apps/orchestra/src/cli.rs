#![allow(dead_code)]

mod api;
mod constants;
mod context;
mod crypto;
mod diraigent_config;
mod git;
mod prompt;
mod providers;
mod step_profile;
mod task_id;
mod util;
mod worker;

use anyhow::{Context, Result, bail};
use api::ProjectsApi;
use clap::{Parser, Subcommand};
use std::io::Write;
use task_id::TaskId;

#[derive(Parser)]
#[command(name = "agent-cli", about = "CLI for the Projects API")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive setup (register agent, join project, write .env)
    Setup,
    /// Full agent context (role, tasks, knowledge)
    Context { project_id: String },
    /// List tasks ready for work
    Ready {
        project_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Get task details
    Task { task_id: String },
    /// List all project tasks
    Tasks {
        project_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Claim a task
    Claim { task_id: String },
    /// Create a subtask
    Create {
        project_id: String,
        json_body: String,
    },
    /// Add dependency between tasks
    Depend { task_id: String, depends_on: String },
    /// Move task state
    Transition { task_id: String, state: String },
    /// Post progress update
    Progress { task_id: String, message: String },
    /// Post artifact (code, output)
    Artifact { task_id: String, message: String },
    /// Post blocker
    Blocker { task_id: String, message: String },
    /// Post discussion comment
    Comment { task_id: String, message: String },
    /// Post project event
    Event {
        project_id: String,
        json_body: String,
    },
    /// Post observation (insight, risk, smell, improvement)
    Observation {
        project_id: String,
        json_body: String,
    },
    /// Contribute knowledge (pattern, convention, architecture)
    Knowledge {
        project_id: String,
        json_body: String,
    },
    /// Propose decision (with context, rationale, alternatives)
    Decision {
        project_id: String,
        json_body: String,
    },
    /// Agent keep-alive
    Heartbeat,
    /// Update agent capabilities (comma-separated list)
    Caps { capabilities: String },
}

fn load_dotenv() {
    util::load_dotenv();
}

fn api_url() -> String {
    std::env::var("DIRAIGENT_API_URL").unwrap_or_else(|_| "http://localhost:8082/v1".into())
}

fn load_env() -> Result<ProjectsApi> {
    load_dotenv();
    let url = api_url();
    let agent_id =
        std::env::var("AGENT_ID").context("AGENT_ID not set — run `agent-cli setup` first")?;
    Ok(ProjectsApi::new(&url, &agent_id))
}

/// Find the .env file path (orchestra dir or cwd).
fn env_file_path() -> std::path::PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = cwd.clone();
        loop {
            let candidate = dir.join("apps/orchestra/.env");
            if candidate.exists() || candidate.parent().is_some_and(|p| p.is_dir()) {
                return candidate;
            }
            if !dir.pop() {
                break;
            }
        }
    }
    std::path::PathBuf::from(".env")
}

fn prompt_input(prompt: &str, default: &str) -> String {
    print!("{prompt} [{default}]: ");
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    let trimmed = input.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

fn prompt_yes_no(prompt: &str, default_yes: bool) -> bool {
    let hint = if default_yes { "Y/n" } else { "y/N" };
    print!("{prompt} [{hint}]: ");
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    let trimmed = input.trim().to_lowercase();
    if trimmed.is_empty() {
        default_yes
    } else {
        trimmed.starts_with('y')
    }
}

fn prompt_choice(prompt: &str, max: usize, default: usize) -> usize {
    print!("{prompt} [{default}]: ");
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return default;
    }
    trimmed.parse::<usize>().unwrap_or(default).min(max).max(1)
}

/// Update or insert a key=value in a .env file (preserves other keys).
fn upsert_env_file(path: &std::path::Path, key: &str, value: &str) {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let line = format!("export {key}={value}");
    let prefix = format!("export {key}=");

    let mut found = false;
    let mut lines: Vec<String> = content
        .lines()
        .map(|l| {
            if l.starts_with(&prefix) {
                found = true;
                line.clone()
            } else {
                l.to_string()
            }
        })
        .collect();

    if !found {
        lines.push(line);
    }

    std::fs::write(path, lines.join("\n") + "\n").ok();
}

async fn run_setup() -> Result<()> {
    load_dotenv();

    println!("=== Diraigent Agent CLI Setup ===");
    println!();

    // 1. API URL
    let default_api = api_url();
    let api_url_input = prompt_input("API URL", &default_api);
    let api = ProjectsApi::without_agent(&api_url_input);

    // 2. Health check
    print!("Checking API connectivity... ");
    std::io::stdout().flush().ok();
    match api.health_check().await {
        Ok(()) => println!("OK"),
        Err(e) => println!("WARNING: {e}"),
    }

    // 3. Register or reuse agent
    let existing_agent_id = std::env::var("AGENT_ID").ok().filter(|s| !s.is_empty());
    let agent_id;

    let register_new = if let Some(ref existing) = existing_agent_id {
        println!();
        println!("Existing AGENT_ID: {existing}");
        prompt_yes_no("Register new agent?", false)
    } else {
        true
    };

    if register_new {
        let agent_name = prompt_input("Agent name", "");
        if agent_name.is_empty() {
            bail!("agent name is required");
        }

        println!();
        println!("Capability presets:");
        println!(
            "  1) full-stack  (rust, kotlin, typescript, angular, sql, docker, code-review, security)"
        );
        println!("  2) backend     (rust, kotlin, sql, docker)");
        println!("  3) frontend    (typescript, angular, css)");
        println!("  4) rust        (rust, sql, docker)");
        println!("  5) kotlin      (kotlin, sql, docker)");
        println!("  6) custom");

        let caps_choice = prompt_choice("Pick", 6, 1);
        let caps: Vec<&str> = match caps_choice {
            1 => vec![
                "rust",
                "kotlin",
                "typescript",
                "angular",
                "sql",
                "docker",
                "code-review",
                "security",
            ],
            2 => vec!["rust", "kotlin", "sql", "docker"],
            3 => vec!["typescript", "angular", "css"],
            4 => vec!["rust", "sql", "docker"],
            5 => vec!["kotlin", "sql", "docker"],
            6 => {
                let input = prompt_input("Capabilities (comma-separated)", "");
                // Leak the string so we can return &str slices — fine for a one-shot CLI
                let leaked: &'static str = Box::leak(input.into_boxed_str());
                leaked.split(',').map(|s| s.trim()).collect()
            }
            _ => vec!["rust", "kotlin", "typescript", "angular", "sql", "docker"],
        };
        let caps_json: Vec<serde_json::Value> = caps
            .iter()
            .map(|c| serde_json::Value::String(c.to_string()))
            .collect();
        println!("  -> {}", caps.join(", "));

        let model = prompt_input("Model", "claude-opus-4-6");

        print!("Registering agent... ");
        std::io::stdout().flush().ok();
        let result = api
            .register_agent(&serde_json::json!({
                "name": agent_name,
                "capabilities": caps_json,
                "metadata": {"model": model, "runtime": "orchestra"}
            }))
            .await?;
        let id = result["id"].as_str().unwrap_or("").to_string();
        println!("OK -> {id}");
        agent_id = id;
    } else {
        agent_id = existing_agent_id.unwrap_or_default();
    }

    // 5. Join a project
    println!();
    if prompt_yes_no("Join a project?", true) {
        println!();
        println!("Available projects:");
        let projects = api.list_projects().await.unwrap_or_default();
        if projects.is_empty() {
            println!("  (no projects found)");
        } else {
            for (i, p) in projects.iter().enumerate() {
                let name = p["name"]
                    .as_str()
                    .or_else(|| p["slug"].as_str())
                    .unwrap_or("?");
                let id = p["id"].as_str().unwrap_or("");
                let short = TaskId::new(id).to_string();
                println!("  {}) {name}  ({short}...)", i + 1);
            }

            println!();
            let project_choice = prompt_choice("Pick project", projects.len(), 1);
            if let Some(project) = projects.get(project_choice - 1) {
                let project_id = project["id"].as_str().unwrap_or("");
                println!("  -> {project_id}");

                // List roles (global)
                println!();
                println!("Available roles:");
                let roles = api.list_roles().await.unwrap_or_default();
                if roles.is_empty() {
                    println!("  (no roles found)");
                } else {
                    for (j, r) in roles.iter().enumerate() {
                        let rname = r["name"].as_str().unwrap_or("?");
                        let auths = r["authorities"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default();
                        println!("  {}) {rname}  [{auths}]", j + 1);
                    }

                    println!();
                    let role_choice = prompt_choice("Pick role", roles.len(), 1);
                    if let Some(role) = roles.get(role_choice - 1) {
                        let role_id = role["id"].as_str().unwrap_or("");
                        print!("Adding agent membership... ");
                        std::io::stdout().flush().ok();
                        api.add_member(
                            &serde_json::json!({"agent_id": agent_id, "role_id": role_id}),
                        )
                        .await?;
                        println!("OK");
                    }
                }
            }
        }
    }

    // 6. Write .env
    println!();
    let env_path = env_file_path();
    print!("Writing {}... ", env_path.display());
    std::io::stdout().flush().ok();

    // Ensure parent dir exists
    if let Some(parent) = env_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    // Touch file if it doesn't exist
    if !env_path.exists() {
        std::fs::write(&env_path, "").ok();
    }

    upsert_env_file(&env_path, "DIRAIGENT_API_URL", &api_url_input);
    upsert_env_file(
        &env_path,
        "DIRAIGENT_API_TOKEN",
        &std::env::var("DIRAIGENT_API_TOKEN").unwrap_or_default(),
    );
    upsert_env_file(&env_path, "AGENT_ID", &agent_id);
    println!("OK");

    println!();
    println!("Setup complete. Run the orchestra with:");
    println!("  cargo run -p diraigent-orchestra --bin orchestra");

    Ok(())
}

/// Collect changed files from the current git branch and post them to the API.
///
/// Uses the current working directory as the repo root. Works correctly from
/// both the main repo and any git worktree. Errors are logged to stderr but
/// never propagate — changed-file collection is best-effort.
async fn collect_and_post_changed_files(api: &ProjectsApi, task_id: &str) {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("agent-cli: could not determine cwd: {e}");
            return;
        }
    };
    let wm = git::WorktreeManager::new(&cwd);
    match wm.collect_changed_files(task_id) {
        Ok(files) if !files.is_empty() => {
            eprintln!("agent-cli: posting {} changed file(s)", files.len());
            if let Err(e) = api.post_changed_files(task_id, &files).await {
                eprintln!("agent-cli: warning: failed to post changed files: {e}");
            }
        }
        Ok(_) => {
            eprintln!("agent-cli: no changed files found in branch diff");
        }
        Err(e) => {
            eprintln!("agent-cli: warning: could not collect changed files: {e}");
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Commands::Setup = cli.command {
        return run_setup().await;
    }

    let api = load_env()?;

    match cli.command {
        Commands::Setup => unreachable!(),
        Commands::Context { project_id } => {
            let ctx = api.get_context(&project_id).await?;
            println!("{}", serde_json::to_string_pretty(&ctx)?);
        }
        Commands::Ready { project_id, json } => {
            let tasks = api.get_ready_tasks(&project_id).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&tasks)?);
            } else {
                print_ready_tasks(&tasks);
            }
        }
        Commands::Task { task_id } => {
            let task = api.get_task(&task_id).await?;
            println!("{}", serde_json::to_string_pretty(&task)?);
        }
        Commands::Tasks { project_id, json } => {
            let tasks = api.get_all_tasks(&project_id).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&tasks)?);
            } else {
                print_task_table(&tasks, &["number", "id", "state", "title"]);
            }
        }
        Commands::Claim { task_id } => {
            let result = api.claim_task(&task_id).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Create {
            project_id,
            json_body,
        } => {
            let body: serde_json::Value =
                serde_json::from_str(&json_body).context("invalid JSON body")?;
            let result = api.create_task(&project_id, &body).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Depend {
            task_id,
            depends_on,
        } => {
            let result = api.add_dependency(&task_id, &depends_on).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Transition { task_id, state } => {
            // When transitioning to done, auto-collect changed files from the current
            // git branch and post them. This ensures changed files are recorded even
            // when the agent runs directly (without an orchestra worker).
            if state == "done" {
                collect_and_post_changed_files(&api, &task_id).await;
            }
            let result = api.transition_task(&task_id, &state).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Progress { task_id, message } => {
            let result = api.post_task_update(&task_id, "progress", &message).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Artifact { task_id, message } => {
            let result = api.post_task_update(&task_id, "artifact", &message).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Blocker { task_id, message } => {
            let result = api.post_task_update(&task_id, "blocker", &message).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Comment { task_id, message } => {
            let result = api.post_comment(&task_id, &message).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Event {
            project_id,
            json_body,
        } => {
            let body: serde_json::Value =
                serde_json::from_str(&json_body).context("invalid JSON body")?;
            let result = api.post_event(&project_id, &body).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Observation {
            project_id,
            json_body,
        } => {
            let body: serde_json::Value =
                serde_json::from_str(&json_body).context("invalid JSON body")?;
            let result = api.post_observation(&project_id, &body).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Knowledge {
            project_id,
            json_body,
        } => {
            let body: serde_json::Value =
                serde_json::from_str(&json_body).context("invalid JSON body")?;
            let result = api.post_knowledge(&project_id, &body).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Decision {
            project_id,
            json_body,
        } => {
            let body: serde_json::Value =
                serde_json::from_str(&json_body).context("invalid JSON body")?;
            let result = api.post_decision(&project_id, &body).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Heartbeat => {
            let result = api.heartbeat().await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Caps { capabilities } => {
            let agent_id = std::env::var("AGENT_ID")
                .context("AGENT_ID not set — run `agent-cli setup` first")?;
            let caps: Vec<serde_json::Value> = capabilities
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| serde_json::Value::String(s.to_string()))
                .collect();
            let result = api
                .update_agent(&agent_id, &serde_json::json!({"capabilities": caps}))
                .await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}

/// Print ready-tasks with optional score breakdown.
/// Falls back to priority-only display when scores are absent in the API response.
fn print_ready_tasks(tasks: &[serde_json::Value]) {
    let has_scores = tasks
        .iter()
        .any(|t| t.get("score").and_then(|s| s.as_f64()).is_some());

    if !has_scores {
        // Fallback: no scores available — show priority-only display
        print_task_table(tasks, &["number", "id", "urgent", "title"]);
        return;
    }

    // Header with score column
    println!(
        "{:<10}{:<14}{:<4}{:<10}TITLE",
        "NUMBER", "ID", "\u{26a1}", "SCORE"
    );
    println!(
        "{:<10}{:<14}{:<4}{:<10}--------------------",
        "--------", "------------", "--", "--------"
    );

    for task in tasks {
        let number = task["number"]
            .as_i64()
            .map(|n| n.to_string())
            .unwrap_or_default();
        let id = task["id"]
            .as_str()
            .map(|s| TaskId::new(s).to_string())
            .unwrap_or_default();
        let urgent = if task["urgent"].as_bool().unwrap_or(false) {
            "\u{26a1}"
        } else {
            ""
        };
        let score = task
            .get("score")
            .and_then(|s| s.as_f64())
            .map(|s| format!("{:.1}", s))
            .unwrap_or_else(|| "-".to_string());
        let title = task["title"].as_str().unwrap_or("");

        println!(
            "{:<10}{:<14}{:<4}{:<10}{}",
            number, id, urgent, score, title
        );

        // Print score breakdown if components are available
        if let Some(components) = task.get("score_components").and_then(|c| c.as_object()) {
            let total = task.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
            // Render known component keys in a stable order, then append any extras
            let known_keys = ["age", "priority", "deps", "goal"];
            let mut parts: Vec<String> = Vec::new();
            for key in &known_keys {
                if let Some(val) = components.get(*key).and_then(|v| v.as_f64()) {
                    parts.push(format!("{}: {:.1}", key, val));
                }
            }
            // Append any extra components not in the known list
            for (key, val) in components {
                if !known_keys.contains(&key.as_str())
                    && let Some(v) = val.as_f64()
                {
                    parts.push(format!("{}: {:.1}", key, v));
                }
            }
            if !parts.is_empty() {
                println!("{:<10}score: {:.1} ({})", "", total, parts.join(", "));
            }
        }
    }
}

fn print_task_table(tasks: &[serde_json::Value], columns: &[&str]) {
    let headers: Vec<String> = columns.iter().map(|c| c.to_uppercase()).collect();
    println!(
        "{}",
        headers
            .iter()
            .map(|h| format!("{:<14}", h))
            .collect::<Vec<_>>()
            .join("")
    );
    println!(
        "{}",
        columns
            .iter()
            .map(|_| format!("{:<14}", "------------"))
            .collect::<Vec<_>>()
            .join("")
    );

    for task in tasks {
        let row: Vec<String> = columns
            .iter()
            .map(|col| {
                let val = &task[*col];
                match val {
                    serde_json::Value::String(s) => {
                        if *col == "id" {
                            TaskId::new(s.as_str()).to_string()
                        } else {
                            s.clone()
                        }
                    }
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => {
                        if *col == "urgent" && *b {
                            "⚡".to_string()
                        } else if *col == "urgent" {
                            String::new()
                        } else {
                            b.to_string()
                        }
                    }
                    _ => val.to_string(),
                }
            })
            .collect();
        println!(
            "{}",
            row.iter()
                .map(|v| format!("{:<14}", v))
                .collect::<Vec<_>>()
                .join("")
        );
    }
}
