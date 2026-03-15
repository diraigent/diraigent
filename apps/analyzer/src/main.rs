mod api_surface;
mod scan;
mod summarize;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// Static codebase analyzer — extracts per-file metadata, API surface maps,
/// and AI-powered summaries for Rust, TypeScript, and SQL source files.
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
        Commands::ApiSurface {
            root,
            output,
            format,
            pretty,
        } => api_surface::run(root, output, &format, pretty),
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
