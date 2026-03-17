//! Dependency graph builder — consumes the static analyzer manifest and produces
//! a directed dependency graph with cycle detection and per-node metrics.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Serialize;

use std::path::PathBuf;

use crate::scan::Manifest;

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct DependencyGraph {
    pub stats: GraphStats,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub cycles: Vec<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub module_name: String,
    pub language: String,
    pub fan_in: usize,
    pub fan_out: usize,
    pub depth: usize,
    pub in_cycle: bool,
}

#[derive(Debug, Serialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub import_path: String,
}

#[derive(Debug, Serialize)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub cycle_count: usize,
    pub max_depth: usize,
}

// ---------------------------------------------------------------------------
// Graph construction
// ---------------------------------------------------------------------------

pub fn build_graph(manifest: &Manifest) -> DependencyGraph {
    let file_set: HashSet<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();

    // Ensure every file is in the adjacency map (even if it has no edges)
    let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
    for f in &manifest.files {
        adj.entry(f.path.clone()).or_default();
    }

    // Resolve imports and build deduplicated edges
    let mut edge_set: HashSet<(String, String)> = HashSet::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut fan_out: HashMap<String, usize> = HashMap::new();
    let mut fan_in: HashMap<String, usize> = HashMap::new();

    for f in &manifest.files {
        for import in &f.imports {
            let targets = resolve_import(&f.path, &f.language, import, &file_set);
            for target in targets {
                if target != f.path && edge_set.insert((f.path.clone(), target.clone())) {
                    edges.push(GraphEdge {
                        from: f.path.clone(),
                        to: target.clone(),
                        import_path: import.clone(),
                    });
                    *fan_out.entry(f.path.clone()).or_default() += 1;
                    *fan_in.entry(target.clone()).or_default() += 1;
                    adj.entry(f.path.clone()).or_default().insert(target);
                }
            }
        }
    }

    let mut nodes_list: Vec<String> = adj.keys().cloned().collect();
    nodes_list.sort();

    // --- Cycle detection (Tarjan's SCC) ---
    let sccs = tarjan_scc(&nodes_list, &adj);
    let mut cycles: Vec<Vec<String>> = Vec::new();
    let mut in_cycle_set: HashSet<String> = HashSet::new();

    for scc in &sccs {
        if scc.len() > 1 {
            let cycle_path = find_cycle_in_scc(scc, &adj);
            for node in scc {
                in_cycle_set.insert(node.clone());
            }
            cycles.push(cycle_path);
        } else if scc.len() == 1 {
            // Check for self-loop
            let node = &scc[0];
            if adj.get(node).is_some_and(|n| n.contains(node)) {
                in_cycle_set.insert(node.clone());
                cycles.push(vec![node.clone(), node.clone()]);
            }
        }
    }

    // Sort cycles for deterministic output
    cycles.sort();

    // --- Depth computation ---
    let depths = compute_depths(&nodes_list, &adj);
    let max_depth = depths.values().copied().max().unwrap_or(0);

    // --- Build output nodes ---
    let mut graph_nodes: Vec<GraphNode> = manifest
        .files
        .iter()
        .map(|f| GraphNode {
            id: f.path.clone(),
            module_name: derive_module_name(&f.path, &f.language),
            language: f.language.clone(),
            fan_in: *fan_in.get(&f.path).unwrap_or(&0),
            fan_out: *fan_out.get(&f.path).unwrap_or(&0),
            depth: *depths.get(&f.path).unwrap_or(&0),
            in_cycle: in_cycle_set.contains(&f.path),
        })
        .collect();

    graph_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    edges.sort_by(|a, b| a.from.cmp(&b.from).then_with(|| a.to.cmp(&b.to)));

    DependencyGraph {
        stats: GraphStats {
            total_nodes: graph_nodes.len(),
            total_edges: edges.len(),
            cycle_count: cycles.len(),
            max_depth,
        },
        nodes: graph_nodes,
        edges,
        cycles,
    }
}

// ---------------------------------------------------------------------------
// Import resolution
// ---------------------------------------------------------------------------

