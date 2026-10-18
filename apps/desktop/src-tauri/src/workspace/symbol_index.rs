use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::scanner::{is_sensitive_file, scan_workspace, WorkspaceError, MAX_PREVIEW_BYTES};

// Limits
pub const MAX_INDEXED_FILES: usize = 500;
pub const MAX_INDEXED_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_SYMBOLS_PER_FILE: usize = 200;
pub const MAX_SYMBOLS_TOTAL: usize = 5000;
pub const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Interface,
    Type,
    Struct,
    Enum,
    Trait,
    Constant,
    Import,
    Export,
    Module,
    File,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Type => "type",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Constant => "constant",
            Self::Import => "import",
            Self::Export => "export",
            Self::Module => "module",
            Self::File => "file",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub path: String, // workspace-relative posix
    pub start_line: u32,
    pub end_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolIndex {
    pub version: u32,
    pub root: String,
    pub symbols: Vec<Symbol>,
    pub file_count: usize,
    pub truncated: bool,
    pub unsupported: usize,
    pub parse_errors: usize,
    pub generated_at: u64,
}

impl Default for SymbolIndex {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            root: String::new(),
            symbols: Vec::new(),
            file_count: 0,
            truncated: false,
            unsupported: 0,
            parse_errors: 0,
            generated_at: now_millis(),
        }
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn is_supported_ext(ext: &str) -> bool {
    matches!(ext, "rs" | "ts" | "tsx" | "js" | "jsx" | "py")
}

fn language_for_path(path: &str) -> &'static str {
    let ext = path.split('.').last().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "ts" => "typescript",
        "tsx" => "tsx",
        "js" => "javascript",
        "jsx" => "jsx",
        "py" => "python",
        _ => "unsupported",
    }
}

// Helper to compute line number from byte offset
fn line_number(content: &str, offset: usize) -> u32 {
    content[..offset.min(content.len())].chars().filter(|&c| c == '\n').count() as u32 + 1
}

fn push_symbol(
    out: &mut Vec<Symbol>,
    name: String,
    kind: SymbolKind,
    path: &str,
    start_line: u32,
    detail: Option<String>,
) {
    if out.len() >= MAX_SYMBOLS_TOTAL {
        return;
    }
    // sanitize name length
    let name = if name.len() > 200 { name[..200].to_string() } else { name };
    let detail = detail.map(|d| if d.len() > 300 { d[..300].to_string() } else { d });
    out.push(Symbol {
        name,
        kind,
        path: path.to_string(),
        start_line,
        end_line: start_line,
        detail,
    });
}

fn extract_rust(content: &str, path: &str, out: &mut Vec<Symbol>) {
    // Order matters: more specific first
    let patterns: &[(&str, SymbolKind)] = &[
        (r"(?m)^\s*pub\s+(?:unsafe\s+)?fn\s+(\w+)", SymbolKind::Function),
        (r"(?m)^\s*fn\s+(\w+)", SymbolKind::Function),
        (r"(?m)^\s*pub\s+struct\s+(\w+)", SymbolKind::Struct),
        (r"(?m)^\s*struct\s+(\w+)", SymbolKind::Struct),
        (r"(?m)^\s*pub\s+enum\s+(\w+)", SymbolKind::Enum),
        (r"(?m)^\s*enum\s+(\w+)", SymbolKind::Enum),
        (r"(?m)^\s*pub\s+trait\s+(\w+)", SymbolKind::Trait),
        (r"(?m)^\s*trait\s+(\w+)", SymbolKind::Trait),
        (r"(?m)^\s*pub\s+type\s+(\w+)", SymbolKind::Type),
        (r"(?m)^\s*type\s+(\w+)", SymbolKind::Type),
        (r"(?m)^\s*pub\s*mod\s+(\w+)", SymbolKind::Module),
        (r"(?m)^\s*mod\s+(\w+)", SymbolKind::Module),
        (r"(?m)^\s*pub\s+const\s+(\w+)", SymbolKind::Constant),
        (r"(?m)^\s*const\s+(\w+)", SymbolKind::Constant),
        (r"(?m)^\s*use\s+([^;]+);", SymbolKind::Import),
    ];
    for (pat, kind) in patterns {
        if out.len() >= MAX_SYMBOLS_PER_FILE {
            break;
        }
        let re = Regex::new(pat).unwrap();
        for mat in re.captures_iter(content) {
            if out.len() >= MAX_SYMBOLS_PER_FILE {
                break;
            }
            let m = mat.get(0).unwrap();
            let line = line_number(content, m.start());
            let name = mat.get(1).map(|g| g.as_str().trim().to_string()).unwrap_or_else(|| m.as_str().trim().to_string());
            // For imports, name is the whole import path, truncate
            let detail = content[m.start()..m.end().min(m.start()+200)].lines().next().map(|s| s.trim().to_string());
            push_symbol(out, name, kind.clone(), path, line, detail);
            if out.len() >= MAX_SYMBOLS_PER_FILE {
                break;
            }
        }
    }
}

