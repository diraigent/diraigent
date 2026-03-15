use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Parser as ClapParser;
use ignore::WalkBuilder;
use regex::Regex;
use serde::Serialize;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// Static codebase analyzer — extracts per-file metadata for Rust, TypeScript,
/// and SQL source files.
#[derive(ClapParser)]
#[command(name = "diraigent-analyzer")]
struct Cli {
    /// Root directory to scan
    #[arg(default_value = ".")]
    root: PathBuf,

    /// Output file path (writes to stdout if omitted)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Pretty-print JSON output
    #[arg(long)]
    pretty: bool,
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct Manifest {
    stats: Stats,
    files: Vec<FileEntry>,
}

#[derive(Debug, Serialize)]
struct Stats {
    total_files: usize,
    by_language: HashMap<String, usize>,
    elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
struct FileEntry {
    path: String,
    language: String,
    imports: Vec<String>,
    exports: Vec<Symbol>,
    routes: Vec<Route>,
    #[serde(skip_serializing_if = "Option::is_none")]
    docstring: Option<String>,
}

#[derive(Debug, Serialize)]
struct Symbol {
    name: String,
    kind: String,
}

#[derive(Debug, Serialize)]
struct Route {
    method: String,
    path: String,
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();
    let start = Instant::now();
    let root = cli.root.canonicalize().unwrap_or_else(|_| cli.root.clone());

    let mut entries = Vec::new();

    let walker = WalkBuilder::new(&root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !matches!(name.as_ref(), "target" | "node_modules" | "dist" | ".git")
                && !name.starts_with("dist-")
        })
        .build();

    for result in walker {
        let Ok(entry) = result else { continue };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(language) = detect_language(path) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };

        let rel_path = path.strip_prefix(&root).unwrap_or(path);

        let (imports, exports, routes, docstring) = match language {
            "rust" => parse_rust(&content),
            "typescript" => parse_typescript(&content),
            "sql" => parse_sql(&content),
            _ => continue,
        };

        entries.push(FileEntry {
            path: rel_path.to_string_lossy().into_owned(),
            language: language.to_string(),
            imports,
            exports,
            routes,
            docstring,
        });
    }

    // Deterministic output order
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    let elapsed = start.elapsed();
    let mut by_language: HashMap<String, usize> = HashMap::new();
    for e in &entries {
        *by_language.entry(e.language.clone()).or_default() += 1;
    }

    let manifest = Manifest {
        stats: Stats {
            total_files: entries.len(),
            by_language,
            elapsed_ms: elapsed.as_millis(),
        },
        files: entries,
    };

    let json = if cli.pretty {
        serde_json::to_string_pretty(&manifest)
    } else {
        serde_json::to_string(&manifest)
    }
    .expect("JSON serialization failed");

    match cli.output {
        Some(ref out) => {
            std::fs::write(out, &json).expect("failed to write output file");
            eprintln!(
                "Wrote {} files to {} in {}ms",
                manifest.stats.total_files,
                out.display(),
                manifest.stats.elapsed_ms
            );
        }
        None => println!("{json}"),
    }
}

// ---------------------------------------------------------------------------
// Language detection
// ---------------------------------------------------------------------------