fn resolve_import(
    file_path: &str,
    language: &str,
    import: &str,
    file_set: &HashSet<&str>,
) -> Vec<String> {
    match language {
        "rust" => resolve_rust_import(file_path, import, file_set),
        "typescript" => resolve_ts_import(file_path, import, file_set)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn resolve_rust_import(file_path: &str, import: &str, file_set: &HashSet<&str>) -> Vec<String> {
    let crate_src = match find_crate_src(file_path) {
        Some(s) => s,
        None => return Vec::new(),
    };

    if let Some(rest) = import.strip_prefix("crate::") {
        expand_and_resolve(&crate_src, rest, file_set)
    } else if let Some(rest) = import.strip_prefix("super::") {
        // super:: resolves relative to the parent module directory
        let parent = Path::new(file_path).parent().and_then(|p| p.parent());
        match parent {
            Some(p) => expand_and_resolve(&p.to_string_lossy(), rest, file_set),
            None => Vec::new(),
        }
    } else {
        // External crate or std — not resolvable within the project
        Vec::new()
    }
}

/// Expand brace-grouped imports and resolve each path.
///
/// `crate::routes::{tasks, projects}` expands to
/// `["crate::routes::tasks", "crate::routes::projects"]`.
fn expand_and_resolve(base: &str, module_path: &str, file_set: &HashSet<&str>) -> Vec<String> {
    if let Some(brace_idx) = module_path.find("::{") {
        let prefix = &module_path[..brace_idx];
        let items_str = module_path[brace_idx + 3..]
            .trim_end_matches('}')
            .trim_end_matches(|c: char| c.is_whitespace());

        let mut results = Vec::new();
        let mut resolved_any = false;

        for item in items_str.split(',') {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            if item == "self" {
                if let Some(r) = try_resolve_module(base, prefix, file_set) {
                    results.push(r);
                    resolved_any = true;
                }
            } else {
                let full = format!("{prefix}::{item}");
                if let Some(r) = try_resolve_module(base, &full, file_set) {
                    results.push(r);
                    resolved_any = true;
                }
            }
        }

        // Fall back to the prefix module if nothing resolved
        if !resolved_any && let Some(r) = try_resolve_module(base, prefix, file_set) {
            results.push(r);
        }

        results
    } else {
        try_resolve_module(base, module_path, file_set)
            .into_iter()
            .collect()
    }
}

/// Try progressively shorter prefixes of `module_path` to find a matching file.
///
/// `routes::tasks::SomeType` tries:
///   1. `{base}/routes/tasks/SomeType.rs`
///   2. `{base}/routes/tasks.rs`
///   3. `{base}/routes.rs`
fn try_resolve_module(base: &str, module_path: &str, file_set: &HashSet<&str>) -> Option<String> {
    let parts: Vec<&str> = module_path.split("::").collect();

    for len in (1..=parts.len()).rev() {
        let joined = parts[..len].join("/");
        let file_path = format!("{base}/{joined}.rs");
        if file_set.contains(file_path.as_str()) {
            return Some(file_path);
        }
        let dir_path = format!("{base}/{joined}/mod.rs");
        if file_set.contains(dir_path.as_str()) {
            return Some(dir_path);
        }
    }

    None
}

fn resolve_ts_import(file_path: &str, import: &str, file_set: &HashSet<&str>) -> Option<String> {
    // Only resolve relative imports (starting with . or ..)
    if !import.starts_with('.') {
        return None;
    }

    let file_dir = Path::new(file_path).parent()?;
    let resolved = file_dir.join(import);
    let normalized = normalize_path(&resolved);
    let normalized_str = normalized.to_string_lossy();

    // Try with various extensions
    for ext in &[".ts", ".tsx", "/index.ts", "/index.tsx", ""] {
        let candidate = format!("{normalized_str}{ext}");
        if file_set.contains(candidate.as_str()) {
            return Some(candidate);
        }
    }

    None
}

/// Find the `src/` directory for a Rust file, e.g.
/// `apps/api/src/routes/tasks.rs` -> `apps/api/src`.
fn find_crate_src(file_path: &str) -> Option<String> {
    if let Some(idx) = file_path.find("/src/") {
        Some(format!("{}/src", &file_path[..idx]))
    } else if file_path.starts_with("src/") {
        Some("src".to_string())
    } else {
        None
    }
}

/// Normalize a path by resolving `.` and `..` components.
fn normalize_path(path: &Path) -> std::path::PathBuf {
    let mut components: Vec<std::path::Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => {
                components.push(other);
            }
        }
    }
    components.iter().collect()
}

