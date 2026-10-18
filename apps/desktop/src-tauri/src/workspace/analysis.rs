use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::scanner::{is_sensitive_file, scan_workspace, WorkspaceError, MAX_PREVIEW_BYTES};
use super::symbol_index::{extract_symbols_for_file, Symbol, SymbolKind};

// Limits - reuse symbol_index limits where possible, keep bounded
pub const MAX_ANALYSIS_FILES: usize = 500;
pub const MAX_ANALYSIS_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_EDGES_TOTAL: usize = 2000;
pub const MAX_EDGES_PER_FILE: usize = 50;
pub const MAX_TRAVERSAL_DEPTH: usize = 4;
pub const MAX_IMPACT_RESULTS: usize = 20;
pub const MAX_CYCLES: usize = 10;
pub const MAX_CYCLE_LEN: usize = 10;
pub const CURRENT_VERSION: u32 = 1;

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn line_number(content: &str, offset: usize) -> u32 {
    content[..offset.min(content.len())].chars().filter(|&c| c == '\n').count() as u32 + 1
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    Import,
    Export,
    Reference,
    Uses,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::Export => "export",
            Self::Reference => "reference",
            Self::Uses => "uses",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyEdge {
    pub from: String,
    pub to: String,           // resolved path or raw spec if unresolved
    pub raw: String,          // original spec
    pub line: u32,
    pub verified: bool,       // true if resolved to existing workspace file
    pub kind: String,         // import/export/uses
    pub note: Option<String>, // e.g., "external package — not resolved locally", "unresolved — file not found"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileNode {
    pub path: String,
    pub imports: Vec<DependencyEdge>,
    pub imported_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisGraph {
    pub version: u32,
    pub root: String,
    pub nodes: Vec<FileNode>,
    pub edges: Vec<DependencyEdge>,
    pub truncated: bool,
    pub unsupported: usize,
    pub cycles: Vec<Vec<String>>,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub path: String,
    pub line: u32,
    pub symbol: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRelationships {
    pub path: String,
    pub imports: Vec<DependencyEdge>,
    pub imported_by: Vec<String>,
    pub defined_symbols: Vec<Symbol>,
    pub referenced_by: Vec<Reference>, // files that mention symbols defined in this file
    pub unresolved: Vec<DependencyEdge>, // imports not verified
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactNode {
    pub path: String,
    pub reason: String,
    pub evidence: String, // path:line or edge raw
    pub line: Option<u32>,
    pub confidence: String, // verified / inferred
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactReport {
    pub target: String,
    pub direct: Vec<ImpactNode>,
    pub transitive: Vec<ImpactNode>,
    pub risks: Vec<String>,
    pub truncated: bool,
    pub notes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Import extraction
// ---------------------------------------------------------------------------

fn extract_imports_ts(content: &str, path: &str) -> Vec<(String, u32, String)> {
    // Returns (raw_spec, line, kind)
    let mut out = Vec::new();
    // patterns: import ... from 'spec', import 'spec', require('spec'), import('spec'), from 'spec'
    let patterns = [
        r#"import\s+(?:[\w*\s{},]+\s+from\s+)?['"]([^'"]+)['"]"#,
        r#"require\s*\(\s*['"]([^'"]+)['"]\s*\)"#,
        r#"import\s*\(\s*['"]([^'"]+)['"]\s*\)"#,
        r#"export\s+.*from\s*['"]([^'"]+)['"]"#,
    ];
    for pat in patterns {
        let re = match Regex::new(pat) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for caps in re.captures_iter(content) {
            let m = caps.get(0).unwrap();
            let spec = caps.get(1).map(|g| g.as_str().to_string()).unwrap_or_default();
            if spec.trim().is_empty() || spec.len() > 300 {
                continue;
            }
            if out.len() >= MAX_EDGES_PER_FILE {
                break;
            }
            let line = line_number(content, m.start());
            // de-duplicate by spec+line? keep simple
            out.push((spec, line, "import".to_string()));
            let _ = path;
        }
    }
    // De-duplicate by raw spec keeping earliest line
    let mut seen = HashSet::new();
    let mut dedup = Vec::new();
    for (spec, line, kind) in out {
        let key = spec.clone();
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        dedup.push((spec, line, kind));
        if dedup.len() >= MAX_EDGES_PER_FILE {
            break;
        }
    }
    dedup
}

fn extract_imports_py(content: &str, path: &str) -> Vec<(String, u32, String)> {
    let mut out = Vec::new();
    let patterns = [
        (r"(?m)^\s*import\s+([A-Za-z0-9_.]+)", "import"),
        (r"(?m)^\s*from\s+([A-Za-z0-9_.]+)\s+import", "import"),
    ];
    for (pat, kind) in patterns {
        let re = match Regex::new(pat) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for caps in re.captures_iter(content) {
            if out.len() >= MAX_EDGES_PER_FILE {
                break;
            }
            let m = caps.get(0).unwrap();
            let spec = caps.get(1).map(|g| g.as_str().to_string()).unwrap_or_default();
            if spec.trim().is_empty() || spec.len() > 300 {
                continue;
            }
            let line = line_number(content, m.start());
            out.push((spec, line, kind.to_string()));
            let _ = path;
        }
    }
    let mut seen = HashSet::new();
    let mut dedup = Vec::new();
    for (spec, line, kind) in out {
        if seen.contains(&spec) {
            continue;
        }
        seen.insert(spec.clone());
        dedup.push((spec, line, kind));
    }
    dedup
}

fn extract_imports_rs(content: &str, path: &str) -> Vec<(String, u32, String)> {
    let mut out = Vec::new();
    let patterns = [
        (r"(?m)^\s*use\s+([^;]+);", "import"),
        (r#"(?m)^\s*mod\s+(\w+)\s*;"#, "import"),
    ];
    for (pat, kind) in patterns {
        let re = match Regex::new(pat) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for caps in re.captures_iter(content) {
            if out.len() >= MAX_EDGES_PER_FILE {
                break;
            }
            let m = caps.get(0).unwrap();
            let spec = caps.get(1).map(|g| g.as_str().trim().to_string()).unwrap_or_default();
            if spec.trim().is_empty() || spec.len() > 300 {
                continue;
            }
            let line = line_number(content, m.start());
            out.push((spec, line, kind.to_string()));
            let _ = path;
        }
    }
    let mut seen = HashSet::new();
    let mut dedup = Vec::new();
    for (spec, line, kind) in out {
        if seen.contains(&spec) {
            continue;
        }
        seen.insert(spec.clone());
        dedup.push((spec, line, kind));
    }
    dedup
}

fn extract_imports_for_file(content: &str, path: &str) -> Vec<(String, u32, String)> {
    let ext = path.split('.').last().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "ts" | "tsx" | "js" | "jsx" => extract_imports_ts(content, path),
        "py" => extract_imports_py(content, path),
        "rs" => extract_imports_rs(content, path),
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Resolve import spec to workspace-relative path
// ---------------------------------------------------------------------------

fn normalize_posix(p: &str) -> String {
    // Collapse //, remove ./, handle ../ naively but validation already ensures files are within root
    // For resolution we just join and normalize via Path components
    let mut parts: Vec<String> = Vec::new();
    for comp in p.split('/') {
        if comp.is_empty() || comp == "." {
            continue;
        } else if comp == ".." {
            parts.pop();
        } else {
            parts.push(comp.to_string());
        }
    }
    parts.join("/")
}

fn resolve_ts_spec(spec: &str, from_path: &str, existing: &HashSet<String>) -> Option<String> {
    // Only resolve relative specs (starting with . or / or without prefix but we check existence)
    // Bare specs like "react" are external — not resolved
    if spec.starts_with('.') {
        // relative
        let from_dir = from_path.rsplitn(2, '/').collect::<Vec<_>>();
        let base_dir = if from_dir.len() == 2 { from_dir[1] } else { "" };
        let joined = if base_dir.is_empty() {
            spec.to_string()
        } else {
            format!("{}/{}", base_dir, spec)
        };
        let norm = normalize_posix(&joined);
        // Try candidates
        let candidates = [
            norm.clone(),
            format!("{}.ts", norm),
            format!("{}.tsx", norm),
            format!("{}.js", norm),
            format!("{}.jsx", norm),
            format!("{}.json", norm),
            format!("{}/index.ts", norm),
            format!("{}/index.tsx", norm),
            format!("{}/index.js", norm),
        ];
        for cand in candidates {
            let cand_norm = normalize_posix(&cand);
            if existing.contains(&cand_norm) {
                return Some(cand_norm);
            }
        }
        // Also try if spec already has extension
        let with_ext_norm = normalize_posix(&norm);
        if existing.contains(&with_ext_norm) {
            return Some(with_ext_norm);
        }
        None
    } else if spec.starts_with('/') {
        let norm = normalize_posix(spec.trim_start_matches('/'));
        if existing.contains(&norm) {
            return Some(norm);
        }
        None
    } else {
        // bare spec — might be workspace file like "components/Button" without dot?
        // Try as workspace-relative if it exists
        let candidates = [
            spec.to_string(),
            format!("{}.ts", spec),
            format!("{}.tsx", spec),
            format!("{}.js", spec),
            format!("{}.jsx", spec),
        ];
        for cand in candidates {
            let n = normalize_posix(&cand);
            if existing.contains(&n) {
                return Some(n);
            }
        }
        // Try src/ prefix common? Not guessing — treat as external
        None
    }
}

fn resolve_py_spec(spec: &str, existing: &HashSet<String>) -> Option<String> {
    // spec like "pkg.sub.module" or "pkg"
    let path = spec.replace('.', "/");
    let candidates = [
        format!("{}.py", path),
        format!("{}/__init__.py", path),
        format!("{}.py", spec), // fallback
    ];
    for cand in candidates {
        let n = normalize_posix(&cand);
        if existing.contains(&n) {
            return Some(n);
        }
    }
    // Also try relative? For local imports, Python may use relative without dot? Hard to know — treat as unresolved if not found
    None
}

fn resolve_rs_mod(spec: &str, from_path: &str, existing: &HashSet<String>) -> Option<String> {
    // spec for `mod foo;` is just "foo"
    // For `use crate::foo::bar` — we can try to map to file path
    // Simplify: if spec is single word (mod), try dir/foo.rs and foo.rs
    let trimmed = spec.trim();
    // If contains ::, take first after crate:: or self:: or super::
    // For use, spec may be "crate::utils::helper" or "std::fs" or "super::foo"
    if trimmed.contains("::") {
        // external crates like std, core, alloc, serde etc are external
        let first = trimmed.split("::").next().unwrap_or("").trim();
        if matches!(first, "std" | "core" | "alloc" | "serde" | "serde_json" | "tauri" | "tokio" | "regex" | "sha2" | "similar" | "tempfile" | "reqwest" | "futures_util" | "getrandom") {
            return None;
        }
        // For crate::..., try to resolve to src/...
        let path_part = if trimmed.starts_with("crate::") {
            trimmed.trim_start_matches("crate::").replace("::", "/")
        } else if trimmed.starts_with("self::") || trimmed.starts_with("super::") {
            // relative to current file's module — approximate: use from_dir
            let last = trimmed.split("::").last().unwrap_or("").to_string();
            last
        } else {
            // Unknown absolute — try as file path
            trimmed.replace("::", "/")
        };
        let from_dir = from_path.rsplitn(2, '/').collect::<Vec<_>>();
        let base_dir = if from_dir.len() == 2 { from_dir[1] } else { "" };
        // Try candidates: src/<path>.rs, <base>/<path>.rs, etc.
        let candidates = [
            format!("src/{}.rs", path_part),
            format!("{}/{}.rs", base_dir, path_part),
            format!("{}.rs", path_part),
            format!("{}/{}/mod.rs", base_dir, path_part),
            format!("src/{}/mod.rs", path_part),
        ];
        for cand in candidates {
            let n = normalize_posix(&cand);
            if existing.contains(&n) {
                return Some(n);
            }
        }
        // Try single file part
        let simple = path_part.split('/').last().unwrap_or(&path_part).to_string();
        let mod_candidates = [
            format!("{}/{}.rs", base_dir, simple),
            format!("{}/mod.rs", simple),
            format!("src/{}.rs", simple),
        ];
        for cand in mod_candidates {
            let n = normalize_posix(&cand);
            if existing.contains(&n) {
                return Some(n);
            }
        }
        None
    } else {
        // single mod name
        let from_dir = from_path.rsplitn(2, '/').collect::<Vec<_>>();
        let base_dir = if from_dir.len() == 2 { from_dir[1] } else { "" };
        let candidates = [
            format!("{}/{}.rs", base_dir, trimmed),
            format!("{}/{}/mod.rs", base_dir, trimmed),
            format!("src/{}.rs", trimmed),
            format!("{}.rs", trimmed),
        ];
        for cand in candidates {
            let n = normalize_posix(&cand);
            if existing.contains(&n) {
                return Some(n);
            }
        }
        None
    }
}

fn resolve_spec(spec: &str, from_path: &str, existing: &HashSet<String>) -> (Option<String>, Option<String>) {
    // Returns (resolved_path, note)
    let ext = from_path.split('.').last().unwrap_or("").to_lowercase();
    let resolved = match ext.as_str() {
        "ts" | "tsx" | "js" | "jsx" => resolve_ts_spec(spec, from_path, existing),
        "py" => resolve_py_spec(spec, existing),
        "rs" => resolve_rs_mod(spec, from_path, existing),
        _ => None,
    };
    if let Some(p) = resolved {
        (Some(p), None)
    } else {
        // Decide note
        let is_external = !spec.starts_with('.') && !spec.starts_with('/') && ext != "py" && ext != "rs" || {
            // bare TS import that is not resolved -> likely external
            !spec.starts_with('.') && matches!(ext.as_str(), "ts" | "tsx" | "js" | "jsx")
        };
        // For py/rs, also external if not found
        let note = if is_external && !spec.contains('/') && !spec.contains('.') {
            // single word like "react", "fs", "os", "std"
            Some("external package — not resolved locally".to_string())
        } else if spec.contains('.') && ext == "py" {
            // py may be stdlib
            Some("unresolved — file not found; may be stdlib or missing".to_string())
        } else if ext == "rs" && spec.contains("::") {
            Some("unresolved — external crate or not found".to_string())
        } else {
            Some("unresolved — file not found".to_string())
        };
        (None, note)
    }
}

// ---------------------------------------------------------------------------
// Graph building
// ---------------------------------------------------------------------------

pub fn build_dependency_graph(root: &Path) -> Result<AnalysisGraph, WorkspaceError> {
    let inventory = scan_workspace(root)?;
    let canonical_root = root.canonicalize().map_err(|_| WorkspaceError::not_found("Project not found"))?;
    let root_str = canonical_root.to_string_lossy().to_string();

    // Collect eligible files (same logic as symbol_index)
    let mut eligible: Vec<(String, PathBuf)> = Vec::new();
    let mut existing_set: HashSet<String> = HashSet::new();
    for entry in inventory.entries.iter().filter(|e| e.kind == "file") {
        let rel = &entry.path;
        if is_sensitive_file(Path::new(rel)) {
            continue;
        }
        if entry.size > MAX_PREVIEW_BYTES as u64 {
            continue;
        }
        let ext = rel.split('.').last().unwrap_or("").to_lowercase();
        let is_supported = matches!(ext.as_str(), "rs" | "ts" | "tsx" | "js" | "jsx" | "py");
        if !is_supported {
            // Track unsupported but still add to existing for resolution? For unsupported we don't need graph, but we still want existing_set for resolution target check?
            // Only add supported to eligible and existing
            continue;
        }
        // Canonical check
        let candidate = canonical_root.join(rel);
        let canonical_file = match candidate.canonicalize() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !canonical_file.starts_with(&canonical_root) {
            continue;
        }
        if !canonical_file.is_file() {
            continue;
        }
        existing_set.insert(rel.clone());
        eligible.push((rel.clone(), canonical_file));
    }

    // Apply limits
    let mut truncated = false;
    let mut unsupported = 0usize;
    // Count unsupported files that were skipped due to ext
    let unsupported_count = inventory.entries.iter().filter(|e| {
        let ext = e.path.split('.').last().unwrap_or("").to_lowercase();
        !matches!(ext.as_str(), "rs" | "ts" | "tsx" | "js" | "jsx" | "py") && e.kind == "file"
    }).count();
    unsupported = unsupported_count;

    if eligible.len() > MAX_ANALYSIS_FILES {
        eligible.truncate(MAX_ANALYSIS_FILES);
        truncated = true;
    }

    let mut total_bytes = 0usize;
    let mut nodes: Vec<FileNode> = Vec::new();
    let mut edges: Vec<DependencyEdge> = Vec::new();
    let mut file_to_index: HashMap<String, usize> = HashMap::new();

    for (rel, canon) in eligible.iter() {
        if total_bytes >= MAX_ANALYSIS_BYTES {
            truncated = true;
            break;
        }
        if edges.len() >= MAX_EDGES_TOTAL {
            truncated = true;
            break;
        }

        let content = match fs::read_to_string(canon) {
            Ok(c) => c,
            Err(_) => {
                // binary or unreadable -> skip, count as unsupported
                unsupported += 1;
                continue;
            }
        };
        if content.as_bytes().len() > MAX_PREVIEW_BYTES {
            unsupported += 1;
            continue;
        }
        if content.contains('\0') {
            unsupported += 1;
            continue;
        }
        total_bytes += content.as_bytes().len();
        if total_bytes > MAX_ANALYSIS_BYTES {
            truncated = true;
            break;
        }

        let imports_raw = extract_imports_for_file(&content, rel);
        let mut imports: Vec<DependencyEdge> = Vec::new();
        for (spec, line, kind) in imports_raw {
            if imports.len() >= MAX_EDGES_PER_FILE {
                break;
            }
            if edges.len() >= MAX_EDGES_TOTAL {
                truncated = true;
                break;
            }
            let (resolved, note) = resolve_spec(&spec, rel, &existing_set);
            let verified = resolved.is_some();
            let to = resolved.unwrap_or_else(|| spec.clone());
            // For unresolved, we still create edge with to=raw but verified false, so dependents can be incomplete
            // But we should not treat self-import as edge
            if to == *rel {
                continue;
            }
            let edge = DependencyEdge {
                from: rel.clone(),
                to: to.clone(),
                raw: spec.clone(),
                line,
                verified,
                kind: kind.clone(),
                note: note.clone(),
            };
            imports.push(edge.clone());
            edges.push(edge);
        }

        let idx = nodes.len();
        file_to_index.insert(rel.clone(), idx);
        nodes.push(FileNode {
            path: rel.clone(),
            imports: imports.clone(),
            imported_by: Vec::new(), // filled after
        });
    }

    // Build imported_by reverse map
    let mut imported_by_map: HashMap<String, Vec<String>> = HashMap::new();
    for edge in &edges {
        if edge.verified {
            imported_by_map.entry(edge.to.clone()).or_default().push(edge.from.clone());
        }
    }
    for node in &mut nodes {
        if let Some(by) = imported_by_map.get(&node.path) {
            let mut dedup = by.clone();
            dedup.sort();
            dedup.dedup();
            // Limit to prevent huge lists
            if dedup.len() > 50 {
                dedup.truncate(50);
            }
            node.imported_by = dedup;
        }
    }

    // Detect cycles (bounded DFS)
    let cycles = detect_cycles(&nodes, &edges);

    Ok(AnalysisGraph {
        version: CURRENT_VERSION,
        root: root_str,
        nodes,
        edges,
        truncated,
        unsupported,
        cycles,
        generated_at: now_millis(),
    })
}

fn detect_cycles(nodes: &[FileNode], edges: &[DependencyEdge]) -> Vec<Vec<String>> {
    // Build adjacency for verified edges only
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for edge in edges.iter().filter(|e| e.verified) {
        adj.entry(edge.from.clone()).or_default().push(edge.to.clone());
    }
    // Also ensure nodes with no outgoing still present
    for node in nodes {
        adj.entry(node.path.clone()).or_default();
    }

    let mut cycles: Vec<Vec<String>> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut stack: Vec<String> = Vec::new();
    let mut on_stack: HashSet<String> = HashSet::new();

    fn dfs(
        node: &str,
        adj: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        stack: &mut Vec<String>,
        on_stack: &mut HashSet<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        if cycles.len() >= MAX_CYCLES {
            return;
        }
        visited.insert(node.to_string());
        stack.push(node.to_string());
        on_stack.insert(node.to_string());

        if let Some(neighbors) = adj.get(node) {
            for nxt in neighbors {
                if cycles.len() >= MAX_CYCLES {
                    break;
                }
                if !visited.contains(nxt) {
                    dfs(nxt, adj, visited, stack, on_stack, cycles);
                } else if on_stack.contains(nxt) {
                    // Found cycle - extract from stack
                    if let Some(pos) = stack.iter().position(|x| x == nxt) {
                        let mut cycle = stack[pos..].to_vec();
                        cycle.push(nxt.clone());
                        // Truncate long cycles
                        if cycle.len() > MAX_CYCLE_LEN {
                            cycle.truncate(MAX_CYCLE_LEN);
                        }
                        // dedup cycle detection (simple)
                        let key = cycle.join("->");
                        if !cycles.iter().any(|c| c.join("->") == key) {
                            cycles.push(cycle);
                        }
                    }
                }
            }
        }

        stack.pop();
        on_stack.remove(node);
    }

    let mut keys: Vec<String> = adj.keys().cloned().collect();
    keys.sort();
    for k in keys {
        if cycles.len() >= MAX_CYCLES {
            break;
        }
        if !visited.contains(&k) {
            dfs(&k, &adj, &mut visited, &mut stack, &mut on_stack, &mut cycles);
        }
    }

    cycles
}

// ---------------------------------------------------------------------------
// File relationships
// ---------------------------------------------------------------------------

pub fn file_relationships(
    root: &Path,
    target: &str,
) -> Result<FileRelationships, WorkspaceError> {
    // Validate target path
    let target_norm = normalize_posix(target.trim());
    if target_norm.is_empty() {
        return Err(WorkspaceError::invalid_request("Target path is empty."));
    }
    // Use scanner validation: validate_relative_path via read? For now check sensitive and traversal
    if target_norm.contains("..") {
        return Err(WorkspaceError::invalid_request("Path traversal not allowed."));
    }
    if is_sensitive_file(Path::new(&target_norm)) {
        return Err(WorkspaceError::secret_excluded("Sensitive file."));
    }

    let graph = build_dependency_graph(root)?;
    let symbol_index = match super::symbol_index::build_symbol_index(root) {
        Ok(idx) => idx,
        Err(_) => super::symbol_index::SymbolIndex {
            version: 1,
            root: root.to_string_lossy().to_string(),
            symbols: Vec::new(),
            file_count: 0,
            truncated: false,
            unsupported: 0,
            parse_errors: 0,
            generated_at: now_millis(),
        },
    };

    // Find node for target
    let node = graph.nodes.iter().find(|n| n.path == target_norm);
    let imports = node.map(|n| n.imports.clone()).unwrap_or_default();
    let imported_by = node.map(|n| n.imported_by.clone()).unwrap_or_default();
    let unresolved: Vec<DependencyEdge> = imports.iter().filter(|e| !e.verified).cloned().collect();

    // Defined symbols
    let defined_symbols: Vec<Symbol> = symbol_index
        .symbols
        .iter()
        .filter(|s| s.path == target_norm)
        .cloned()
        .take(200)
        .collect();

    // Referenced_by: search other files for symbol names defined in target
    let mut referenced_by: Vec<Reference> = Vec::new();
    if !defined_symbols.is_empty() {
        // Build set of symbol names (filter short/common)
        let names: HashSet<String> = defined_symbols
            .iter()
            .filter(|s| s.name.len() > 2 && !matches!(s.kind, SymbolKind::Import))
            .map(|s| s.name.clone())
            .collect();
        if !names.is_empty() {
            // For each eligible file not target, search content for names
            let canonical_root = root.canonicalize().map_err(|_| WorkspaceError::not_found("Project not found"))?;
            for node in &graph.nodes {
                if node.path == target_norm {
                    continue;
                }
                if referenced_by.len() >= 50 {
                    break;
                }
                let canon = canonical_root.join(&node.path);
                let content = match fs::read_to_string(&canon) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                // Quick check: if content contains any name
                for name in &names {
                    if content.contains(name) {
                        // Find first occurrence line
                        if let Some(idx) = content.find(name) {
                            let line = line_number(&content, idx);
                            // Snippet around line (max 120 chars)
                            let snippet = content
                                .lines()
                                .nth((line as usize).saturating_sub(1))
                                .unwrap_or("")
                                .trim()
                                .chars()
                                .take(120)
                                .collect::<String>();
                            referenced_by.push(Reference {
                                path: node.path.clone(),
                                line,
                                symbol: name.clone(),
                                snippet,
                            });
                            if referenced_by.len() >= 50 {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let mut notes = Vec::new();
    if graph.truncated {
        notes.push("Graph truncated — limits hit; some relationships may be missing.".to_string());
    }
    if unresolved.len() > 0 {
        notes.push(format!("{} unresolved imports — verified=false", unresolved.len()));
    }
    let unsupported_note = if graph.unsupported > 0 {
        format!("{} unsupported files skipped", graph.unsupported)
    } else {
        String::new()
    };
    if !unsupported_note.is_empty() {
        notes.push(unsupported_note);
    }
    if defined_symbols.is_empty() {
        notes.push("No symbols indexed for this file (unsupported language or empty)".to_string());
    }

    Ok(FileRelationships {
        path: target_norm,
        imports,
        imported_by,
        defined_symbols,
        referenced_by,
        unresolved,
        notes,
    })
}

// ---------------------------------------------------------------------------
// Impact analysis
// ---------------------------------------------------------------------------

pub fn change_impact(root: &Path, target: &str) -> Result<ImpactReport, WorkspaceError> {
    let target_norm = normalize_posix(target.trim());
    if target_norm.is_empty() {
        return Err(WorkspaceError::invalid_request("Target path is empty."));
    }
    if target_norm.contains("..") {
        return Err(WorkspaceError::invalid_request("Path traversal not allowed."));
    }
    if is_sensitive_file(Path::new(&target_norm)) {
        return Err(WorkspaceError::secret_excluded("Sensitive file."));
    }

    let graph = build_dependency_graph(root)?;
    let mut adj_rev: HashMap<String, Vec<String>> = HashMap::new(); // to -> from
    let mut adj_fwd: HashMap<String, Vec<DependencyEdge>> = HashMap::new();
    for edge in &graph.edges {
        adj_rev.entry(edge.to.clone()).or_default().push(edge.from.clone());
        adj_fwd.entry(edge.from.clone()).or_default().push(edge.clone());
    }

    // BFS for direct and transitive
    let mut direct: Vec<ImpactNode> = Vec::new();
    let mut transitive: Vec<ImpactNode> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(target_norm.clone());

    // Direct dependents
    if let Some(direct_deps) = adj_rev.get(&target_norm) {
        let mut sorted = direct_deps.clone();
        sorted.sort();
        sorted.dedup();
        for dep in sorted {
            if direct.len() >= MAX_IMPACT_RESULTS {
                break;
            }
            // Find edge for evidence
            let edge = graph.edges.iter().find(|e| e.from == dep && e.to == target_norm);
            let (evidence, line, confidence) = if let Some(e) = edge {
                (
                    format!("{} imports {} at line {}", e.from, e.to, e.line),
                    Some(e.line),
                    if e.verified { "verified".to_string() } else { "inferred".to_string() },
                )
            } else {
                (format!("{} depends on {}", dep, target_norm), None, "inferred".to_string())
            };
            direct.push(ImpactNode {
                path: dep.clone(),
                reason: format!("imports {}", target_norm),
                evidence,
                line,
                confidence,
            });
            visited.insert(dep.clone());
        }
    }

    // Transitive: BFS depth 2..MAX_TRAVERSAL_DEPTH from direct nodes following reverse edges
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    for d in &direct {
        queue.push_back((d.path.clone(), 1));
    }
    let mut truncated = false;
    while let Some((cur, depth)) = queue.pop_front() {
        if depth >= MAX_TRAVERSAL_DEPTH {
            continue;
        }
        if let Some(next_rev) = adj_rev.get(&cur) {
            let mut sorted = next_rev.clone();
            sorted.sort();
            sorted.dedup();
            for nxt in sorted {
                if visited.contains(&nxt) {
                    continue;
                }
                if transitive.len() + direct.len() >= MAX_IMPACT_RESULTS {
                    truncated = true;
                    break;
                }
                let evidence = format!("{} (transitively via {})", nxt, cur);
                transitive.push(ImpactNode {
                    path: nxt.clone(),
                    reason: format!("transitively depends via {}", cur),
                    evidence,
                    line: None,
                    confidence: "inferred".to_string(), // transitive is inferred
                });
                visited.insert(nxt.clone());
                queue.push_back((nxt, depth + 1));
            }
        }
        if transitive.len() + direct.len() >= MAX_IMPACT_RESULTS {
            truncated = true;
            break;
        }
    }

    if direct.len() + transitive.len() >= MAX_IMPACT_RESULTS {
        truncated = true;
    }

    // Also consider files that reference symbols defined in target (using symbol search heuristic)
    // We add those as potential risks if not already in direct/transitive
    let symbol_index = match super::symbol_index::build_symbol_index(root) {
        Ok(idx) => idx,
        Err(_) => super::symbol_index::SymbolIndex {
            version: 1,
            root: root.to_string_lossy().to_string(),
            symbols: Vec::new(),
            file_count: 0,
            truncated: false,
            unsupported: 0,
            parse_errors: 0,
            generated_at: now_millis(),
        },
    };
    let defined: Vec<String> = symbol_index
        .symbols
        .iter()
        .filter(|s| s.path == target_norm && s.name.len() > 2)
        .map(|s| s.name.clone())
        .take(20)
        .collect();
    if !defined.is_empty() && direct.len() + transitive.len() < MAX_IMPACT_RESULTS {
        let canonical_root = root.canonicalize().map_err(|_| WorkspaceError::not_found("Project not found"))?;
        for node in &graph.nodes {
            if visited.contains(&node.path) || node.path == target_norm {
                continue;
            }
            if direct.len() + transitive.len() >= MAX_IMPACT_RESULTS {
                truncated = true;
                break;
            }
            let canon = canonical_root.join(&node.path);
            let content = match fs::read_to_string(&canon) {
                Ok(c) => c,
                Err(_) => continue,
            };
            for name in &defined {
                if content.contains(name) {
                    if let Some(idx) = content.find(name) {
                        let line = line_number(&content, idx);
                        let evidence = format!("mentions symbol `{}` at {}:{}", name, node.path, line);
                        let reason = format!("references symbol `{}` from {}", name, target_norm);
                        // Only add if not already
                        if !direct.iter().any(|n| n.path == node.path) && !transitive.iter().any(|n| n.path == node.path) {
                            transitive.push(ImpactNode {
                                path: node.path.clone(),
                                reason,
                                evidence,
                                line: Some(line),
                                confidence: "inferred".to_string(),
                            });
                        }
                    }
                    break; // one per file
                }
            }
        }
    }

    let mut risks = Vec::new();
    if !graph.cycles.is_empty() {
        for c in &graph.cycles {
            if c.contains(&target_norm) {
                risks.push(format!("Target participates in circular dependency: {}", c.join(" -> ")));
            }
        }
        if risks.is_empty() && !graph.cycles.is_empty() {
            risks.push(format!("Project has {} circular dependencies; changes may propagate unexpectedly", graph.cycles.len()));
        }
    }
    if direct.len() + transitive.len() == 0 {
        risks.push("No dependents found — change appears isolated (verified imports only)".to_string());
    } else {
        risks.push(format!("{} direct + {} transitive potential dependents — treat as potential risks, not guaranteed".to_string(), direct.len(), transitive.len()));
    }
    risks.push("Static analysis does not prove runtime behavior — verify with tests".to_string());

    let mut notes = Vec::new();
    if graph.truncated {
        notes.push("Graph truncated — results may be incomplete".to_string());
    }
    if graph.unsupported > 0 {
        notes.push(format!("{} unsupported files excluded", graph.unsupported));
    }
    notes.push("Confidence: verified = resolved import exists; inferred = heuristic (name match, transitive)".to_string());

    Ok(ImpactReport {
        target: target_norm,
        direct,
        transitive,
        risks,
        truncated,
        notes,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(dir: &TempDir, rel: &str, content: &str) {
        let p = dir.path().join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    #[test]
    fn extract_ts_imports() {
        let content = r#"import { foo } from "./bar";
import * as x from "../utils";
import React from "react";
const y = require("./c");
import("./dynamic");
export * from "./other";
"#;
        let imports = extract_imports_ts(content, "src/app.ts");
        assert!(imports.iter().any(|(s, _, _)| s == "./bar"));
        assert!(imports.iter().any(|(s, _, _)| s == "../utils"));
        assert!(imports.iter().any(|(s, _, _)| s == "react"));
        assert!(imports.iter().any(|(s, _, _)| s == "./c"));
    }

    #[test]
    fn resolve_relative_ts() {
        let mut set = HashSet::new();
        set.insert("src/bar.ts".to_string());
        set.insert("src/utils/index.ts".to_string());
        // ./bar from src/app.ts -> src/bar.ts
        let r = resolve_ts_spec("./bar", "src/app.ts", &set);
        assert_eq!(r, Some("src/bar.ts".to_string()));
        // ../utils from src/app.ts -> src/utils not found, but ../utils would be src? actually src/app.ts + ../utils = utils, not existing
        // utils/index.ts is not matched via ../utils? Let's see: from src/app.ts, ../utils -> utils -> candidate utils.ts, utils/index.ts -> utils/index.ts not in set (set has src/utils/index.ts)
        let r2 = resolve_ts_spec("../utils", "src/components/foo.ts", &set);
        // from src/components/foo.ts + ../utils = src/utils -> src/utils/index.ts should match
        // Our set has src/utils/index.ts, so should resolve to src/utils/index.ts
        // Check
        assert_eq!(r2, Some("src/utils/index.ts".to_string()));
        // external
        let (res, note) = resolve_spec("react", "src/app.ts", &set);
        assert!(res.is_none());
        assert!(note.unwrap().contains("external"));
    }

    #[test]
    fn graph_simple_chain() {
        let dir = TempDir::new().unwrap();
        write(&dir, "src/a.ts", r#"import { b } from "./b"; export const a = 1;"#);
        write(&dir, "src/b.ts", r#"import { c } from "./c"; export const b = 1;"#);
        write(&dir, "src/c.ts", r#"export const c = 1;"#);
        let graph = build_dependency_graph(dir.path()).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        // a->b, b->c
        assert!(graph.edges.iter().any(|e| e.from == "src/a.ts" && e.to == "src/b.ts" && e.verified));
        assert!(graph.edges.iter().any(|e| e.from == "src/b.ts" && e.to == "src/c.ts" && e.verified));
        // imported_by
        let b_node = graph.nodes.iter().find(|n| n.path == "src/b.ts").unwrap();
        assert!(b_node.imported_by.contains(&"src/a.ts".to_string()));
        assert!(graph.cycles.is_empty());
    }

    #[test]
    fn detects_cycle() {
        let dir = TempDir::new().unwrap();
        write(&dir, "a.ts", r#"import { b } from "./b";"#);
        write(&dir, "b.ts", r#"import { a } from "./a";"#);
        let graph = build_dependency_graph(dir.path()).unwrap();
        assert!(!graph.cycles.is_empty());
        let cycle = &graph.cycles[0];
        assert!(cycle.contains(&"a.ts".to_string()));
        assert!(cycle.contains(&"b.ts".to_string()));
    }

    #[test]
    fn unresolved_import() {
        let dir = TempDir::new().unwrap();
        write(&dir, "src/app.ts", r#"import { x } from "./missing"; import React from "react";"#);
        let graph = build_dependency_graph(dir.path()).unwrap();
        let node = graph.nodes.iter().find(|n| n.path == "src/app.ts").unwrap();
        assert_eq!(node.imports.len(), 2);
        let unresolved = node.imports.iter().filter(|e| !e.verified).count();
        assert_eq!(unresolved, 2);
        // first should be unresolved file not found, second external
        assert!(node.imports.iter().any(|e| e.note.as_ref().unwrap().contains("not found")));
        assert!(node.imports.iter().any(|e| e.note.as_ref().unwrap().contains("external")));
    }

    #[test]
    fn impact_direct_and_transitive() {
        let dir = TempDir::new().unwrap();
        write(&dir, "src/base.ts", r#"export function base() {}"#);
        write(&dir, "src/mid.ts", r#"import { base } from "./base"; export const mid = base();"#);
        write(&dir, "src/top.ts", r#"import { mid } from "./mid"; const x = mid;"#);
        write(&dir, "src/unrelated.ts", r#"export const y = 1;"#);
        let impact = change_impact(dir.path(), "src/base.ts").unwrap();
        // direct should contain mid
        assert!(impact.direct.iter().any(|n| n.path == "src/mid.ts"));
        // transitive should contain top via mid
        assert!(impact.transitive.iter().any(|n| n.path == "src/top.ts"));
        assert!(!impact.direct.iter().any(|n| n.path == "src/unrelated.ts"));
        assert!(impact.risks.iter().any(|r| r.contains("potential")));
    }

    #[test]
    fn relationships_referenced_by() {
        let dir = TempDir::new().unwrap();
        write(&dir, "src/foo.ts", r#"export function greet() { return "hi"; }"#);
        write(&dir, "src/bar.ts", r#"import { greet } from "./foo"; console.log(greet());"#);
        write(&dir, "src/baz.ts", r#"const x = greet(); // no import but mentions"#);
        let rel = file_relationships(dir.path(), "src/foo.ts").unwrap();
        assert!(rel.defined_symbols.iter().any(|s| s.name == "greet"));
        // bar should be referenced_by via name search, also imported_by via graph
        assert!(rel.imported_by.contains(&"src/bar.ts".to_string()));
        // referenced_by should include bar and maybe baz (inferred via name)
        assert!(rel.referenced_by.iter().any(|r| r.path == "src/bar.ts"));
    }

    #[test]
    fn sensitive_excluded() {
        let dir = TempDir::new().unwrap();
        write(&dir, ".env", "SECRET=1");
        write(&dir, "src/app.ts", r#"import x from "./secret";"#);
        write(&dir, "src/secret.ts", "export const s=1;");
        // Manually create .env as sensitive should be excluded from graph
        let graph = build_dependency_graph(dir.path()).unwrap();
        assert!(!graph.nodes.iter().any(|n| n.path == ".env"));
        assert!(!graph.nodes.iter().any(|n| n.path.contains(".env")));
        // impact on sensitive should error
        let err = change_impact(dir.path(), ".env").unwrap_err();
        assert_eq!(err.code, "secret_excluded");
    }

    #[test]
    fn python_and_rust_imports() {
        let dir = TempDir::new().unwrap();
        write(&dir, "main.py", "import os\nimport mymod\nfrom pkg import foo\n");
        write(&dir, "mymod.py", "x=1");
        write(&dir, "pkg/__init__.py", "");
        write(&dir, "pkg/foo.py", "y=1");
        write(&dir, "src/main.rs", "use crate::utils::helper;\nmod utils;\n");
        write(&dir, "src/utils.rs", "pub fn helper() {}");
        let graph = build_dependency_graph(dir.path()).unwrap();
        // py mymod should be verified (main.py -> mymod.py)
        let py_node = graph.nodes.iter().find(|n| n.path == "main.py").unwrap();
        // os is external/unresolved
        assert!(py_node.imports.iter().any(|e| e.raw == "os" && !e.verified));
        // mymod is verified if exists? Our py resolve tries mymod.py
        // Check
        let mymod_edge = py_node.imports.iter().find(|e| e.raw == "mymod").unwrap();
        // mymod.py exists at root, so should be verified
        assert!(mymod_edge.verified);
        // rust mod utils should be verified (src/main.rs -> src/utils.rs)
        let rs_node = graph.nodes.iter().find(|n| n.path == "src/main.rs").unwrap();
        assert!(rs_node.imports.iter().any(|e| e.verified));
    }

    #[test]
    fn limits_truncated() {
        let dir = TempDir::new().unwrap();
        for i in 0..600 {
            write(&dir, &format!("src/f{}.ts", i), &format!(r#"import {{ x }} from "./f{}";"#, (i+1)%600));
        }
        let graph = build_dependency_graph(dir.path()).unwrap();
        assert!(graph.nodes.len() <= MAX_ANALYSIS_FILES);
        assert!(graph.truncated);
        assert!(graph.edges.len() <= MAX_EDGES_TOTAL);
    }

    #[test]
    fn unsupported_language_skipped() {
        let dir = TempDir::new().unwrap();
        write(&dir, "README.md", "# hello");
        write(&dir, "data.json", "{}");
        write(&dir, "src/app.ts", "export const a=1;");
        let graph = build_dependency_graph(dir.path()).unwrap();
        // md and json are unsupported, not in nodes
        assert!(!graph.nodes.iter().any(|n| n.path == "README.md"));
        assert!(graph.unsupported >= 2);
    }

    #[test]
    fn traversal_protected() {
        let dir = TempDir::new().unwrap();
        write(&dir, "src/app.ts", "export const a=1;");
        let err = file_relationships(dir.path(), "../outside.ts").unwrap_err();
        assert_eq!(err.code, "invalid_request");
        let err2 = change_impact(dir.path(), "../outside.ts").unwrap_err();
        assert_eq!(err2.code, "invalid_request");
    }

    #[test]
    fn no_circular_for_chain() {
        let dir = TempDir::new().unwrap();
        write(&dir, "a.ts", r#"import "./b";"#);
        write(&dir, "b.ts", r#"import "./c";"#);
        write(&dir, "c.ts", r#"export const x=1;"#);
        let graph = build_dependency_graph(dir.path()).unwrap();
        assert!(graph.cycles.is_empty());
    }
}