fn extract_typescript(content: &str, path: &str, out: &mut Vec<Symbol>) {
    let patterns: &[(&str, SymbolKind)] = &[
        (r"(?m)^\s*export\s+(?:async\s+)?function\s+(\w+)", SymbolKind::Function),
        (r"(?m)^\s*function\s+(\w+)", SymbolKind::Function),
        (r"(?m)^\s*export\s+class\s+(\w+)", SymbolKind::Class),
        (r"(?m)^\s*class\s+(\w+)", SymbolKind::Class),
        (r"(?m)^\s*export\s+interface\s+(\w+)", SymbolKind::Interface),
        (r"(?m)^\s*interface\s+(\w+)", SymbolKind::Interface),
        (r"(?m)^\s*export\s+type\s+(\w+)", SymbolKind::Type),
        (r"(?m)^\s*type\s+(\w+)\s*=", SymbolKind::Type),
        (r"(?m)^\s*export\s+enum\s+(\w+)", SymbolKind::Enum),
        (r"(?m)^\s*enum\s+(\w+)", SymbolKind::Enum),
        (r"(?m)^\s*import\s+.*from\s+['\"]([^'\"]+)['\"]", SymbolKind::Import),
        (r"(?m)^\s*import\s+\{([^}]+)\}", SymbolKind::Import),
        (r"(?m)export\s+(?:const|let|var)\s+(\w+)", SymbolKind::Export),
        (r"(?m)^\s*const\s+(\w+)\s*=\s*(?:async\s*)?\([^)]*\)\s*=>", SymbolKind::Function),
    ];
    for (pat, kind) in patterns {
        if out.len() >= MAX_SYMBOLS_PER_FILE {
            break;
        }
        let re = match Regex::new(pat) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for mat in re.captures_iter(content) {
            if out.len() >= MAX_SYMBOLS_PER_FILE {
                break;
            }
            let m = mat.get(0).unwrap();
            let line = line_number(content, m.start());
            let name = mat.get(1).map(|g| g.as_str().trim().to_string()).unwrap_or_else(|| m.as_str().trim().to_string());
            let detail = content[m.start()..m.end().min(m.start()+200)].lines().next().map(|s| s.trim().to_string());
            push_symbol(out, name, kind.clone(), path, line, detail);
        }
    }
}

fn extract_python(content: &str, path: &str, out: &mut Vec<Symbol>) {
    let patterns: &[(&str, SymbolKind)] = &[
        (r"(?m)^\s*def\s+(\w+)\s*\(", SymbolKind::Function),
        (r"(?m)^\s*class\s+(\w+)", SymbolKind::Class),
        (r"(?m)^\s*import\s+(\w+)", SymbolKind::Import),
        (r"(?m)^\s*from\s+(\w+)\s+import", SymbolKind::Import),
    ];
    for (pat, kind) in patterns {
        if out.len() >= MAX_SYMBOLS_PER_FILE {
            break;
        }
        let re = match Regex::new(pat) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for mat in re.captures_iter(content) {
            if out.len() >= MAX_SYMBOLS_PER_FILE {
                break;
            }
            let m = mat.get(0).unwrap();
            let line = line_number(content, m.start());
            let name = mat.get(1).map(|g| g.as_str().trim().to_string()).unwrap_or_else(|| m.as_str().trim().to_string());
            let detail = content[m.start()..m.end().min(m.start()+200)].lines().next().map(|s| s.trim().to_string());
            push_symbol(out, name, kind.clone(), path, line, detail);
        }
    }
}

