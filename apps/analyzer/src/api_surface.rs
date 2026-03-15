use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use ignore::WalkBuilder;
use regex::Regex;
use serde::Serialize;

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ApiSurface {
    pub routes: Vec<HttpRoute>,
    pub ws_messages: Vec<WsMessageDef>,
    pub traits: Vec<TraitDef>,
    pub interfaces: Vec<InterfaceDef>,
    pub stats: SurfaceStats,
}

#[derive(Debug, Serialize)]
pub struct SurfaceStats {
    pub total_routes: usize,
    pub total_ws_messages: usize,
    pub total_traits: usize,
    pub total_interfaces: usize,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct HttpRoute {
    pub method: String,
    pub path: String,
    pub handler: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WsMessageDef {
    pub type_tag: String,
    pub variant: String,
    pub direction: String,
    pub fields: Vec<String>,
    pub file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraitDef {
    pub name: String,
    pub file: String,
    pub methods: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceDef {
    pub name: String,
    pub file: String,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(root: PathBuf, output: Option<PathBuf>, format: &str, pretty: bool) {
    let start = Instant::now();
    let root = root.canonicalize().unwrap_or_else(|_| root.clone());

    let mut all_routes: Vec<HttpRoute> = Vec::new();
    let mut all_ws: Vec<WsMessageDef> = Vec::new();
    let mut all_traits: Vec<TraitDef> = Vec::new();
    let mut all_interfaces: Vec<InterfaceDef> = Vec::new();
    let mut nest_prefixes: HashMap<String, String> = HashMap::new();

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

        let ext = path.extension().and_then(|e| e.to_str());
        let is_rust = ext == Some("rs");
        let is_typescript = matches!(ext, Some("ts") | Some("tsx"));

        if !is_rust && !is_typescript {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let rel_path = path.strip_prefix(&root).unwrap_or(path);
        let rel_str = rel_path.to_string_lossy().to_string();

        if is_rust {
            // Extract HTTP routes with handler names
            let routes = extract_http_routes(&content, &rel_str);
            all_routes.extend(routes);

            // Extract nest prefixes from main.rs-like files
            let nests = extract_nest_prefixes(&content);
            for (module, prefix) in nests {
                nest_prefixes.insert(module, prefix);
            }

            // Extract WebSocket protocol messages
            let ws = extract_ws_messages(&content, &rel_str);
            all_ws.extend(ws);

            // Extract trait definitions
            let traits = extract_traits(&content, &rel_str);
            all_traits.extend(traits);
        }

        if is_typescript {
            // Extract TypeScript interfaces
            let interfaces = extract_interfaces(&content, &rel_str);
            all_interfaces.extend(interfaces);
        }
    }

    // Apply nest prefixes to routes (e.g., /v1 prefix for routes under routes/)
    resolve_route_prefixes(&mut all_routes, &nest_prefixes);

    // Sort for deterministic output
    all_routes.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.method.cmp(&b.method)));
    all_ws.sort_by(|a, b| a.type_tag.cmp(&b.type_tag));
    all_traits.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.name.cmp(&b.name)));
    all_interfaces.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.name.cmp(&b.name)));

    let surface = ApiSurface {
        stats: SurfaceStats {
            total_routes: all_routes.len(),
            total_ws_messages: all_ws.len(),
            total_traits: all_traits.len(),
            total_interfaces: all_interfaces.len(),
            elapsed_ms: start.elapsed().as_millis(),
        },
        routes: all_routes,
        ws_messages: all_ws,
        traits: all_traits,
        interfaces: all_interfaces,
    };

    let output_str = match format {
        "json" => format_json(&surface, pretty),
        "markdown" | "md" => format_markdown(&surface),
        _ => format_json(&surface, pretty),
    };

    match output {
        Some(ref out) => {
            std::fs::write(out, &output_str).expect("failed to write output file");
            eprintln!(
                "API surface: {} routes, {} WS messages, {} traits, {} interfaces ({}ms)",
                surface.stats.total_routes,
                surface.stats.total_ws_messages,
                surface.stats.total_traits,
                surface.stats.total_interfaces,
                surface.stats.elapsed_ms,
            );
        }
        None => println!("{output_str}"),
    }
}