// ---------------------------------------------------------------------------
// Cycle detection — Tarjan's strongly connected components
// ---------------------------------------------------------------------------

fn tarjan_scc(nodes: &[String], adj: &HashMap<String, HashSet<String>>) -> Vec<Vec<String>> {
    let mut index_counter: usize = 0;
    let mut stack: Vec<String> = Vec::new();
    let mut on_stack: HashSet<String> = HashSet::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut lowlink: HashMap<String, usize> = HashMap::new();
    let mut sccs: Vec<Vec<String>> = Vec::new();

    for node in nodes {
        if !index.contains_key(node.as_str()) {
            strongconnect(
                node,
                adj,
                &mut index_counter,
                &mut stack,
                &mut on_stack,
                &mut index,
                &mut lowlink,
                &mut sccs,
            );
        }
    }

    sccs
}

#[allow(clippy::too_many_arguments)]
fn strongconnect(
    v: &str,
    adj: &HashMap<String, HashSet<String>>,
    index_counter: &mut usize,
    stack: &mut Vec<String>,
    on_stack: &mut HashSet<String>,
    index: &mut HashMap<String, usize>,
    lowlink: &mut HashMap<String, usize>,
    sccs: &mut Vec<Vec<String>>,
) {
    index.insert(v.to_string(), *index_counter);
    lowlink.insert(v.to_string(), *index_counter);
    *index_counter += 1;
    stack.push(v.to_string());
    on_stack.insert(v.to_string());

    if let Some(neighbors) = adj.get(v) {
        let mut sorted_neighbors: Vec<&String> = neighbors.iter().collect();
        sorted_neighbors.sort();
        for w in sorted_neighbors {
            if !index.contains_key(w.as_str()) {
                strongconnect(w, adj, index_counter, stack, on_stack, index, lowlink, sccs);
                let w_low = lowlink[w.as_str()];
                let v_low = lowlink.get_mut(v).unwrap();
                *v_low = (*v_low).min(w_low);
            } else if on_stack.contains(w.as_str()) {
                let w_idx = index[w.as_str()];
                let v_low = lowlink.get_mut(v).unwrap();
                *v_low = (*v_low).min(w_idx);
            }
        }
    }

    if lowlink[v] == index[v] {
        let mut scc = Vec::new();
        loop {
            let w = stack.pop().unwrap();
            on_stack.remove(&w);
            scc.push(w.clone());
            if w == v {
                break;
            }
        }
        sccs.push(scc);
    }
}

/// Given an SCC with >1 node, trace one cycle path through it.
fn find_cycle_in_scc(scc: &[String], adj: &HashMap<String, HashSet<String>>) -> Vec<String> {
    let scc_set: HashSet<&str> = scc.iter().map(|s| s.as_str()).collect();
    // Use the lexicographically smallest node as start for deterministic output
    let start = scc.iter().min().unwrap();

    let mut visited: HashSet<String> = HashSet::new();
    let mut path: Vec<String> = vec![start.clone()];
    visited.insert(start.clone());

    if dfs_cycle(start, start, adj, &scc_set, &mut visited, &mut path) {
        path
    } else {
        // Should not happen for an SCC with >1 node, but return the SCC list sorted
        let mut fallback: Vec<String> = scc.to_vec();
        fallback.sort();
        fallback.push(fallback[0].clone());
        fallback
    }
}