pub fn extract_symbols_for_file(content: &str, path: &str) -> (Vec<Symbol>, bool) {
    let ext = path.split('.').last().unwrap_or("").to_lowercase();
    let mut out = Vec::new();
    let supported = is_supported_ext(&ext);
    if !supported {
        return (out, false);
    }
    // Avoid panicking on malformed regex or content
    let result = std::panic::catch_unwind(|| {
        let mut inner = Vec::new();
        match ext.as_str() {
            "rs" => extract_rust(content, path, &mut inner),
            "ts" | "tsx" | "js" | "jsx" => extract_typescript(content, path, &mut inner),
            "py" => extract_python(content, path, &mut inner),
            _ => {}
        }
        inner
    });
    match result {
        Ok(v) => (v, true),
        Err(_) => (out, false),
    }
}

pub fn build_symbol_index(root: &Path) -> Result<SymbolIndex, WorkspaceError> {
    // Use scan_workspace to get eligible files
    let inventory = scan_workspace(root)?;
    let canonical_root = root.canonicalize().map_err(|_| WorkspaceError::not_found("Project not found"))?;
    let root_str = canonical_root.to_string_lossy().to_string();

    let mut symbols = Vec::new();
    let mut file_count = 0usize;
    let mut total_bytes = 0usize;
    let mut unsupported = 0usize;
    let mut parse_errors = 0usize;
    let mut truncated = false;

    for entry in inventory.entries.iter().filter(|e| e.kind == "file") {
        if file_count >= MAX_INDEXED_FILES {
            truncated = true;
            break;
        }
        if total_bytes >= MAX_INDEXED_BYTES {
            truncated = true;
            break;
        }
        if symbols.len() >= MAX_SYMBOLS_TOTAL {
            truncated = true;
            break;
        }

        let rel_path = &entry.path;
        // Double-check sensitive (scan already does, but safe)
        if is_sensitive_file(Path::new(rel_path)) {
            continue;
        }

        // Check extension supported
        let ext = rel_path.split('.').last().unwrap_or("").to_lowercase();
        if !is_supported_ext(&ext) {
            unsupported += 1;
            continue;
        }

        // Size check before reading
        if entry.size > MAX_PREVIEW_BYTES as u64 {
            unsupported += 1;
            continue;
        }

        // Read file securely via canonical check
        let candidate = canonical_root.join(rel_path);
        let canonical_file = match candidate.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                parse_errors += 1;
                continue;
            }
        };
        if !canonical_file.starts_with(&canonical_root) {
            continue;
        }
        if !canonical_file.is_file() {
            continue;
        }
        // Check symlink outside already via scan, but re-check
        let content = match fs::read_to_string(&canonical_file) {
            Ok(c) => c,
            Err(_) => {
                // Try to detect binary
                match fs::read(&canonical_file) {
                    Ok(bytes) => {
                        if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
                            unsupported += 1;
                        } else {
                            parse_errors += 1;
                        }
                    }
                    Err(_) => parse_errors += 1,
                }
                continue;
            }
        };

        if content.as_bytes().len() as u64 > MAX_PREVIEW_BYTES {
            unsupported += 1;
            continue;
        }
        if content.contains('\0') {
            unsupported += 1;
            continue;
        }

        total_bytes += content.as_bytes().len();
        if total_bytes > MAX_INDEXED_BYTES {
            truncated = true;
            break;
        }

        file_count += 1;

        let (mut file_symbols, ok) = extract_symbols_for_file(&content, rel_path);
        if !ok {
            parse_errors += 1;
        }
        // Truncate per-file already handled, but also ensure total
        if symbols.len() + file_symbols.len() > MAX_SYMBOLS_TOTAL {
            let remaining = MAX_SYMBOLS_TOTAL - symbols.len();
            file_symbols.truncate(remaining);
            truncated = true;
        }
        symbols.extend(file_symbols);

        if symbols.len() >= MAX_SYMBOLS_TOTAL {
            truncated = true;
            break;
        }
    }

    Ok(SymbolIndex {
        version: CURRENT_VERSION,
        root: root_str,
        symbols,
        file_count,
        truncated,
        unsupported,
        parse_errors,
        generated_at: now_millis(),
    })
}

