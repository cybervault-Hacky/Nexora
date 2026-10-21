use std::fs;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::patch::{hash_content, PatchProposal};
use super::scanner::{
    canonicalize_root, is_sensitive_file, validate_relative_path, MAX_PATH_CHARS, MAX_PREVIEW_BYTES,
};

// ---------------------------------------------------------------------------
// Types — public API for frontend
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Passed,
    Failed,
    NotRun,
    Unsupported,
    Inconclusive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub path: String,
    pub line: usize,
    pub column: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationCheck {
    pub id: String,
    pub label: String,
    pub status: CheckStatus,
    pub diagnostics: Vec<Diagnostic>,
    /// Human-readable note when NotRun/Unsupported/Inconclusive (reason)
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub overall: CheckStatus,
    pub checks: Vec<ValidationCheck>,
    pub summary: String,
    pub duration_ms: u64,
    pub proposal_id: String,
    pub proposal_path: String,
    pub generated_at: u64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn diag(path: &str, line: usize, column: Option<usize>, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.to_string(),
        line,
        column,
        message: message.into(),
    }
}

// Balanced delimiters check — in-memory, no exec, no fs write.
// Supports rust/ts/js/py generic. Reports first mismatch with line/col.
// Returns Ok(()) if balanced, Err(diagnostics) if unbalanced.
// If content too short or ambiguous, caller may treat as Inconclusive.
fn check_balanced(path: &str, content: &str) -> Result<(), Vec<Diagnostic>> {
    let mut stack: Vec<(char, usize, usize)> = Vec::new(); // char, line, col
    let mut line: usize = 1;
    let mut col: usize = 1;

    let mut in_single = false;
    let mut in_double = false;
    let mut in_backtick = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut escape = false;

    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = if i + 1 < chars.len() {
            Some(chars[i + 1])
        } else {
            None
        };

        // Track line/col before processing char (col refers to this char)
        let cur_line = line;
        let cur_col = col;

        // Handle newline
        let is_newline = c == '\n';

        // If in line comment, skip until newline
        if in_line_comment {
            if is_newline {
                in_line_comment = false;
            }
            // advance
            if is_newline {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            i += 1;
            continue;
        }

        // If in block comment, look for */
        if in_block_comment {
            if c == '*' && next == Some('/') {
                in_block_comment = false;
                i += 2;
                // update col for both chars (rough)
                col += 2;
                // handle newline inside? already consumed both; but block comment may contain newlines already counted? We counted line per char, but we skipped counting line inside comment for simplicity; but we should count newlines even inside block comment.
                // To keep line accurate, we counted newline above before this block? Actually we already handled line comment newline, but block comment newlines not counted separately.
                // For simplicity, if char was newline inside block comment we would have not counted line; fix: count newlines while in block comment as well.
                // Since we are inside block comment, we need to count newlines if c == '\n'
                // But we already skipped earlier; we need to re-add line counting for block comment path.
                // Let's handle line counting at top before checks, but for block comment we still need to count.
                // We'll just if is_newline handle line increment even inside block comment above (we did before entering this branch? No)
                // To keep simple, we already handled is_newline earlier for line comment only; for block comment we handle here.
                if is_newline {
                    line += 1;
                    col = 1;
                }
                continue;
            }
            // Count newline inside block comment
            if is_newline {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            i += 1;
            continue;
        }

        // Handle escape inside strings
        if (in_single || in_double || in_backtick) && !escape && c == '\\' {
            escape = true;
            if is_newline {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            i += 1;
            continue;
        }
        if escape {
            escape = false;
            if is_newline {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            i += 1;
            continue;
        }

        // Detect string start/end when not in comment
        if !in_single && !in_double && !in_backtick {
            // Detect comment start before string
            // For rust/ts/js: // and /* ; for py: # (but we handle generically as // and #)
            // To keep language-agnostic, treat // and # as line comment, /* as block
            if c == '/' && next == Some('/') {
                in_line_comment = true;
                i += 2;
                col += 2;
                continue;
            }
            if c == '/' && next == Some('*') {
                in_block_comment = true;
                i += 2;
                col += 2;
                continue;
            }
            if c == '#' {
                // For python and also shell; treat as line comment until newline
                // But be careful: # inside url? It's okay to treat as comment for balanced check heuristic — may be inconclusive but not fail.
                // We will treat # as line comment only if at start of line or after whitespace? For simplicity, treat as line comment.
                // However this may hide brackets inside comments, which is good (ignore).
                in_line_comment = true;
                i += 1;
                col += 1;
                continue;
            }
            if c == '\'' {
                in_single = true;
                i += 1;
                col += 1;
                continue;
            }
            if c == '"' {
                in_double = true;
                i += 1;
                col += 1;
                continue;
            }
            if c == '`' {
                in_backtick = true;
                i += 1;
                col += 1;
                continue;
            }
        } else {
            // Inside a string, check for close
            if in_single && c == '\'' {
                in_single = false;
                i += 1;
                col += 1;
                continue;
            }
            if in_double && c == '"' {
                in_double = false;
                i += 1;
                col += 1;
                continue;
            }
            if in_backtick && c == '`' {
                in_backtick = false;
                i += 1;
                col += 1;
                continue;
            }
            // Inside string, ignore brackets
            if is_newline {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            i += 1;
            continue;
        }

        // Not in string/comment — handle brackets
        match c {
            '(' | '[' | '{' => {
                stack.push((c, cur_line, cur_col));
            }
            ')' | ']' | '}' => {
                let expected = match c {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => unreachable!(),
                };
                if let Some((open, oline, ocol)) = stack.pop() {
                    if open != expected {
                        let msg = format!(
                            "Mismatched brackets: expected closing for `{}` at {}:{}, found `{}` at {}:{}",
                            open, oline, ocol, c, cur_line, cur_col
                        );
                        return Err(vec![diag(path, cur_line, Some(cur_col), msg)]);
                    }
                } else {
                    let msg = format!("Unmatched closing `{}` at {}:{}", c, cur_line, cur_col);
                    return Err(vec![diag(path, cur_line, Some(cur_col), msg)]);
                }
            }
            _ => {}
        }

        if is_newline {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
        i += 1;
    }

    // If still inside string/comment at EOF, that's a diagnostic (unclosed)
    if in_single {
        return Err(vec![diag(
            path,
            line,
            None,
            "Unclosed single-quoted string",
        )]);
    }
    if in_double {
        return Err(vec![diag(
            path,
            line,
            None,
            "Unclosed double-quoted string",
        )]);
    }
    if in_backtick {
        return Err(vec![diag(path, line, None, "Unclosed backtick string")]);
    }
    if in_block_comment {
        return Err(vec![diag(path, line, None, "Unclosed block comment /*")]);
    }

    if let Some((open, oline, ocol)) = stack.pop() {
        let msg = format!("Unclosed `{}` opened at {}:{}", open, oline, ocol);
        return Err(vec![diag(path, oline, Some(ocol), msg)]);
    }

    Ok(())
}

// Language detection for syntax check
fn ext_of(path: &str) -> String {
    path.rsplit('.').next().unwrap_or("").to_ascii_lowercase()
}

// ---------------------------------------------------------------------------
// Public validation
// ---------------------------------------------------------------------------

/// Bounded, non-mutating validation of a single-file patch proposal.
/// Does not write into `root`, does not execute project code.
/// Each check reports Passed/Failed/NotRun/Unsupported/Inconclusive with real diagnostics.
pub fn validate_detailed(root: &Path, proposal: &PatchProposal) -> ValidationReport {
    let start = Instant::now();
    let generated_at = now_millis();
    let mut checks: Vec<ValidationCheck> = Vec::new();
    let mut overall_failed = false;
    let mut overall_inconclusive = false;

    // Helper to push check and update overall
    let mut push = |check: ValidationCheck| {
        if check.status == CheckStatus::Failed {
            overall_failed = true;
        } else if check.status == CheckStatus::Inconclusive {
            overall_inconclusive = true;
        }
        checks.push(check);
    };

    // 1. Contract & schema (hash, size, binary, non-empty)
    {
        let mut diags = Vec::new();
        let mut status = CheckStatus::Passed;
        let note = None;

        // Recompute hashes
        let recomputed_original = hash_content(&proposal.original_content);
        if recomputed_original != proposal.original_hash {
            status = CheckStatus::Failed;
            diags.push(diag(
                &proposal.path,
                1,
                None,
                "Original hash does not match original content",
            ));
        }
        let recomputed_proposed = hash_content(&proposal.proposed_content);
        if recomputed_proposed != proposal.proposed_hash {
            status = CheckStatus::Failed;
            diags.push(diag(
                &proposal.path,
                1,
                None,
                "Proposed hash does not match proposed content",
            ));
        }
        if proposal.proposed_content.trim().is_empty() {
            status = CheckStatus::Failed;
            diags.push(diag(&proposal.path, 1, None, "Proposed content is empty"));
        }
        if proposal.proposed_content == proposal.original_content {
            // Not a schema failure but a warning — here we treat as Inconclusive/Failed? For contract we treat as Failed to prevent no-op?
            // Spec: contract validation should flag no changes as warning, but for detailed we mark Inconclusive with note.
            // To keep Passed/Failed clean, we mark Failed with diagnostic about no changes.
            status = CheckStatus::Failed;
            diags.push(diag(
                &proposal.path,
                1,
                None,
                "No changes detected — proposedContent equals originalContent",
            ));
        }
        if proposal.proposed_content.contains('\0') {
            status = CheckStatus::Failed;
            diags.push(diag(
                &proposal.path,
                1,
                None,
                "Proposed content contains NUL (binary)",
            ));
        }
        if proposal.proposed_content.len() as u64 > MAX_PREVIEW_BYTES {
            status = CheckStatus::Failed;
            diags.push(diag(
                &proposal.path,
                1,
                None,
                format!(
                    "Proposed content too large ({} bytes, max {})",
                    proposal.proposed_content.len(),
                    MAX_PREVIEW_BYTES
                ),
            ));
        }
        if proposal.proposed_content.chars().count() > 200_000 {
            status = CheckStatus::Failed;
            diags.push(diag(
                &proposal.path,
                1,
                None,
                "Proposed content exceeds 200,000 char limit",
            ));
        }
        if proposal.path.trim().is_empty() {
            status = CheckStatus::Failed;
            diags.push(diag(&proposal.path, 1, None, "Missing target path"));
        }

        push(ValidationCheck {
            id: "contract_schema".to_string(),
            label: "Patch contract & schema".to_string(),
            status,
            diagnostics: diags,
            note,
        });
    }

    // 2. Target path & workspace boundary
    {
        let mut diags = Vec::new();
        let mut status = CheckStatus::Passed;
        let note = None;

        if let Err(e) = validate_relative_path(&proposal.path) {
            status = CheckStatus::Failed;
            diags.push(diag(
                &proposal.path,
                1,
                None,
                format!("Invalid target path: {}", e.message),
            ));
        }
        if proposal.path.chars().count() > MAX_PATH_CHARS {
            status = CheckStatus::Failed;
            diags.push(diag(&proposal.path, 1, None, "Target path too long"));
        }
        if Path::new(&proposal.path).is_absolute() {
            status = CheckStatus::Failed;
            diags.push(diag(&proposal.path, 1, None, "Absolute path not allowed"));
        }
        if proposal.path.contains("..") {
            // validate_relative_path already checks ParentDir, but double-check string
            if status != CheckStatus::Failed {
                status = CheckStatus::Failed;
                diags.push(diag(&proposal.path, 1, None, "Path traversal not allowed"));
            }
        }

        // Canonical root + starts_with (non-mutating, only reads fs metadata for canonicalize)
        match canonicalize_root(root) {
            Ok(canonical_root) => {
                let joined = canonical_root.join(&proposal.path);
                match fs::canonicalize(&joined) {
                    Ok(canonical_file) => {
                        if !canonical_file.starts_with(&canonical_root) {
                            status = CheckStatus::Failed;
                            diags.push(diag(&proposal.path, 1, None, "File escapes workspace"));
                        } else {
                            // Check is_dir
                            if let Ok(meta) = fs::metadata(&canonical_file) {
                                if meta.is_dir() {
                                    status = CheckStatus::Failed;
                                    diags.push(diag(
                                        &proposal.path,
                                        1,
                                        None,
                                        "Target is a directory",
                                    ));
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // File not found — maybe new file? For patch, we require existing file, so this is Failed.
                        // But if file is new (not yet created), this would be NotRun? For single-file patch, we treat as Failed with diagnostic.
                        // However create_patch_proposal requires file to exist, so validate should report Failed.
                        status = CheckStatus::Failed;
                        diags.push(diag(&proposal.path, 1, None, "Target file not found"));
                    }
                }
            }
            Err(e) => {
                status = CheckStatus::Failed;
                diags.push(diag(
                    &proposal.path,
                    1,
                    None,
                    format!("Invalid workspace root: {}", e.message),
                ));
            }
        }

        push(ValidationCheck {
            id: "target_boundary".to_string(),
            label: "Target path & workspace boundary".to_string(),
            status,
            diagnostics: diags,
            note,
        });
    }

    // 3. Sensitive / binary / file-size
    {
        let mut diags = Vec::new();
        let mut status = CheckStatus::Passed;
        let path_sensitive = is_sensitive_file(Path::new(&proposal.path));
        let lower = proposal.path.to_ascii_lowercase();
        let env_sensitive =
            lower == ".env" || lower.starts_with(".env.") || lower.contains("/.env");
        if path_sensitive || env_sensitive {
            status = CheckStatus::Failed;
            diags.push(diag(
                &proposal.path,
                1,
                None,
                "Sensitive file — patching excluded",
            ));
        }
        if proposal.original_content.contains('\0') || proposal.proposed_content.contains('\0') {
            if status != CheckStatus::Failed {
                status = CheckStatus::Failed;
            }
            diags.push(diag(
                &proposal.path,
                1,
                None,
                "Binary content detected (NUL)",
            ));
        }
        // Binary via non-utf8 already handled as String, but we also check for high control? Not needed.

        if proposal.original_content.len() as u64 > MAX_PREVIEW_BYTES {
            status = CheckStatus::Failed;
            diags.push(diag(&proposal.path, 1, None, "Original content too large"));
        }
        if proposal.proposed_content.len() as u64 > MAX_PREVIEW_BYTES {
            if status != CheckStatus::Failed {
                status = CheckStatus::Failed;
            }
            diags.push(diag(
                &proposal.path,
                1,
                None,
                format!(
                    "Proposed content too large ({} bytes, max {})",
                    proposal.proposed_content.len(),
                    MAX_PREVIEW_BYTES
                ),
            ));
        }

        push(ValidationCheck {
            id: "sensitive_binary_size".to_string(),
            label: "Sensitive / binary / size".to_string(),
            status,
            diagnostics: diags,
            note: None,
        });
    }

    // 4. Stale hash
    {
        let mut diags = Vec::new();
        let mut status = CheckStatus::Passed;
        let mut note = None;

        match canonicalize_root(root) {
            Ok(canonical_root) => {
                let joined = canonical_root.join(&proposal.path);
                match fs::canonicalize(&joined) {
                    Ok(canonical_file) => {
                        if let Ok(meta) = fs::metadata(&canonical_file) {
                            if !meta.is_dir() {
                                if let Ok(bytes) = fs::read(&canonical_file) {
                                    if bytes.contains(&0) {
                                        // Binary stale not checked
                                        status = CheckStatus::NotRun;
                                        note = Some(
                                            "Stale check not run — current file is binary"
                                                .to_string(),
                                        );
                                    } else if let Ok(current) = String::from_utf8(bytes) {
                                        let current_hash = hash_content(&current);
                                        if current_hash != proposal.original_hash {
                                            status = CheckStatus::Failed;
                                            diags.push(diag(
                                                &proposal.path,
                                                1,
                                                None,
                                                "File has changed since proposal was created (stale original hash)",
                                            ));
                                        }
                                    } else {
                                        status = CheckStatus::NotRun;
                                        note = Some(
                                            "Stale check not run — current file not UTF-8"
                                                .to_string(),
                                        );
                                    }
                                } else {
                                    status = CheckStatus::NotRun;
                                    note = Some(
                                        "Stale check not run — could not read current file"
                                            .to_string(),
                                    );
                                }
                            }
                        }
                    }
                    Err(_) => {
                        status = CheckStatus::NotRun;
                        note = Some("Stale check not run — target file not found".to_string());
                    }
                }
            }
            Err(_) => {
                status = CheckStatus::NotRun;
                note = Some("Stale check not run — invalid workspace root".to_string());
            }
        }

        push(ValidationCheck {
            id: "stale".to_string(),
            label: "Stale source (hash)".to_string(),
            status,
            diagnostics: diags,
            note,
        });
    }

    // 5. Syntax / structure (non-exec, in-memory)
    {
        let ext = ext_of(&proposal.path);
        let (status, diags, note) = match ext.as_str() {
            "json" => {
                // Use serde_json to validate proposedContent
                match serde_json::from_str::<serde_json::Value>(&proposal.proposed_content) {
                    Ok(_) => (CheckStatus::Passed, vec![], None),
                    Err(e) => {
                        // Try to extract line/col from serde error (it includes line/col)
                        let msg = e.to_string();
                        // serde_json error format: "error at line X column Y"
                        let line = msg
                            .split("line ")
                            .nth(1)
                            .and_then(|s| s.split_whitespace().next())
                            .and_then(|n| n.parse::<usize>().ok())
                            .unwrap_or(1);
                        let col = msg
                            .split("column ")
                            .nth(1)
                            .and_then(|s| s.split_whitespace().next())
                            .and_then(|n| n.parse::<usize>().ok());
                        (
                            CheckStatus::Failed,
                            vec![diag(
                                &proposal.path,
                                line,
                                col,
                                format!("JSON syntax: {}", msg),
                            )],
                            None,
                        )
                    }
                }
            }
            "rs" | "ts" | "tsx" | "js" | "jsx" | "py" => {
                // Balanced delimiters check — in-memory, no exec
                // For very small or empty content, mark Inconclusive? But we already handled empty.
                // For now, run check_balanced; on error → Failed, else Passed.
                // If file is large (>200k chars already limited), but we still check.
                // This is heuristic — false positives possible, but we report actual diagnostics.
                match check_balanced(&proposal.path, &proposal.proposed_content) {
                    Ok(()) => (CheckStatus::Passed, vec![], None),
                    Err(ds) => {
                        // If proposed content is trivial (e.g., single line), mark as Failed still but note it's heuristic
                        (
                            CheckStatus::Failed,
                            ds,
                            Some("Heuristic balanced-delimiter check (no compiler)".to_string()),
                        )
                    }
                }
            }
            _ => {
                // Unsupported language for structural check
                (
                    CheckStatus::Unsupported,
                    vec![],
                    Some(format!(
                        "No structural check for .{} — contract/boundary/sensitive checks still ran",
                        ext
                    )),
                )
            }
        };

        push(ValidationCheck {
            id: "syntax".to_string(),
            label: format!(
                "Syntax (.{}) — {}",
                ext,
                if ext == "json" {
                    "JSON parse"
                } else if ["rs", "ts", "tsx", "js", "jsx", "py"].contains(&ext.as_str()) {
                    "balanced delimiters"
                } else {
                    "unsupported"
                }
            ),
            status,
            diagnostics: diags,
            note,
        });
    }

    // 6. Not-run placeholder for checks that would require exec (cargo check, pytest, etc.)
    {
        push(ValidationCheck {
            id: "static_analysis".to_string(),
            label: "Static analysis (cargo check / typecheck)".to_string(),
            status: CheckStatus::NotRun,
            diagnostics: vec![],
            note: Some(
                "Not run — would require executing project build/test (unsafe, skipped). Validation is in-memory only.".to_string(),
            ),
        });
    }

    // Overall
    let overall = if overall_failed {
        CheckStatus::Failed
    } else if overall_inconclusive {
        CheckStatus::Inconclusive
    } else {
        // If all checks are Passed/Unsupported/NotRun and at least one Passed, overall Passed
        // Check if any Failed already handled, else Passed
        CheckStatus::Passed
    };

    let summary = match overall {
        CheckStatus::Passed => {
            "All supported checks passed — proposal appears safe to review".to_string()
        }
        CheckStatus::Failed => {
            "Validation failed — proposal has errors that must be addressed".to_string()
        }
        CheckStatus::Inconclusive => {
            "Validation inconclusive — some checks could not be fully determined".to_string()
        }
        CheckStatus::NotRun => "Validation not run".to_string(),
        CheckStatus::Unsupported => "Validation unsupported".to_string(),
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    ValidationReport {
        overall,
        checks,
        summary,
        duration_ms,
        proposal_id: proposal.id.clone(),
        proposal_path: proposal.path.clone(),
        generated_at,
    }
}

// ---------------------------------------------------------------------------
// Tests — in-memory, no fs writes into project dir (uses TempDir outside root)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::patch::PatchProposal;
    use std::fs;
    use tempfile::TempDir;

    fn make_proposal(path: &str, original: &str, proposed: &str) -> PatchProposal {
        PatchProposal {
            id: "patch-test-1".to_string(),
            path: path.to_string(),
            original_content: original.to_string(),
            original_hash: hash_content(original),
            proposed_content: proposed.to_string(),
            proposed_hash: hash_content(proposed),
            created_at: 0,
        }
    }

    fn temp_root_with(file_path: &str, content: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        let full = dir.path().join(file_path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, content).unwrap();
        dir
    }

    #[test]
    fn contract_passes() {
        let dir = temp_root_with("src/a.txt", "old");
        let p = make_proposal("src/a.txt", "old", "new");
        let r = validate_detailed(dir.path(), &p);
        let c = r.checks.iter().find(|c| c.id == "contract_schema").unwrap();
        assert_eq!(c.status, CheckStatus::Passed);
        assert_eq!(r.overall, CheckStatus::Passed);
    }

    #[test]
    fn contract_fails_on_hash_mismatch() {
        let dir = temp_root_with("src/a.txt", "old");
        let mut p = make_proposal("src/a.txt", "old", "new");
        p.proposed_hash = "bad".to_string();
        let r = validate_detailed(dir.path(), &p);
        let c = r.checks.iter().find(|c| c.id == "contract_schema").unwrap();
        assert_eq!(c.status, CheckStatus::Failed);
        assert!(c
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Proposed hash")));
        assert_eq!(r.overall, CheckStatus::Failed);
    }

    #[test]
    fn stale_detected() {
        let dir = temp_root_with("src/a.txt", "changed");
        let p = make_proposal("src/a.txt", "old", "new");
        // original file is "changed", proposal original_hash is hash("old") → stale
        let r = validate_detailed(dir.path(), &p);
        let c = r.checks.iter().find(|c| c.id == "stale").unwrap();
        assert_eq!(c.status, CheckStatus::Failed);
        assert!(c.diagnostics[0].message.contains("changed since"));
        assert_eq!(r.overall, CheckStatus::Failed);
    }

    #[test]
    fn target_boundary_traversal() {
        let dir = TempDir::new().unwrap();
        let p = make_proposal("../outside.txt", "old", "new");
        let r = validate_detailed(dir.path(), &p);
        let c = r.checks.iter().find(|c| c.id == "target_boundary").unwrap();
        assert_eq!(c.status, CheckStatus::Failed);
        assert!(c
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Invalid") || d.message.contains("traversal")));
    }

    #[test]
    fn sensitive_blocked() {
        let dir = TempDir::new().unwrap();
        let p = make_proposal(".env", "old", "new");
        let r = validate_detailed(dir.path(), &p);
        let c = r
            .checks
            .iter()
            .find(|c| c.id == "sensitive_binary_size")
            .unwrap();
        assert_eq!(c.status, CheckStatus::Failed);
        assert!(c
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Sensitive")));
    }

    #[test]
    fn json_syntax_ok_and_fail() {
        let dir = temp_root_with("data.json", r#"{"a":1}"#);
        let p_ok = make_proposal("data.json", r#"{"a":1}"#, r#"{"a":2,"b":[1,2]}"#);
        let r_ok = validate_detailed(dir.path(), &p_ok);
        let c_ok = r_ok.checks.iter().find(|c| c.id == "syntax").unwrap();
        assert_eq!(c_ok.status, CheckStatus::Passed);

        let p_bad = make_proposal("data.json", r#"{"a":1}"#, r#"{"a":1,}"#);
        let r_bad = validate_detailed(dir.path(), &p_bad);
        let c_bad = r_bad.checks.iter().find(|c| c.id == "syntax").unwrap();
        assert_eq!(c_bad.status, CheckStatus::Failed);
        assert!(c_bad.diagnostics[0].message.contains("JSON"));
        assert_eq!(r_bad.overall, CheckStatus::Failed);
    }

    #[test]
    fn balanced_rs_passes() {
        let dir = temp_root_with("src/main.rs", "fn main() {}");
        let p = make_proposal(
            "src/main.rs",
            "fn main() {}",
            "fn main() { let x = (1 + 2); }",
        );
        let r = validate_detailed(dir.path(), &p);
        let c = r.checks.iter().find(|c| c.id == "syntax").unwrap();
        assert_eq!(c.status, CheckStatus::Passed);
    }

    #[test]
    fn balanced_rs_fails_on_mismatch() {
        let dir = temp_root_with("src/main.rs", "fn main() {}");
        let p = make_proposal(
            "src/main.rs",
            "fn main() {}",
            "fn main() { let x = (1 + 2; }",
        );
        let r = validate_detailed(dir.path(), &p);
        let c = r.checks.iter().find(|c| c.id == "syntax").unwrap();
        assert_eq!(c.status, CheckStatus::Failed);
        assert!(
            c.diagnostics[0].message.contains("Mismatched")
                || c.diagnostics[0].message.contains("Unclosed")
                || c.diagnostics[0].message.contains("Unmatched")
        );
    }

    #[test]
    fn unsupported_lang() {
        let dir = temp_root_with("README.md", "# hi");
        let p = make_proposal("README.md", "# hi", "# hello");
        let r = validate_detailed(dir.path(), &p);
        let c = r.checks.iter().find(|c| c.id == "syntax").unwrap();
        assert_eq!(c.status, CheckStatus::Unsupported);
    }

    #[test]
    fn not_run_static_analysis() {
        let dir = temp_root_with("src/a.txt", "old");
        let p = make_proposal("src/a.txt", "old", "new");
        let r = validate_detailed(dir.path(), &p);
        let c = r.checks.iter().find(|c| c.id == "static_analysis").unwrap();
        assert_eq!(c.status, CheckStatus::NotRun);
        assert!(c.note.as_deref().unwrap().contains("Not run"));
    }

    #[test]
    fn overall_failed_if_any_failed() {
        let dir = TempDir::new().unwrap();
        let p = make_proposal(".env", "old", "new");
        let r = validate_detailed(dir.path(), &p);
        assert_eq!(r.overall, CheckStatus::Failed);
    }

    #[test]
    fn cleans_temp_not_in_root() {
        // Ensure validation does not write into root (we use in-memory only, so root should stay clean)
        let dir = temp_root_with("src/a.txt", "old");
        let before: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        let p = make_proposal("src/a.txt", "old", "new");
        let _ = validate_detailed(dir.path(), &p);
        let after: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(before, after);
    }
}