fn dfs_cycle(
    current: &str,
    start: &str,
    adj: &HashMap<String, HashSet<String>>,
    scc_set: &HashSet<&str>,
    visited: &mut HashSet<String>,
    path: &mut Vec<String>,
) -> bool {
    if let Some(neighbors) = adj.get(current) {
        let mut sorted_neighbors: Vec<&String> = neighbors.iter().collect();
        sorted_neighbors.sort();
        for next in sorted_neighbors {
            if !scc_set.contains(next.as_str()) {
                continue;
            }
            if next == start && path.len() > 1 {
                path.push(start.to_string());
                return true;
            }
            if !visited.contains(next.as_str()) {
                visited.insert(next.clone());
                path.push(next.clone());
                if dfs_cycle(next, start, adj, scc_set, visited, path) {
                    return true;
                }
                path.pop();
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Depth computation
// ---------------------------------------------------------------------------

/// Compute the depth of each node. Depth is defined as the length of the
/// longest dependency chain below the node (a leaf with no imports has depth 0).
fn compute_depths(
    nodes: &[String],
    adj: &HashMap<String, HashSet<String>>,
) -> HashMap<String, usize> {
    let mut depths: HashMap<String, usize> = HashMap::new();
    let mut in_progress: HashSet<String> = HashSet::new();

    for node in nodes {
        dfs_depth(node, adj, &mut depths, &mut in_progress);
    }

    depths
}

fn dfs_depth(
    node: &str,
    adj: &HashMap<String, HashSet<String>>,
    depths: &mut HashMap<String, usize>,
    in_progress: &mut HashSet<String>,
) -> usize {
    if let Some(&d) = depths.get(node) {
        return d;
    }
    if in_progress.contains(node) {
        // Cycle — break with 0 to avoid infinite recursion
        return 0;
    }

    in_progress.insert(node.to_string());

    let mut max_dep_depth: usize = 0;
    if let Some(deps) = adj.get(node) {
        for dep in deps {
            let dep_depth = dfs_depth(dep, adj, depths, in_progress);
            max_dep_depth = max_dep_depth.max(dep_depth + 1);
        }
    }

    in_progress.remove(node);
    depths.insert(node.to_string(), max_dep_depth);
    max_dep_depth
}

// ---------------------------------------------------------------------------
// Module name derivation
// ---------------------------------------------------------------------------

fn derive_module_name(path: &str, language: &str) -> String {
    match language {
        "rust" => {
            if let Some(idx) = path.find("/src/") {
                let module = &path[idx + 5..];
                let module = module.strip_suffix(".rs").unwrap_or(module);
                let module = module.strip_suffix("/mod").unwrap_or(module);
                module.replace('/', "::")
            } else {
                path.strip_suffix(".rs").unwrap_or(path).to_string()
            }
        }
        "typescript" => path
            .strip_suffix(".ts")
            .or_else(|| path.strip_suffix(".tsx"))
            .unwrap_or(path)
            .to_string(),
        _ => path.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Mermaid output
// ---------------------------------------------------------------------------

pub fn to_mermaid(graph: &DependencyGraph) -> String {
    let mut out = String::new();
    out.push_str("graph TD\n");

    // Build short-id map and declare nodes
    let mut id_map: HashMap<&str, String> = HashMap::new();
    for (i, node) in graph.nodes.iter().enumerate() {
        let short = format!("n{i}");
        // Sanitize label for Mermaid (escape double quotes)
        let label = node.module_name.replace('"', "'");
        out.push_str(&format!("    {short}[\"{label}\"]\n"));
        id_map.insert(&node.id, short);
    }

    out.push('\n');

    // Edges
    for edge in &graph.edges {
        if let (Some(from), Some(to)) =
            (id_map.get(edge.from.as_str()), id_map.get(edge.to.as_str()))
        {
            out.push_str(&format!("    {from} --> {to}\n"));
        }
    }

    // Highlight cycle nodes
    let cycle_nodes: Vec<&GraphNode> = graph.nodes.iter().filter(|n| n.in_cycle).collect();
    if !cycle_nodes.is_empty() {
        out.push('\n');
        for node in &cycle_nodes {
            if let Some(short) = id_map.get(node.id.as_str()) {
                out.push_str(&format!("    style {short} fill:#f96,stroke:#333\n"));
            }
        }
    }

    // Annotate cycles as comments
    if !graph.cycles.is_empty() {
        out.push('\n');
        for (i, cycle) in graph.cycles.iter().enumerate() {
            let names: Vec<String> = cycle
                .iter()
                .map(|id| {
                    graph
                        .nodes
                        .iter()
                        .find(|n| n.id == *id)
                        .map(|n| n.module_name.clone())
                        .unwrap_or_else(|| id.clone())
                })
                .collect();
            out.push_str(&format!("    %% Cycle {}: {}\n", i + 1, names.join(" -> ")));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// DOT (Graphviz) output
// ---------------------------------------------------------------------------

pub fn to_dot(graph: &DependencyGraph) -> String {
    let mut out = String::new();
    out.push_str("digraph dependencies {\n");
    out.push_str("    rankdir=LR;\n");
    out.push_str("    node [shape=box, style=filled, fillcolor=\"#e8e8e8\"];\n\n");

    // Declare cycle nodes with a different color
    for node in &graph.nodes {
        if node.in_cycle {
            let label = node.module_name.replace('"', "\\\"");
            out.push_str(&format!(
                "    \"{}\" [label=\"{label}\", fillcolor=\"#ff9966\"];\n",
                node.id
            ));
        }
    }

    out.push('\n');

    // Edges
    for edge in &graph.edges {
        out.push_str(&format!("    \"{}\" -> \"{}\";\n", edge.from, edge.to));
    }

    // Cycle annotations as comments
    if !graph.cycles.is_empty() {
        out.push('\n');
        for (i, cycle) in graph.cycles.iter().enumerate() {
            out.push_str(&format!("    // Cycle {}: {}\n", i + 1, cycle.join(" -> ")));
        }
    }

    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(
    manifest_path: PathBuf,
    output: Option<PathBuf>,
    pretty: bool,
    mermaid: bool,
    dot: bool,
) {
    let content = std::fs::read_to_string(&manifest_path).unwrap_or_else(|e| {
        eprintln!("Failed to read {}: {e}", manifest_path.display());
        std::process::exit(1);
    });
    let manifest: Manifest = serde_json::from_str(&content).unwrap_or_else(|e| {
        eprintln!("Failed to parse manifest: {e}");
        std::process::exit(1);
    });

    let dep_graph = build_graph(&manifest);

    let output_text = if mermaid {
        to_mermaid(&dep_graph)
    } else if dot {
        to_dot(&dep_graph)
    } else if pretty {
        serde_json::to_string_pretty(&dep_graph).expect("JSON serialization failed")
    } else {
        serde_json::to_string(&dep_graph).expect("JSON serialization failed")
    };

    match output {
        Some(ref out) => {
            std::fs::write(out, &output_text).unwrap_or_else(|e| {
                eprintln!("Failed to write {}: {e}", out.display());
                std::process::exit(1);
            });
            eprintln!(
                "Wrote dependency graph ({} nodes, {} edges) to {}",
                dep_graph.stats.total_nodes,
                dep_graph.stats.total_edges,
                out.display()
            );
        }
        None => println!("{output_text}"),
    }
}

// ---------------------------------------------------------------------------
// Module-level dependency graph (coarse-grained: apps/api, libs/utils, etc.)
// ---------------------------------------------------------------------------

/// Extract the top-level module from a file path.
///
/// `apps/api/src/routes/tasks.rs` → `apps/api`
/// `libs/common-rust/shared-utils/src/lib.rs` → `libs/common-rust`
pub fn module_of(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 2 && (parts[0] == "apps" || parts[0] == "libs") {
        format!("{}/{}", parts[0], parts[1])
    } else if !parts.is_empty() && !parts[0].is_empty() {
        parts[0].to_string()
    } else {
        "root".to_string()
    }
}

/// Try to resolve an import string to a known module in the workspace.
#[allow(dead_code)]
fn resolve_import_target(
    import: &str,
    known_modules: &std::collections::BTreeSet<String>,
    crate_to_module: &HashMap<String, String>,
) -> Option<String> {
    // Relative path imports (TypeScript style: ../../core/services/...)
    // These are intra-module — skip them
    if import.starts_with('.') {
        return None;
    }

    // Direct module path match (e.g., "apps/api/src/...")
    for m in known_modules {
        if import.starts_with(m.as_str()) {
            return Some(m.clone());
        }
    }

    // Rust crate imports: check if first segment matches a workspace crate
    let first_segment = import.split("::").next().unwrap_or(import);
    if let Some(module) = crate_to_module.get(first_segment) {
        return Some(module.clone());
    }

    // Not a workspace import (external crate or std lib)
    None
}

/// Build a module-level adjacency map from a scan manifest.
///
/// For each file, maps its module to the set of modules it imports from.
/// Only considers imports that reference another module in the workspace.
#[allow(dead_code)]
pub fn build_module_graph(
    manifest: &Manifest,
) -> std::collections::BTreeMap<String, std::collections::BTreeSet<String>> {
    use std::collections::{BTreeMap, BTreeSet};

    // First pass: collect all known modules
    let known_modules: BTreeSet<String> =
        manifest.files.iter().map(|f| module_of(&f.path)).collect();

    // Build crate name → module mapping for Rust workspace crates
    let mut crate_to_module: HashMap<String, String> = HashMap::new();
    for m in &known_modules {
        if let Some(crate_name) = m.split('/').next_back() {
            crate_to_module.insert(crate_name.replace('-', "_"), m.clone());
        }
    }

    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for file in &manifest.files {
        let src_module = module_of(&file.path);
        let entry = graph.entry(src_module.clone()).or_default();

        for imp in &file.imports {
            let target = resolve_import_target(imp, &known_modules, &crate_to_module);
            if let Some(target_module) = target
                && target_module != src_module
            {
                entry.insert(target_module);
            }
        }
    }

    graph
}

/// Detect cycles in a module dependency graph using Tarjan's SCC algorithm.
/// Returns cycles as sorted vectors of module names (only SCCs with size > 1).
#[allow(dead_code)]
pub fn detect_module_cycles(
    graph: &std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
) -> Vec<Vec<String>> {
    use std::collections::BTreeSet;

    let nodes: Vec<&String> = graph.keys().collect();
    let node_index: HashMap<&String, usize> =
        nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();

    let n = nodes.len();
    let mut index_counter: usize = 0;
    let mut stack: Vec<usize> = Vec::new();
    let mut on_stack = vec![false; n];
    let mut indices = vec![usize::MAX; n];
    let mut lowlinks = vec![usize::MAX; n];
    let mut sccs: Vec<Vec<String>> = Vec::new();

    #[allow(clippy::too_many_arguments)]
    fn strongconnect(
        v: usize,
        nodes: &[&String],
        graph: &std::collections::BTreeMap<String, BTreeSet<String>>,
        node_index: &HashMap<&String, usize>,
        index_counter: &mut usize,
        stack: &mut Vec<usize>,
        on_stack: &mut [bool],
        indices: &mut [usize],
        lowlinks: &mut [usize],
        sccs: &mut Vec<Vec<String>>,
    ) {
        indices[v] = *index_counter;
        lowlinks[v] = *index_counter;
        *index_counter += 1;
        stack.push(v);
        on_stack[v] = true;

        if let Some(neighbors) = graph.get(nodes[v]) {
            for neighbor in neighbors {
                if let Some(&w) = node_index.get(neighbor) {
                    if indices[w] == usize::MAX {
                        strongconnect(
                            w,
                            nodes,
                            graph,
                            node_index,
                            index_counter,
                            stack,
                            on_stack,
                            indices,
                            lowlinks,
                            sccs,
                        );
                        lowlinks[v] = lowlinks[v].min(lowlinks[w]);
                    } else if on_stack[w] {
                        lowlinks[v] = lowlinks[v].min(indices[w]);
                    }
                }
            }
        }

        if lowlinks[v] == indices[v] {
            let mut scc = Vec::new();
            while let Some(w) = stack.pop() {
                on_stack[w] = false;
                scc.push(nodes[w].clone());
                if w == v {
                    break;
                }
            }
            if scc.len() > 1 {
                scc.sort();
                sccs.push(scc);
            }
        }
    }

    for i in 0..n {
        if indices[i] == usize::MAX {
            strongconnect(
                i,
                &nodes,
                graph,
                &node_index,
                &mut index_counter,
                &mut stack,
                &mut on_stack,
                &mut indices,
                &mut lowlinks,
                &mut sccs,
            );
        }
    }

    sccs.sort();
    sccs
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::{FileEntry, Stats};

    fn make_manifest(files: Vec<FileEntry>) -> Manifest {
        Manifest {
            stats: Stats {
                total_files: files.len(),
                by_language: std::collections::HashMap::new(),
                elapsed_ms: 0,
            },
            files,
        }
    }

    fn make_file(path: &str, language: &str, imports: Vec<&str>) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            language: language.to_string(),
            imports: imports.into_iter().map(String::from).collect(),
            exports: Vec::new(),
            routes: Vec::new(),
            docstring: None,
        }
    }

    #[test]
    fn test_empty_manifest() {
        let manifest = make_manifest(Vec::new());
        let graph = build_graph(&manifest);
        assert_eq!(graph.stats.total_nodes, 0);
        assert_eq!(graph.stats.total_edges, 0);
        assert!(graph.cycles.is_empty());
    }

    #[test]
    fn test_rust_crate_import_resolution() {
        let manifest = make_manifest(vec![
            make_file("apps/api/src/main.rs", "rust", vec!["crate::models"]),
            make_file("apps/api/src/models.rs", "rust", vec![]),
        ]);
        let graph = build_graph(&manifest);

        assert_eq!(graph.stats.total_nodes, 2);
        assert_eq!(graph.stats.total_edges, 1);
        assert_eq!(graph.edges[0].from, "apps/api/src/main.rs");
        assert_eq!(graph.edges[0].to, "apps/api/src/models.rs");
    }

    #[test]
    fn test_fan_in_fan_out() {
        let manifest = make_manifest(vec![
            make_file(
                "apps/api/src/main.rs",
                "rust",
                vec!["crate::models", "crate::routes"],
            ),
            make_file("apps/api/src/routes.rs", "rust", vec!["crate::models"]),
            make_file("apps/api/src/models.rs", "rust", vec![]),
        ]);
        let graph = build_graph(&manifest);

        let main = graph
            .nodes
            .iter()
            .find(|n| n.id == "apps/api/src/main.rs")
            .unwrap();
        let routes = graph
            .nodes
            .iter()
            .find(|n| n.id == "apps/api/src/routes.rs")
            .unwrap();
        let models = graph
            .nodes
            .iter()
            .find(|n| n.id == "apps/api/src/models.rs")
            .unwrap();

        // main imports 2 modules
        assert_eq!(main.fan_out, 2);
        assert_eq!(main.fan_in, 0);

        // routes imports 1, is imported by 1
        assert_eq!(routes.fan_out, 1);
        assert_eq!(routes.fan_in, 1);

        // models imports nothing, is imported by 2
        assert_eq!(models.fan_out, 0);
        assert_eq!(models.fan_in, 2);
    }

    #[test]
    fn test_depth_computation() {
        // main -> routes -> models
        let manifest = make_manifest(vec![
            make_file("apps/api/src/main.rs", "rust", vec!["crate::routes"]),
            make_file("apps/api/src/routes.rs", "rust", vec!["crate::models"]),
            make_file("apps/api/src/models.rs", "rust", vec![]),
        ]);
        let graph = build_graph(&manifest);

        let main = graph
            .nodes
            .iter()
            .find(|n| n.id == "apps/api/src/main.rs")
            .unwrap();
        let routes = graph
            .nodes
            .iter()
            .find(|n| n.id == "apps/api/src/routes.rs")
            .unwrap();
        let models = graph
            .nodes
            .iter()
            .find(|n| n.id == "apps/api/src/models.rs")
            .unwrap();

        // models has depth 0 (no deps), routes depth 1, main depth 2
        assert_eq!(models.depth, 0);
        assert_eq!(routes.depth, 1);
        assert_eq!(main.depth, 2);
    }

    #[test]
    fn test_cycle_detection() {
        let manifest = make_manifest(vec![
            make_file("apps/api/src/a.rs", "rust", vec!["crate::b"]),
            make_file("apps/api/src/b.rs", "rust", vec!["crate::a"]),
        ]);
        let graph = build_graph(&manifest);

        assert_eq!(graph.stats.cycle_count, 1);
        assert!(!graph.cycles.is_empty());

        let a = graph
            .nodes
            .iter()
            .find(|n| n.id == "apps/api/src/a.rs")
            .unwrap();
        let b = graph
            .nodes
            .iter()
            .find(|n| n.id == "apps/api/src/b.rs")
            .unwrap();
        assert!(a.in_cycle);
        assert!(b.in_cycle);

        // The cycle path should start and end with the same node
        let cycle = &graph.cycles[0];
        assert_eq!(cycle.first(), cycle.last());
    }

    #[test]
    fn test_brace_import_expansion() {
        let manifest = make_manifest(vec![
            make_file(
                "apps/api/src/main.rs",
                "rust",
                vec!["crate::routes::{tasks, projects}"],
            ),
            make_file("apps/api/src/routes/tasks.rs", "rust", vec![]),
            make_file("apps/api/src/routes/projects.rs", "rust", vec![]),
        ]);
        let graph = build_graph(&manifest);

        assert_eq!(graph.stats.total_edges, 2);

        let main = graph
            .nodes
            .iter()
            .find(|n| n.id == "apps/api/src/main.rs")
            .unwrap();
        assert_eq!(main.fan_out, 2);
    }

    #[test]
    fn test_typescript_relative_import() {
        let manifest = make_manifest(vec![
            make_file(
                "apps/web/src/app/main.ts",
                "typescript",
                vec!["./services/api"],
            ),
            make_file("apps/web/src/app/services/api.ts", "typescript", vec![]),
        ]);
        let graph = build_graph(&manifest);

        assert_eq!(graph.stats.total_edges, 1);
        assert_eq!(graph.edges[0].from, "apps/web/src/app/main.ts");
        assert_eq!(graph.edges[0].to, "apps/web/src/app/services/api.ts");
    }

    #[test]
    fn test_external_imports_skipped() {
        let manifest = make_manifest(vec![
            make_file(
                "apps/api/src/main.rs",
                "rust",
                vec!["std::collections::HashMap", "serde::Serialize"],
            ),
            make_file(
                "apps/web/src/app/main.ts",
                "typescript",
                vec!["@angular/core", "rxjs"],
            ),
        ]);
        let graph = build_graph(&manifest);

        // External imports produce no edges
        assert_eq!(graph.stats.total_edges, 0);
    }

    #[test]
    fn test_mermaid_output_valid() {
        let manifest = make_manifest(vec![
            make_file("apps/api/src/main.rs", "rust", vec!["crate::models"]),
            make_file("apps/api/src/models.rs", "rust", vec![]),
        ]);
        let graph = build_graph(&manifest);
        let mermaid = to_mermaid(&graph);

        assert!(mermaid.starts_with("graph TD\n"));
        assert!(mermaid.contains("-->"));
        // Basic syntax check: no unbalanced brackets
        assert_eq!(mermaid.matches('[').count(), mermaid.matches(']').count());
    }

    #[test]
    fn test_dot_output_valid() {
        let manifest = make_manifest(vec![
            make_file("apps/api/src/main.rs", "rust", vec!["crate::models"]),
            make_file("apps/api/src/models.rs", "rust", vec![]),
        ]);
        let graph = build_graph(&manifest);
        let dot = to_dot(&graph);

        assert!(dot.starts_with("digraph dependencies {\n"));
        assert!(dot.ends_with("}\n"));
        assert!(dot.contains("->"));
    }

    #[test]
    fn test_module_name_derivation() {
        assert_eq!(
            derive_module_name("apps/api/src/routes/tasks.rs", "rust"),
            "routes::tasks"
        );
        assert_eq!(derive_module_name("apps/api/src/main.rs", "rust"), "main");
        assert_eq!(
            derive_module_name("apps/api/src/routes/mod.rs", "rust"),
            "routes"
        );
        assert_eq!(
            derive_module_name("apps/web/src/app/services/api.ts", "typescript"),
            "apps/web/src/app/services/api"
        );
    }

    #[test]
    fn test_no_self_edges() {
        // A file importing from its own module should not create a self-edge
        let manifest = make_manifest(vec![make_file(
            "apps/api/src/models.rs",
            "rust",
            vec!["crate::models::SomeType"],
        )]);
        let graph = build_graph(&manifest);
        assert_eq!(graph.stats.total_edges, 0);
    }
}