fn detect_language(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "rs" => Some("rust"),
        "ts" | "tsx" => Some("typescript"),
        "sql" => Some("sql"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Rust parser
// ---------------------------------------------------------------------------

fn parse_rust(content: &str) -> (Vec<String>, Vec<Symbol>, Vec<Route>, Option<String>) {
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut routes = Vec::new();

    // --- Imports: `use crate::foo;` ---
    let re = Regex::new(r"(?m)^\s*use\s+([^;]+);").unwrap();
    for cap in re.captures_iter(content) {
        imports.push(cap[1].trim().to_string());
    }

    // --- Exported symbols (pub items) ---
    for (pattern, kind) in [
        (r"(?m)^\s*pub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+(\w+)", "fn"),
        (r"(?m)^\s*pub(?:\([^)]*\))?\s+struct\s+(\w+)", "struct"),
        (r"(?m)^\s*pub(?:\([^)]*\))?\s+enum\s+(\w+)", "enum"),
        (r"(?m)^\s*pub(?:\([^)]*\))?\s+trait\s+(\w+)", "trait"),
        (r"(?m)^\s*pub(?:\([^)]*\))?\s+type\s+(\w+)", "type"),
        (r"(?m)^\s*pub(?:\([^)]*\))?\s+mod\s+(\w+)", "mod"),
        (r"(?m)^\s*pub(?:\([^)]*\))?\s+const\s+(\w+)", "const"),
        (r"(?m)^\s*pub(?:\([^)]*\))?\s+static\s+(\w+)", "static"),
    ] {
        let re = Regex::new(pattern).unwrap();
        for cap in re.captures_iter(content) {
            exports.push(Symbol {
                name: cap[1].to_string(),
                kind: kind.to_string(),
            });
        }
    }

    // --- Route annotations ---
    // Axum: .route("/path", get(handler).post(handler2))
    let route_re = Regex::new(r#"\.route\(\s*"([^"]+)""#).unwrap();
    let method_re = Regex::new(r"\b(get|post|put|delete|patch|head|options)\s*\(").unwrap();

    for route_cap in route_re.captures_iter(content) {
        let route_path = route_cap[1].to_string();
        let start = route_cap.get(0).unwrap().end();
        let rest = &content[start..];
        // Scan until the next .route( / .nest( or up to 500 chars
        let end = rest
            .find(".route(")
            .or_else(|| rest.find(".nest("))
            .unwrap_or(rest.len().min(500));
        let segment = &rest[..end];

        let methods: Vec<String> = method_re
            .captures_iter(segment)
            .map(|m| m[1].to_uppercase())
            .collect();

        if methods.is_empty() {
            routes.push(Route {
                method: "ANY".to_string(),
                path: route_path,
            });
        } else {
            for method in methods {
                routes.push(Route {
                    method,
                    path: route_path.clone(),
                });
            }
        }
    }

    // .nest("/prefix", sub_router)
    let nest_re = Regex::new(r#"\.nest\(\s*"([^"]+)""#).unwrap();
    for cap in nest_re.captures_iter(content) {
        routes.push(Route {
            method: "NEST".to_string(),
            path: cap[1].to_string(),
        });
    }

    // --- Docstring (//! module-level docs) ---
    let docstring = extract_rust_module_doc(content);

    (imports, exports, routes, docstring)
}

fn extract_rust_module_doc(content: &str) -> Option<String> {
    let mut lines = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("//!") {
            lines.push(t.trim_start_matches("//!").trim().to_string());
        } else if t.is_empty() || t.starts_with("//") || t.starts_with('#') {
            continue;
        } else {
            break;
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

// ---------------------------------------------------------------------------
// TypeScript parser
// ---------------------------------------------------------------------------

fn parse_typescript(content: &str) -> (Vec<String>, Vec<Symbol>, Vec<Route>, Option<String>) {
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut routes = Vec::new();

    // --- Imports ---
    // import { X } from 'mod';  /  import * as X from 'mod';  /  import X from 'mod';
    let re = Regex::new(
        r#"(?m)^\s*import\s+(?:type\s+)?(?:\{[^}]*\}|\*\s+as\s+\w+|\w+)\s+from\s+['"]([^'"]+)['"]"#,
    )
    .unwrap();
    for cap in re.captures_iter(content) {
        imports.push(cap[1].to_string());
    }
    // Side-effect: import 'mod';
    let re = Regex::new(r#"(?m)^\s*import\s+['"]([^'"]+)['"]"#).unwrap();
    for cap in re.captures_iter(content) {
        imports.push(cap[1].to_string());
    }

    // --- Exported symbols ---
    for (pattern, kind) in [
        (
            r"(?m)^\s*export\s+(?:async\s+)?function\s+(\w+)",
            "function",
        ),
        (r"(?m)^\s*export\s+(?:abstract\s+)?class\s+(\w+)", "class"),
        (r"(?m)^\s*export\s+interface\s+(\w+)", "interface"),
        (r"(?m)^\s*export\s+type\s+(\w+)", "type"),
        (r"(?m)^\s*export\s+const\s+(\w+)", "const"),
        (r"(?m)^\s*export\s+let\s+(\w+)", "let"),
        (r"(?m)^\s*export\s+enum\s+(\w+)", "enum"),
    ] {
        let re = Regex::new(pattern).unwrap();
        for cap in re.captures_iter(content) {
            exports.push(Symbol {
                name: cap[1].to_string(),
                kind: kind.to_string(),
            });
        }
    }

    // Default exports
    let re = Regex::new(r"(?m)^\s*export\s+default\s+(?:class|function|abstract\s+class)?\s*(\w*)")
        .unwrap();
    for cap in re.captures_iter(content) {
        let name = if cap[1].is_empty() {
            "default"
        } else {
            &cap[1]
        };
        exports.push(Symbol {
            name: name.to_string(),
            kind: "default".to_string(),
        });
    }

    // --- Routes ---
    // Angular route definitions: { path: 'foo', ... }
    let re = Regex::new(r#"path:\s*['"]([^'"]+)['"]"#).unwrap();
    for cap in re.captures_iter(content) {
        routes.push(Route {
            method: "ROUTE".to_string(),
            path: cap[1].to_string(),
        });
    }

    // Angular decorators
    let re = Regex::new(r"@(Component|Injectable|NgModule|Pipe|Directive)\s*\(").unwrap();
    for cap in re.captures_iter(content) {
        routes.push(Route {
            method: "DECORATOR".to_string(),
            path: cap[1].to_string(),
        });
    }

    // --- Docstring ---
    let docstring = extract_jsdoc_header(content);

    (imports, exports, routes, docstring)
}

fn extract_jsdoc_header(content: &str) -> Option<String> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("/**") {
        return None;
    }
    let end = trimmed.find("*/")?;
    let doc = &trimmed[3..end];
    let lines: Vec<&str> = doc
        .lines()
        .map(|l| l.trim().trim_start_matches('*').trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

// ---------------------------------------------------------------------------
// SQL parser
// ---------------------------------------------------------------------------

fn parse_sql(content: &str) -> (Vec<String>, Vec<Symbol>, Vec<Route>, Option<String>) {
    let mut imports = Vec::new();
    let mut exports = Vec::new();

    // Extensions used
    let re =
        Regex::new(r#"(?i)CREATE\s+EXTENSION\s+(?:IF\s+NOT\s+EXISTS\s+)?["']?(\w+)["']?"#).unwrap();
    for cap in re.captures_iter(content) {
        imports.push(format!("extension:{}", &cap[1]));
    }

    // Schema references
    let re = Regex::new(r"(?i)SET\s+search_path\s+(?:TO|=)\s+(\w+)").unwrap();
    for cap in re.captures_iter(content) {
        imports.push(format!("schema:{}", &cap[1]));
    }

    // DDL exports
    for (pattern, kind) in [
        (
            r"(?i)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(?:\w+\.)?(\w+)",
            "table",
        ),
        (
            r"(?i)CREATE\s+(?:UNIQUE\s+)?INDEX\s+(?:IF\s+NOT\s+EXISTS\s+)?(?:CONCURRENTLY\s+)?(\w+)",
            "index",
        ),
        (
            r"(?i)CREATE\s+(?:OR\s+REPLACE\s+)?FUNCTION\s+(?:\w+\.)?(\w+)",
            "function",
        ),
        (
            r"(?i)CREATE\s+(?:OR\s+REPLACE\s+)?TRIGGER\s+(\w+)",
            "trigger",
        ),
        (r"(?i)CREATE\s+TYPE\s+(?:\w+\.)?(\w+)", "type"),
        (
            r"(?i)CREATE\s+(?:OR\s+REPLACE\s+)?(?:MATERIALIZED\s+)?VIEW\s+(?:\w+\.)?(\w+)",
            "view",
        ),
    ] {
        let re = Regex::new(pattern).unwrap();
        for cap in re.captures_iter(content) {
            exports.push(Symbol {
                name: cap[1].to_string(),
                kind: kind.to_string(),
            });
        }
    }

    // ALTER TABLE ... ADD COLUMN
    let re = Regex::new(
        r"(?i)ALTER\s+TABLE\s+(?:IF\s+EXISTS\s+)?(?:\w+\.)?(\w+)\s+ADD\s+(?:COLUMN\s+)?(?:IF\s+NOT\s+EXISTS\s+)?(\w+)",
    )
    .unwrap();
    for cap in re.captures_iter(content) {
        exports.push(Symbol {
            name: format!("{}.{}", &cap[1], &cap[2]),
            kind: "column".to_string(),
        });
    }

    // --- Docstring ---
    let docstring = extract_sql_header_comment(content);

    (imports, exports, Vec::new(), docstring)
}

fn extract_sql_header_comment(content: &str) -> Option<String> {
    let mut lines = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("--") {
            lines.push(t.trim_start_matches("--").trim().to_string());
        } else if t.is_empty() {
            continue;
        } else {
            break;
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}