// ---------------------------------------------------------------------------
// HTTP Route extraction
// ---------------------------------------------------------------------------

fn extract_http_routes(content: &str, file: &str) -> Vec<HttpRoute> {
    let route_re = Regex::new(r#"\.route\(\s*"([^"]+)""#).unwrap();
    let handler_re =
        Regex::new(r"\b(get|post|put|delete|patch|head|options)\s*\(\s*(\w+)").unwrap();
    let method_re = Regex::new(r"\b(get|post|put|delete|patch|head|options)\s*\(").unwrap();

    let mut routes = Vec::new();

    for route_cap in route_re.captures_iter(content) {
        // Skip matches inside comments or string literals (e.g. code examples)
        let match_pos = route_cap.get(0).unwrap().start();
        if is_in_comment_or_string(content, match_pos) {
            continue;
        }

        let path = route_cap[1].to_string();
        let start = route_cap.get(0).unwrap().end();
        let rest = &content[start..];

        // Scan forward until the next route/nest/merge/layer/with_state call
        let end = [".route(", ".nest(", ".merge(", ".layer(", ".with_state("]
            .iter()
            .filter_map(|pat| rest.find(pat))
            .min()
            .unwrap_or(rest.len().min(500));
        let segment = &rest[..end];

        let named: Vec<(String, String)> = handler_re
            .captures_iter(segment)
            .map(|c| (c[1].to_uppercase(), c[2].to_string()))
            .collect();

        if named.is_empty() {
            // Check for method calls with closures
            let methods: Vec<String> = method_re
                .captures_iter(segment)
                .map(|c| c[1].to_uppercase())
                .collect();

            if methods.is_empty() {
                routes.push(HttpRoute {
                    method: "ANY".to_string(),
                    path,
                    handler: "<unknown>".to_string(),
                    file: file.to_string(),
                });
            } else {
                for method in methods {
                    routes.push(HttpRoute {
                        method,
                        path: path.clone(),
                        handler: "<closure>".to_string(),
                        file: file.to_string(),
                    });
                }
            }
        } else {
            for (method, handler) in named {
                routes.push(HttpRoute {
                    method,
                    path: path.clone(),
                    handler,
                    file: file.to_string(),
                });
            }
        }
    }

    routes
}

// ---------------------------------------------------------------------------
// Nest prefix extraction & resolution
// ---------------------------------------------------------------------------

fn extract_nest_prefixes(content: &str) -> Vec<(String, String)> {
    let nest_re = Regex::new(r#"\.nest\(\s*"([^"]+)"\s*,\s*(\w+)::"#).unwrap();
    nest_re
        .captures_iter(content)
        .map(|c| (c[2].to_string(), c[1].to_string()))
        .collect()
}

#[allow(clippy::collapsible_if)]
fn resolve_route_prefixes(routes: &mut [HttpRoute], nests: &HashMap<String, String>) {
    for route in routes.iter_mut() {
        // Routes defined in a routes/ directory get the nest prefix
        if route.file.contains("/routes/") || route.file.starts_with("routes/") {
            if let Some(prefix) = nests.get("routes") {
                if !route.path.starts_with(prefix) {
                    route.path = format!("{}{}", prefix, route.path);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// WebSocket message extraction
// ---------------------------------------------------------------------------

fn extract_ws_messages(content: &str, file: &str) -> Vec<WsMessageDef> {
    // Only process files with serde-tagged enums
    if !content.contains("#[serde(tag =") && !content.contains("#[serde(tag=") {
        return Vec::new();
    }

    let enum_re = Regex::new(r"pub\s+enum\s+(\w+)").unwrap();
    let mut all_messages = Vec::new();

    for cap in enum_re.captures_iter(content) {
        let enum_pos = cap.get(0).unwrap().start();

        // Check for #[serde(tag = "...")] in the 300 chars before the enum
        let prefix_start = enum_pos.saturating_sub(300);
        let prefix = &content[prefix_start..enum_pos];
        if !prefix.contains("#[serde(tag =") && !prefix.contains("#[serde(tag=") {
            continue;
        }

        // Extract the enum body using brace matching
        let Some((body_start, body_end)) = extract_brace_block(content, enum_pos) else {
            continue;
        };
        let body = &content[body_start..body_end];

        let messages = parse_enum_variants(body, file);
        all_messages.extend(messages);
    }

    all_messages
}

#[allow(clippy::collapsible_if)]
fn parse_enum_variants(body: &str, file: &str) -> Vec<WsMessageDef> {
    let mut messages = Vec::new();
    let mut direction = "unknown".to_string();
    let mut pending_rename: Option<String> = None;

    let rename_re = Regex::new(r#"#\[serde\(rename\s*=\s*"([^"]+)"\)\]"#).unwrap();
    let field_re = Regex::new(r"^\s*(\w+)\s*:").unwrap();
    let variant_struct_re = Regex::new(r"^(\w+)\s*\{").unwrap();
    let variant_unit_re = Regex::new(r"^([A-Z]\w*)\s*,?\s*$").unwrap();

    let lines: Vec<&str> = body.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Skip empty lines
        if line.is_empty() {
            i += 1;
            continue;
        }

        // Direction comments
        if line.starts_with("//") {
            let upper = line.to_uppercase();
            if upper.contains("API") && upper.contains("ORCHESTRA") {
                let api_pos = upper.find("API").unwrap_or(0);
                let orch_pos = upper.find("ORCHESTRA").unwrap_or(0);
                if api_pos < orch_pos {
                    // API -> Orchestra = server sends to client
                    direction = "server\u{2192}client".to_string();
                } else {
                    // Orchestra -> API = client sends to server
                    direction = "client\u{2192}server".to_string();
                }
            }
            i += 1;
            continue;
        }

        // Serde rename attribute
        if let Some(cap) = rename_re.captures(line) {
            pending_rename = Some(cap[1].to_string());
            i += 1;
            continue;
        }

        // Skip other attribute lines
        if line.starts_with("#[") {
            i += 1;
            continue;
        }

        // Struct variant: VariantName { field: Type, ... }
        if let Some(cap) = variant_struct_re.captures(line) {
            let variant = cap[1].to_string();

            // Skip if it looks like a keyword
            if is_rust_keyword(&variant) {
                i += 1;
                continue;
            }

            let type_tag = pending_rename
                .take()
                .unwrap_or_else(|| variant.to_lowercase());

            let mut fields = Vec::new();
            let mut depth: i32 =
                line.matches('{').count() as i32 - line.matches('}').count() as i32;

            if depth <= 0 {
                // Single-line variant: VariantName { field: Type },
                let after_brace = line.split('{').nth(1).unwrap_or("");
                for fc in field_re.captures_iter(after_brace) {
                    let name = fc[1].to_string();
                    if !is_serde_attr(&name) {
                        fields.push(name);
                    }
                }
            } else {
                i += 1;
                while i < lines.len() && depth > 0 {
                    let fl = lines[i].trim();
                    depth += fl.matches('{').count() as i32;
                    depth -= fl.matches('}').count() as i32;

                    if (depth > 0 || !fl.starts_with('}')) && !fl.trim_start().starts_with('#') {
                        if let Some(fc) = field_re.captures(fl) {
                            let name = fc[1].to_string();
                            if !is_serde_attr(&name) {
                                fields.push(name);
                            }
                        }
                    }
                    i += 1;
                }
            }

            messages.push(WsMessageDef {
                type_tag,
                variant,
                direction: direction.clone(),
                fields,
                file: file.to_string(),
            });

            i += 1;
            continue;
        }

        // Unit variant: VariantName, or VariantName
        if let Some(cap) = variant_unit_re.captures(line) {
            let variant = cap[1].to_string();
            let type_tag = pending_rename
                .take()
                .unwrap_or_else(|| variant.to_lowercase());

            messages.push(WsMessageDef {
                type_tag,
                variant,
                direction: direction.clone(),
                fields: Vec::new(),
                file: file.to_string(),
            });
        }

        i += 1;
    }

    messages
}

// ---------------------------------------------------------------------------
// Rust trait extraction
// ---------------------------------------------------------------------------

fn extract_traits(content: &str, file: &str) -> Vec<TraitDef> {
    let trait_re = Regex::new(r"pub(?:\([^)]*\))?\s+trait\s+(\w+)").unwrap();
    let fn_re = Regex::new(r"(?:async\s+)?fn\s+(\w+)\s*[(<]").unwrap();

    let mut traits = Vec::new();

    for cap in trait_re.captures_iter(content) {
        let name = cap[1].to_string();
        let start = cap.get(0).unwrap().start();

        let Some((body_start, body_end)) = extract_brace_block(content, start) else {
            continue;
        };
        let body = &content[body_start..body_end];

        // Extract method names (skip nested impl blocks by only matching top-level fns)
        let methods: Vec<String> = fn_re
            .captures_iter(body)
            .map(|c| c[1].to_string())
            .collect();

        if !methods.is_empty() {
            traits.push(TraitDef {
                name,
                file: file.to_string(),
                methods,
            });
        }
    }

    traits
}

// ---------------------------------------------------------------------------
// TypeScript interface extraction
// ---------------------------------------------------------------------------

fn extract_interfaces(content: &str, file: &str) -> Vec<InterfaceDef> {
    let iface_re = Regex::new(r"export\s+interface\s+(\w+)").unwrap();
    let field_re = Regex::new(r"^\s*(\w+)\s*\??\s*:\s*(.+?)\s*;?\s*$").unwrap();

    let mut interfaces = Vec::new();

    for cap in iface_re.captures_iter(content) {
        let name = cap[1].to_string();
        let start = cap.get(0).unwrap().start();

        let Some((body_start, body_end)) = extract_brace_block(content, start) else {
            continue;
        };
        let body = &content[body_start..body_end];

        let mut fields = Vec::new();
        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
            {
                continue;
            }
            if let Some(fc) = field_re.captures(trimmed) {
                fields.push(FieldDef {
                    name: fc[1].to_string(),
                    type_name: fc[2].trim_end_matches(';').trim().to_string(),
                });
            }
        }

        interfaces.push(InterfaceDef {
            name,
            file: file.to_string(),
            fields,
        });
    }

    interfaces
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check whether a byte offset in `content` falls inside a comment or string literal.
fn is_in_comment_or_string(content: &str, pos: usize) -> bool {
    let line_start = content[..pos].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let line_prefix = &content[line_start..pos];
    let trimmed = line_prefix.trim_start();

    // Line comment
    if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
        return true;
    }

    // Inside a raw string literal (r#"..."#) or regular string
    // Simple heuristic: count unescaped quotes before this position on the line
    let quote_count = line_prefix.matches('"').count();
    // Odd number of quotes means we're inside a string
    if quote_count % 2 == 1 {
        return true;
    }

    // Inside a raw string (r#")
    if line_prefix.contains("r#\"") || line_prefix.contains("r##\"") {
        // Check if we're between r#" and "#
        let after_raw = line_prefix
            .rfind("r#\"")
            .or_else(|| line_prefix.rfind("r##\""));
        if let Some(raw_start) = after_raw {
            let after = &content[line_start + raw_start..pos];
            // If the raw string hasn't been closed yet on this line
            if !after.contains("\"#")
                || after.rfind("r#\"").unwrap_or(0) > after.rfind("\"#").unwrap_or(0)
            {
                return true;
            }
        }
    }

    false
}

/// Extract the content inside a brace-delimited block starting from `start_pos`.
/// Returns the (start, end) byte offsets of the interior (excluding braces).
fn extract_brace_block(content: &str, start_pos: usize) -> Option<(usize, usize)> {
    let bytes = content.as_bytes();
    let mut pos = start_pos;

    // Find opening brace
    while pos < bytes.len() && bytes[pos] != b'{' {
        pos += 1;
    }
    if pos >= bytes.len() {
        return None;
    }

    let open = pos;
    let mut depth = 1;
    pos += 1;

    while pos < bytes.len() && depth > 0 {
        match bytes[pos] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        pos += 1;
    }

    if depth == 0 {
        Some((open + 1, pos - 1))
    } else {
        None
    }
}

fn is_rust_keyword(s: &str) -> bool {
    matches!(
        s,
        "pub"
            | "fn"
            | "impl"
            | "struct"
            | "enum"
            | "trait"
            | "mod"
            | "use"
            | "let"
            | "mut"
            | "const"
            | "static"
            | "if"
            | "else"
            | "match"
            | "for"
            | "while"
            | "loop"
            | "return"
            | "break"
            | "continue"
            | "where"
            | "async"
            | "await"
            | "self"
            | "super"
            | "crate"
            | "type"
    )
}

fn is_serde_attr(s: &str) -> bool {
    matches!(
        s,
        "serde" | "skip_serializing_if" | "default" | "rename" | "flatten"
    )
}

// ---------------------------------------------------------------------------
// Output formatters
// ---------------------------------------------------------------------------

fn format_json(surface: &ApiSurface, pretty: bool) -> String {
    if pretty {
        serde_json::to_string_pretty(surface).expect("JSON serialization failed")
    } else {
        serde_json::to_string(surface).expect("JSON serialization failed")
    }
}

fn format_markdown(surface: &ApiSurface) -> String {
    let mut md = String::new();

    md.push_str("# API Surface\n\n");
    md.push_str(&format!(
        "> {} routes | {} WebSocket messages | {} traits | {} interfaces\n\n",
        surface.stats.total_routes,
        surface.stats.total_ws_messages,
        surface.stats.total_traits,
        surface.stats.total_interfaces,
    ));

    // ---- HTTP Routes ----
    md.push_str("## HTTP Routes\n\n");
    md.push_str("| Method | Path | Handler | File |\n");
    md.push_str("|--------|------|---------|------|\n");
    for route in &surface.routes {
        md.push_str(&format!(
            "| {} | `{}` | `{}` | {} |\n",
            route.method, route.path, route.handler, route.file
        ));
    }
    md.push('\n');

    // ---- WebSocket Messages ----
    if !surface.ws_messages.is_empty() {
        md.push_str("## WebSocket Messages\n\n");
        md.push_str("| Type | Direction | Fields | File |\n");
        md.push_str("|------|-----------|--------|------|\n");
        for msg in &surface.ws_messages {
            let fields_str = if msg.fields.is_empty() {
                "\u{2014}".to_string()
            } else {
                msg.fields
                    .iter()
                    .map(|f| format!("`{f}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            md.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                msg.type_tag, msg.direction, fields_str, msg.file
            ));
        }
        md.push('\n');
    }

    // ---- Rust Traits ----
    if !surface.traits.is_empty() {
        md.push_str("## Rust Traits\n\n");
        for t in &surface.traits {
            md.push_str(&format!("### `{}` ({})\n\n", t.name, t.file));
            for m in &t.methods {
                md.push_str(&format!("- `{m}`\n"));
            }
            md.push('\n');
        }
    }

    // ---- TypeScript Interfaces ----
    if !surface.interfaces.is_empty() {
        md.push_str("## TypeScript Interfaces\n\n");
        for iface in &surface.interfaces {
            md.push_str(&format!("### `{}` ({})\n\n", iface.name, iface.file));
            if iface.fields.is_empty() {
                md.push_str("_(no fields extracted)_\n");
            } else {
                for f in &iface.fields {
                    md.push_str(&format!("- `{}`: `{}`\n", f.name, f.type_name));
                }
            }
            md.push('\n');
        }
    }

    md
}