pub fn search_symbols(index: &SymbolIndex, query: &str, limit: usize) -> Vec<Symbol> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let lim = limit.min(50);
    let mut exact = Vec::new();
    let mut prefix = Vec::new();
    let mut substring = Vec::new();
    let mut path_match = Vec::new();

    for sym in &index.symbols {
        let name_lower = sym.name.to_lowercase();
        let path_lower = sym.path.to_lowercase();
        if name_lower == q {
            exact.push(sym.clone());
        } else if name_lower.starts_with(&q) {
            prefix.push(sym.clone());
        } else if name_lower.contains(&q) {
            substring.push(sym.clone());
        } else if path_lower.contains(&q) {
            path_match.push(sym.clone());
        }
        if exact.len() + prefix.len() + substring.len() + path_match.len() >= lim * 2 {
            // early break to keep deterministic but not too large
            if exact.len() >= lim {
                break;
            }
        }
    }

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for group in [exact, prefix, substring, path_match] {
        for sym in group {
            let key = format!("{}:{}", sym.path, sym.name);
            if seen.contains(&key) {
                continue;
            }
            seen.insert(key.clone());
            out.push(sym);
            if out.len() >= lim {
                break;
            }
        }
        if out.len() >= lim {
            break;
        }
    }

    // Deterministic: sort by path then name, but preserve ranking groups? Keep as collected which is already grouped by rank
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn extracts_rust_symbols() {
        let content = r#"
            use std::fs;
            pub fn hello() {}
            struct Foo;
            enum Bar { A, B }
            trait Baz { fn do_it(&self); }
            type Alias = String;
            mod inner {}
            const VAL: i32 = 1;
        "#;
        let (syms, ok) = extract_symbols_for_file(content, "src/main.rs");
        assert!(ok);
        assert!(syms.iter().any(|s| s.name == "hello" && s.kind == SymbolKind::Function));
        assert!(syms.iter().any(|s| s.name == "Foo" && s.kind == SymbolKind::Struct));
        assert!(syms.iter().any(|s| s.name == "Bar" && s.kind == SymbolKind::Enum));
        assert!(syms.iter().any(|s| s.name == "Baz" && s.kind == SymbolKind::Trait));
        assert!(syms.iter().any(|s| s.kind == SymbolKind::Import));
    }

    #[test]
    fn extracts_typescript_symbols() {
        let content = r#"
            import { foo } from "./bar";
            export function greet() {}
            export class MyClass {}
            export interface Props { x: number }
            export type MyType = string;
            enum MyEnum { A }
            const arrow = (x: number) => x + 1;
        "#;
        let (syms, ok) = extract_symbols_for_file(content, "src/app.ts");
        assert!(ok);
        assert!(syms.iter().any(|s| s.name == "greet"));
        assert!(syms.iter().any(|s| s.name == "MyClass" && s.kind == SymbolKind::Class));
        assert!(syms.iter().any(|s| s.name == "Props" && s.kind == SymbolKind::Interface));
    }

    #[test]
    fn extracts_python_symbols() {
        let content = "import os\ndef foo():\n    pass\nclass Bar:\n    def method(self):\n        pass\n";
        let (syms, ok) = extract_symbols_for_file(content, "main.py");
        assert!(ok);
        assert!(syms.iter().any(|s| s.name == "foo"));
        assert!(syms.iter().any(|s| s.name == "Bar" && s.kind == SymbolKind::Class));
    }

    #[test]
    fn marks_unsupported() {
        let (syms, ok) = extract_symbols_for_file("{}", "data.json");
        assert!(!ok);
        assert!(syms.is_empty());
    }

    #[test]
    fn handles_malformed_without_crash() {
        let content = "fn broken(\n struct {\n";
        let (syms, ok) = extract_symbols_for_file(content, "src/bad.rs");
        assert!(ok); // still returns ok, may have partial
        let _ = syms;
    }

    #[test]
    fn line_numbers_reliable() {
        let content = "line1\nline2\nfn hello() {}\n";
        let (syms, _) = extract_symbols_for_file(content, "a.rs");
        let sym = syms.iter().find(|s| s.name == "hello").expect("hello");
        assert_eq!(sym.start_line, 3);
    }

    #[test]
    fn sensitive_excluded_in_build() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "SECRET=1").unwrap();
        fs::write(dir.path().join("main.rs"), "fn ok() {}").unwrap();
        let idx = build_symbol_index(dir.path()).expect("build");
        assert!(idx.symbols.iter().any(|s| s.name == "ok"));
        assert!(!idx.symbols.iter().any(|s| s.path.contains(".env")));
    }

    #[test]
    fn binary_and_oversized_rejected() {
        let dir = TempDir::new().unwrap();
        let mut big = "fn a() {}\n".repeat(10000);
        big.push_str(&"a".repeat(300_000));
        fs::write(dir.path().join("big.rs"), big).unwrap();
        fs::write(dir.path().join("image.png"), [0u8, 1, 2, 3]).unwrap();
        let idx = build_symbol_index(dir.path()).expect("build");
        // big file oversized should be unsupported, binary unsupported
        assert!(idx.unsupported >= 1);
    }

    #[test]
    fn file_count_limit() {
        let dir = TempDir::new().unwrap();
        for i in 0..600 {
            fs::write(dir.path().join(format!("f{}.rs", i)), "fn foo() {}").unwrap();
        }
        let idx = build_symbol_index(dir.path()).expect("build");
        assert!(idx.file_count <= MAX_INDEXED_FILES);
        assert!(idx.truncated);
    }

    #[test]
    fn search_ranking() {
        let mut idx = SymbolIndex::default();
        idx.symbols = vec![
            Symbol { name: "hello".into(), kind: SymbolKind::Function, path: "a.rs".into(), start_line: 1, end_line: 1, detail: None },
            Symbol { name: "hello_world".into(), kind: SymbolKind::Function, path: "b.rs".into(), start_line: 1, end_line: 1, detail: None },
            Symbol { name: "world_hello".into(), kind: SymbolKind::Function, path: "c.rs".into(), start_line: 1, end_line: 1, detail: None },
            Symbol { name: "other".into(), kind: SymbolKind::Function, path: "hello.rs".into(), start_line: 1, end_line: 1, detail: None },
        ];
        let res = search_symbols(&idx, "hello", 10);
        assert_eq!(res[0].name, "hello"); // exact
        assert_eq!(res[1].name, "hello_world"); // prefix
        assert!(res.iter().any(|s| s.name == "world_hello"));
        assert!(res.iter().any(|s| s.path == "hello.rs"));
    }

    #[test]
    fn workspace_switch_invalidation() {
        let dir1 = TempDir::new().unwrap();
        let dir2 = TempDir::new().unwrap();
        fs::write(dir1.path().join("a.rs"), "fn foo() {}").unwrap();
        fs::write(dir2.path().join("b.rs"), "fn bar() {}").unwrap();
        let idx1 = build_symbol_index(dir1.path()).expect("idx1");
        let idx2 = build_symbol_index(dir2.path()).expect("idx2");
        assert_ne!(idx1.root, idx2.root);
        assert!(idx1.symbols.iter().any(|s| s.name == "foo"));
        assert!(idx2.symbols.iter().any(|s| s.name == "bar"));
    }

    #[test]
    fn symlink_traversal_protected() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        fs::write(outside.path().join("secret.rs"), "fn secret() {}").unwrap();
        // Create symlink inside workspace pointing outside
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link = dir.path().join("link.rs");
            let _ = symlink(outside.path().join("secret.rs"), &link);
            let idx = build_symbol_index(dir.path()).expect("build");
            // Should not index the symlink outside
            assert!(!idx.symbols.iter().any(|s| s.name == "secret"));
        }
    }
}
