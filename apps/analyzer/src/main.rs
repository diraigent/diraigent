mod api_surface;
mod graph;
mod scan;
mod summarize;
mod sync;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// Static codebase analyzer — extracts per-file metadata, API surface maps,
/// dependency graphs, and AI-powered summaries for Rust, TypeScript, and SQL
/// source files.
#[derive(Parser)]
#[command(name = "diraigent-analyzer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan codebase and produce a per-file metadata manifest (imports, exports, routes)
    Scan {
        /// Root directory to scan
        #[arg(default_value = ".")]
        root: PathBuf,

        /// Output file path (writes to stdout if omitted)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Pretty-print JSON output
        #[arg(long)]
        pretty: bool,
    },

    /// Extract API surface: HTTP routes, WebSocket messages, traits, interfaces
    ApiSurface {
        /// Root directory to scan
        #[arg(default_value = ".")]
        root: PathBuf,

        /// Output file path (writes to stdout if omitted)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format: json or markdown
        #[arg(long, default_value = "json")]
        format: String,

        /// Pretty-print JSON output
        #[arg(long)]
        pretty: bool,
    },

    /// Sync analyzer outputs to the Diraigent knowledge store
    Sync {
        /// Path to the scan manifest JSON (from `scan`)
        #[arg(short, long)]
        manifest: PathBuf,

        /// Path to the summary manifest JSON (from `summarize`); optional
        #[arg(short = 's', long)]
        summaries: Option<PathBuf>,

        /// Path to the API surface JSON (from `api-surface`)
        #[arg(short = 'a', long)]
        api_surface: PathBuf,

        /// Diraigent project ID
        #[arg(long, env = "PROJECT_ID")]
        project_id: String,

        /// Diraigent API URL
        #[arg(long, env = "DIRAIGENT_API_URL")]
        api_url: String,

        /// Diraigent API token (agent key or JWT)
        #[arg(long, env = "DIRAIGENT_API_TOKEN")]
        api_token: String,

        /// Agent ID for the X-Agent-Id header
        #[arg(long, env = "AGENT_ID")]
        agent_id: Option<String>,

        /// Cache file path for tracking synced hashes
        #[arg(short, long, default_value = ".analyzer-sync-cache.json")]
        cache: PathBuf,

        /// Dry run — compute what would change without calling the API
        #[arg(long)]
        dry_run: bool,
    },

    /// Build a dependency graph from a scan manifest JSON
    Graph {
        /// Path to a manifest JSON file (produced by `scan`)
        manifest: PathBuf,

        /// Output file path (writes to stdout if omitted)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Pretty-print JSON output
        #[arg(long)]
        pretty: bool,

        /// Output Mermaid diagram instead of JSON
        #[arg(long)]
        mermaid: bool,

        /// Output Graphviz DOT format instead of JSON
        #[arg(long)]
        dot: bool,
    },

    /// Generate AI-powered summaries for each module in the manifest
    Summarize {
        /// Path to the analysis manifest JSON (from `scan`)
        #[arg(short, long)]
        manifest: PathBuf,

        /// Root directory of the codebase (for reading source files)
        #[arg(short, long, default_value = ".")]
        root: PathBuf,

        /// Cache file path for storing content hashes and summaries
        #[arg(short, long, default_value = ".analyzer-cache.json")]
        cache: PathBuf,

        /// Output file path (writes to stdout if omitted)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Anthropic API key (defaults to ANTHROPIC_API_KEY env var)
        #[arg(long, env = "ANTHROPIC_API_KEY")]
        api_key: String,

        /// Claude model to use
        #[arg(long, default_value = "claude-sonnet-4-20250514")]
        model: String,

        /// Maximum budget in USD (stops processing when exceeded)
        #[arg(long, default_value = "5.0")]
        budget: f64,

        /// Max concurrent API requests
        #[arg(long, default_value = "4")]
        concurrency: usize,

        /// Minimum lines of code to summarise a file (files with fewer LOC are skipped)
        #[arg(long, default_value = "10")]
        min_loc: usize,

        /// Pretty-print JSON output
        #[arg(long)]
        pretty: bool,
    },
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            root,
            output,
            pretty,
        } => scan::run(root, output, pretty),
        Commands::Sync {
            manifest,
            summaries,
            api_surface,
            project_id,
            api_url,
            api_token,
            agent_id,
            cache,
            dry_run,
        } => {
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "info".into()),
                )
                .with_target(false)
                .init();

            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            let config = sync::SyncConfig {
                manifest_path: manifest,
                summaries_path: summaries,
                api_surface_path: api_surface,
                cache_path: cache,
                api_url,
                api_token,
                project_id,
                agent_id,
                dry_run,
            };
            if let Err(e) = rt.block_on(sync::run(config)) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::ApiSurface {
            root,
            output,
            format,
            pretty,
        } => api_surface::run(root, output, &format, pretty),
        Commands::Graph {
            manifest,
            output,
            pretty,
            mermaid,
            dot,
        } => graph::run(manifest, output, pretty, mermaid, dot),
        Commands::Summarize {
            manifest,
            root,
            cache,
            output,
            api_key,
            model,
            budget,
            concurrency,
            min_loc,
            pretty,
        } => {
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "info".into()),
                )
                .with_target(false)
                .init();

            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            let root = root.canonicalize().unwrap_or(root);
            let config = summarize::SummarizeConfig {
                manifest_path: manifest,
                cache_path: cache,
                output_path: output,
                root_dir: root,
                api_key,
                model,
                budget_usd: budget,
                concurrency,
                min_loc,
                pretty,
            };
            if let Err(e) = rt.block_on(summarize::run(config)) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}
