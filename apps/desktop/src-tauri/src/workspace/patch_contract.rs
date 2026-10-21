use serde::{Deserialize, Serialize};

use super::scanner::{
    is_sensitive_file, validate_relative_path, MAX_PATH_CHARS, MAX_PREVIEW_BYTES,
};

// ---------------------------------------------------------------------------
// Contract — versioned, strict, single-file
// ---------------------------------------------------------------------------

pub const PATCH_CONTRACT_VERSION: u8 = 1;
const MAX_EXPLANATION_CHARS: usize = 1_000;
const MAX_INSTRUCTION_CHARS: usize = 2_000;
const MAX_PROPOSED_CHARS: usize = 200_000;

// Normalized, validated proposal after parsing
#[derive(Debug, Clone)]
pub struct ValidatedPatchContract {
    pub version: u8,
    pub target_path: String,
    pub proposed_content: String,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchGenerationResponseStrict {
    pub version: u8,
    #[serde(alias = "target_path")]
    pub targetPath: String,
    #[serde(alias = "proposed_content")]
    pub proposedContent: String,
    #[serde(default)]
    pub explanation: Option<String>,
}

impl PatchGenerationResponseStrict {
    pub fn validate(
        &self,
        expected_target: &str,
        original_content: &str,
    ) -> Result<ValidatedPatchContract, String> {
        // version
        if self.version != PATCH_CONTRACT_VERSION {
            return Err(format!(
                "Unsupported contract version {} (expected {}).",
                self.version, PATCH_CONTRACT_VERSION
            ));
        }

        // targetPath must equal expected
        let target = self.targetPath.trim();
        if target.is_empty() {
            return Err("Missing 'targetPath'.".to_string());
        }
        if target != expected_target {
            return Err(format!(
                "Target path '{}' does not match requested '{}'. Only single-file patches are allowed.",
                target, expected_target
            ));
        }

        // validate relative path (no traversal, no absolute, no ParentDir)
        if let Err(e) = validate_relative_path(target) {
            return Err(format!("Invalid target path '{}': {}", target, e.message));
        }
        if target.len() > MAX_PATH_CHARS {
            return Err("Target path too long.".to_string());
        }
        if std::path::Path::new(target).is_absolute() {
            return Err("Absolute target path not allowed.".to_string());
        }
        if is_sensitive_file(std::path::Path::new(target)) {
            return Err("Sensitive file cannot be patched.".to_string());
        }

        // proposedContent checks
        let proposed = &self.proposedContent;
        if proposed.trim().is_empty() {
            return Err(
                "Missing 'proposedContent' — patch must contain replacement content.".to_string(),
            );
        }
        if proposed.as_bytes().len() as u64 > MAX_PREVIEW_BYTES {
            return Err(format!(
                "Proposed content too large ({} bytes, max {}).",
                proposed.as_bytes().len(),
                MAX_PREVIEW_BYTES
            ));
        }
        if proposed.chars().count() > MAX_PROPOSED_CHARS {
            return Err("Proposed content exceeds character limit.".to_string());
        }
        if proposed.contains('\0') {
            return Err("Proposed content is binary (contains NUL).".to_string());
        }
        if proposed == original_content {
            return Err("No changes detected — proposed content equals original.".to_string());
        }

        // explanation optional
        let explanation = self
            .explanation
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if let Some(exp) = &explanation {
            if exp.chars().count() > MAX_EXPLANATION_CHARS {
                return Err("Explanation too long (max 1000 chars).".to_string());
            }
        }

        Ok(ValidatedPatchContract {
            version: self.version,
            target_path: target.to_string(),
            proposed_content: proposed.clone(),
            explanation,
        })
    }
}

/// Strict JSON parser — rejects missing/malformed/unknown fields, multiple files, prose.
///
/// Attempts:
/// 1. Direct `serde_json::from_str` on trimmed input (must be exact object)
/// 2. If that fails, try to extract first balanced `{...}` or ```json fenced block
/// 3. Validate via `PatchGenerationResponseStrict::validate`
pub fn parse_and_validate(
    raw: &str,
    expected_target: &str,
    original_content: &str,
) -> Result<ValidatedPatchContract, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Model returned empty response.".to_string());
    }

    // Try direct parse first — strict, deny_unknown_fields will reject extra keys
    let direct: Result<PatchGenerationResponseStrict, _> = serde_json::from_str(trimmed);
    if let Ok(resp) = direct {
        return resp.validate(expected_target, original_content);
    }

    // Try to extract JSON from prose or fenced block
    // Look for ```json ... ``` or ``` ... ```
    let fenced = extract_fenced_json(trimmed);
    if let Some(json_str) = fenced {
        if let Ok(resp) = serde_json::from_str::<PatchGenerationResponseStrict>(&json_str) {
            return resp.validate(expected_target, original_content);
        }
        // Also try generic JSON value to detect multiple files
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json_str) {
            if let Some(err) = detect_multi_file(&v) {
                return Err(err);
            }
        }
    }

    // Try to find first balanced JSON object via brace scan
    if let Some(json_str) = extract_first_json_object(trimmed) {
        if let Ok(resp) = serde_json::from_str::<PatchGenerationResponseStrict>(&json_str) {
            return resp.validate(expected_target, original_content);
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json_str) {
            if let Some(err) = detect_multi_file(&v) {
                return Err(err);
            }
        }
        // Still try generic error
        if let Err(e) = serde_json::from_str::<PatchGenerationResponseStrict>(&json_str) {
            return Err(format!("Malformed patch JSON: {}", e));
        }
    }

    // Detect obvious multi-file attempts in raw string
    if trimmed.contains("\"files\"")
        || trimmed.contains("\"patches\"")
        || trimmed.contains("\"edits\"")
    {
        return Err(
            "Only single-file patches are allowed — model returned multiple files.".to_string(),
        );
    }

    // Return original direct parse error with more context
    if let Err(e) = serde_json::from_str::<PatchGenerationResponseStrict>(trimmed) {
        // Check for unknown fields / missing fields
        let msg = e.to_string();
        if msg.contains("unknown field") {
            return Err(format!("Unexpected field in patch response: {}", msg));
        }
        if msg.contains("missing field") {
            return Err(format!("Missing required field: {}", msg));
        }
        return Err(format!("Malformed patch JSON: {}", msg));
    }

    Err("Model did not return valid JSON. Try a more specific instruction.".to_string())
}

fn extract_fenced_json(input: &str) -> Option<String> {
    // Find ```json or ``` then capture until closing ```
    let lower = input.to_lowercase();
    // Try ```json
    if let Some(start) = lower.find("```json") {
        let after = &input[start + 7..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim().to_string());
        }
    }
    if let Some(start) = input.find("```") {
        let after = &input[start + 3..];
        // skip optional language tag line
        let after = after.trim_start_matches(|c: char| c.is_alphanumeric() || c == '-' || c == '_');
        let after = after.trim_start_matches('\n').trim_start_matches('\r');
        // Actually simpler: find next ```
        if let Some(end) = after.find("```") {
            let candidate = after[..end].trim();
            if candidate.starts_with('{') {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn extract_first_json_object(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut start: Option<usize> = None;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;

    for (i, &b) in bytes.iter().enumerate() {
        if start.is_none() {
            if b == b'{' {
                start = Some(i);
                depth = 1;
                in_string = false;
                escape = false;
            }
            continue;
        }

        if in_string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }

        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    let s = start.unwrap();
                    return Some(input[s..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn detect_multi_file(v: &serde_json::Value) -> Option<String> {
    if let serde_json::Value::Object(map) = v {
        if map.contains_key("files") || map.contains_key("patches") || map.contains_key("edits") {
            return Some(
                "Only single-file patches are allowed — model returned multiple files.".to_string(),
            );
        }
        // If has both targetPath and files array, it's multi
        if map.contains_key("targetPath")
            && (map.contains_key("file")
                || map.contains_key("path") && map.get("path") != map.get("targetPath"))
        {
            // Not reliable, but check if proposedContent is array
            if let Some(pc) = map.get("proposedContent") {
                if pc.is_array() {
                    return Some("Only single-file patches are allowed.".to_string());
                }
            }
        }
    }
    if let serde_json::Value::Array(arr) = v {
        if !arr.is_empty() {
            return Some(
                "Only single-file patches are allowed — model returned an array.".to_string(),
            );
        }
    }
    None
}

pub fn validate_instruction(instruction: &str) -> Result<String, String> {
    let t = instruction.trim();
    if t.is_empty() {
        return Err("Change request is empty — describe the change you want.".to_string());
    }
    if t.chars().count() > MAX_INSTRUCTION_CHARS {
        return Err(format!(
            "Change request too long ({} chars, max {}).",
            t.chars().count(),
            MAX_INSTRUCTION_CHARS
        ));
    }
    // Basic check: should not be just whitespace or very short
    if t.len() < 3 {
        return Err("Change request too short.".to_string());
    }
    Ok(t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn original() -> String {
        "let a = 1;\n".to_string()
    }

    #[test]
    fn valid_single_file() {
        let json = r#"{"version":1,"targetPath":"src/app.ts","proposedContent":"let a = 2;\n","explanation":"update"}"#;
        let res = parse_and_validate(json, "src/app.ts", &original()).expect("valid");
        assert_eq!(res.target_path, "src/app.ts");
        assert_eq!(res.proposed_content, "let a = 2;\n");
        assert_eq!(res.explanation.as_deref(), Some("update"));
    }

    #[test]
    fn valid_without_explanation() {
        let json = r#"{"version":1,"targetPath":"a.txt","proposedContent":"hello"}"#;
        let res = parse_and_validate(json, "a.txt", "old").expect("valid");
        assert!(res.explanation.is_none());
    }

    #[test]
    fn valid_with_fenced_json() {
        let raw = "Here is patch:\n```json\n{\"version\":1,\"targetPath\":\"a.txt\",\"proposedContent\":\"new\"}\n```\n";
        let res = parse_and_validate(raw, "a.txt", "old").expect("valid fenced");
        assert_eq!(res.proposed_content, "new");
    }

    #[test]
    fn rejects_missing_fields() {
        let json = r#"{"version":1,"targetPath":"a.txt"}"#;
        let err = parse_and_validate(json, "a.txt", "old").unwrap_err();
        assert!(
            err.contains("Missing") || err.contains("missing field"),
            "got {}",
            err
        );
    }

    #[test]
    fn rejects_unsupported_version() {
        let json = r#"{"version":2,"targetPath":"a.txt","proposedContent":"new"}"#;
        let err = parse_and_validate(json, "a.txt", "old").unwrap_err();
        assert!(err.contains("Unsupported"), "got {}", err);
    }

    #[test]
    fn rejects_malformed_json() {
        let err = parse_and_validate("not json at all", "a.txt", "old").unwrap_err();
        assert!(
            err.contains("Malformed") || err.contains("valid JSON"),
            "got {}",
            err
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        let json = r#"{"version":1,"targetPath":"a.txt","proposedContent":"new","extra":"field"}"#;
        let err = parse_and_validate(json, "a.txt", "old").unwrap_err();
        assert!(
            err.contains("Unexpected") || err.contains("unknown field"),
            "got {}",
            err
        );
    }

    #[test]
    fn rejects_multiple_files() {
        let json = r#"{"version":1,"targetPath":"a.txt","proposedContent":"new","files":[{"path":"b.txt"}]}"#;
        let err = parse_and_validate(json, "a.txt", "old").unwrap_err();
        assert!(err.contains("Only single-file"), "got {}", err);

        let json2 = r#"{"files":[{"path":"a.txt","content":"x"}]}"#;
        let err2 = parse_and_validate(json2, "a.txt", "old").unwrap_err();
        assert!(
            err2.contains("Only single-file") || err2.contains("Missing"),
            "got {}",
            err2
        );
    }

    #[test]
    fn rejects_empty_proposed() {
        let json = r#"{"version":1,"targetPath":"a.txt","proposedContent":""}"#;
        let err = parse_and_validate(json, "a.txt", "old").unwrap_err();
        assert!(
            err.contains("Missing") || err.contains("proposedContent"),
            "got {}",
            err
        );
    }

    #[test]
    fn rejects_oversized() {
        let big = "a".repeat(300_000);
        let v = serde_json::json!({"version":1,"targetPath":"a.txt","proposedContent": big});
        let s = v.to_string();
        let err = parse_and_validate(&s, "a.txt", "old").unwrap_err();
        assert!(
            err.contains("too large") || err.contains("character limit"),
            "got {}",
            err
        );
    }

    #[test]
    fn rejects_invalid_target_traversal() {
        let json = r#"{"version":1,"targetPath":"../outside.txt","proposedContent":"new"}"#;
        let err = parse_and_validate(json, "../outside.txt", "old").unwrap_err();
        assert!(
            err.contains("Invalid") || err.contains("forbidden"),
            "got {}",
            err
        );
    }

    #[test]
    fn rejects_absolute_path() {
        let json = r#"{"version":1,"targetPath":"/etc/passwd","proposedContent":"new"}"#;
        let err = parse_and_validate(json, "/etc/passwd", "old").unwrap_err();
        assert!(
            err.contains("Absolute") || err.contains("Invalid"),
            "got {}",
            err
        );
    }

    #[test]
    fn rejects_sensitive() {
        let json = r#"{"version":1,"targetPath":".env","proposedContent":"new"}"#;
        let err = parse_and_validate(json, ".env", "old").unwrap_err();
        assert!(err.contains("Sensitive"), "got {}", err);
    }

    #[test]
    fn rejects_target_mismatch() {
        let json = r#"{"version":1,"targetPath":"a.txt","proposedContent":"new"}"#;
        let err = parse_and_validate(json, "b.txt", "old").unwrap_err();
        assert!(err.contains("does not match"), "got {}", err);
    }

    #[test]
    fn rejects_no_changes() {
        let json = r#"{"version":1,"targetPath":"a.txt","proposedContent":"same"}"#;
        let err = parse_and_validate(json, "a.txt", "same").unwrap_err();
        assert!(err.contains("No changes"), "got {}", err);
    }

    #[test]
    fn validate_instruction_ok() {
        assert!(validate_instruction("  fix bug  ").is_ok());
    }

    #[test]
    fn validate_instruction_empty() {
        assert!(validate_instruction("   ").is_err());
    }

    #[test]
    fn validate_instruction_too_long() {
        let s = "a".repeat(3000);
        assert!(validate_instruction(&s).is_err());
    }
}

// ---------------------------------------------------------------------------
// Contract v2 — multi-file batches (mirror of patchContract.ts; additive)
// ---------------------------------------------------------------------------
//
// v2: { "version": 2, "explanation"?: "...", "files": [ { "targetPath",
//       "proposedContent", "explanation"? }, ... ] } — 1..=MAX_BATCH_FILES
// entries, unique paths (case-insensitive). A v1 object parses as a 1-file
// batch. Per-file rules are exactly the v1 rules.

use std::collections::HashMap;

pub const PATCH_CONTRACT_VERSION_V2: u8 = 2;
pub const MAX_BATCH_FILES: usize = 20;

#[derive(Debug, Clone)]
pub struct ValidatedFileContract {
    pub target_path: String,
    pub proposed_content: String,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ValidatedPatchBatch {
    pub version: u8,
    pub files: Vec<ValidatedFileContract>,
    pub explanation: Option<String>,
}

fn validate_file_fields(
    target_raw: &str,
    proposed: &str,
    explanation: Option<&str>,
    original: Option<&str>,
) -> Result<ValidatedFileContract, String> {
    let target = target_raw.trim();
    if target.is_empty() {
        return Err("Missing 'targetPath'.".to_string());
    }
    if let Err(e) = validate_relative_path(target) {
        return Err(format!("Invalid target path '{}': {}", target, e.message));
    }
    if target.len() > MAX_PATH_CHARS {
        return Err("Target path too long.".to_string());
    }
    if std::path::Path::new(target).is_absolute() {
        return Err("Absolute target path not allowed.".to_string());
    }
    if is_sensitive_file(std::path::Path::new(target)) {
        return Err("Sensitive file cannot be patched.".to_string());
    }

    if proposed.trim().is_empty() {
        return Err(
            "Missing 'proposedContent' — patch must contain replacement content.".to_string(),
        );
    }
    if proposed.as_bytes().len() as u64 > MAX_PREVIEW_BYTES {
        return Err(format!(
            "Proposed content too large ({} bytes, max {}).",
            proposed.as_bytes().len(),
            MAX_PREVIEW_BYTES
        ));
    }
    if proposed.chars().count() > MAX_PROPOSED_CHARS {
        return Err("Proposed content exceeds character limit.".to_string());
    }
    if proposed.contains('\0') {
        return Err("Proposed content is binary (contains NUL).".to_string());
    }
    if let Some(original) = original {
        if proposed == original {
            return Err(format!(
                "No changes detected in '{}' — proposed content equals original.",
                target
            ));
        }
    }

    let explanation = explanation
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(exp) = &explanation {
        if exp.chars().count() > MAX_EXPLANATION_CHARS {
            return Err("Explanation too long (max 1000 chars).".to_string());
        }
    }

    Ok(ValidatedFileContract {
        target_path: target.to_string(),
        proposed_content: proposed.to_string(),
        explanation,
    })
}

/// Strict parser for v2 batches. `originals` (path → content) enables the
/// no-change check for known files; files the caller has not seen are checked
/// against disk at batch creation instead.
pub fn parse_and_validate_batch(
    raw: &str,
    originals: &HashMap<String, String>,
) -> Result<ValidatedPatchBatch, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Model returned empty response.".to_string());
    }
    let parsed: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|e| format!("Malformed patch JSON: {}", e))?;
    let obj = parsed
        .as_object()
        .ok_or_else(|| "Patch response must be a JSON object.".to_string())?;

    // v1 single-file object → 1-file batch.
    if !obj.contains_key("files") {
        let version = obj.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
        if version != PATCH_CONTRACT_VERSION && version != PATCH_CONTRACT_VERSION_V2 {
            return Err(format!(
                "Unsupported contract version {} (expected 1 or 2).",
                version
            ));
        }
        for key in obj.keys() {
            let allowed = matches!(
                key.as_str(),
                "version"
                    | "targetPath"
                    | "target_path"
                    | "proposedContent"
                    | "proposed_content"
                    | "explanation"
            );
            if !allowed {
                return Err(format!("Unexpected field in patch response: '{}'", key));
            }
        }
        let target_raw = obj
            .get("targetPath")
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("target_path").and_then(|v| v.as_str()))
            .unwrap_or("");
        let proposed = obj
            .get("proposedContent")
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("proposed_content").and_then(|v| v.as_str()))
            .unwrap_or("");
        let explanation = obj.get("explanation").and_then(|v| v.as_str());
        let original = originals.get(target_raw.trim()).map(|s| s.as_str());
        let file = validate_file_fields(target_raw, proposed, explanation, original)?;
        return Ok(ValidatedPatchBatch {
            version: PATCH_CONTRACT_VERSION,
            files: vec![file],
            explanation: explanation
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        });
    }

    // v2 batch object.
    let version = obj.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    if version != PATCH_CONTRACT_VERSION_V2 {
        return Err(format!(
            "Unsupported contract version {} (expected 2 for 'files').",
            version
        ));
    }
    for key in obj.keys() {
        if !matches!(key.as_str(), "version" | "files" | "explanation") {
            return Err(format!("Unexpected field in patch response: '{}'", key));
        }
    }
    let files = obj
        .get("files")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Patch response 'files' must be a non-empty array.".to_string())?;
    if files.is_empty() {
        return Err("Patch response 'files' must be a non-empty array.".to_string());
    }
    if files.len() > MAX_BATCH_FILES {
        return Err(format!(
            "Too many files in patch response (max {}).",
            MAX_BATCH_FILES
        ));
    }

    let mut seen = std::collections::HashSet::new();
    let mut validated = Vec::with_capacity(files.len());
    for entry in files {
        let entry_obj = entry
            .as_object()
            .ok_or_else(|| "Each entry in 'files' must be a JSON object.".to_string())?;
        for key in entry_obj.keys() {
            let allowed = matches!(
                key.as_str(),
                "targetPath"
                    | "target_path"
                    | "proposedContent"
                    | "proposed_content"
                    | "explanation"
            );
            if !allowed {
                return Err(format!("Unexpected field in file entry: '{}'", key));
            }
        }
        let target_raw = entry_obj
            .get("targetPath")
            .and_then(|v| v.as_str())
            .or_else(|| entry_obj.get("target_path").and_then(|v| v.as_str()))
            .unwrap_or("");
        let proposed = entry_obj
            .get("proposedContent")
            .and_then(|v| v.as_str())
            .or_else(|| entry_obj.get("proposed_content").and_then(|v| v.as_str()))
            .unwrap_or("");
        let explanation = entry_obj.get("explanation").and_then(|v| v.as_str());
        let original = originals.get(target_raw.trim()).map(|s| s.as_str());
        let file = validate_file_fields(target_raw, proposed, explanation, original)?;
        if !seen.insert(file.target_path.to_lowercase()) {
            return Err(format!(
                "Duplicate target path in patch response: '{}'",
                file.target_path
            ));
        }
        validated.push(file);
    }

    let explanation = obj.get("explanation").and_then(|v| v.as_str());
    if let Some(exp) = explanation {
        if exp.trim().chars().count() > MAX_EXPLANATION_CHARS {
            return Err("Explanation too long (max 1000 chars).".to_string());
        }
    }

    Ok(ValidatedPatchBatch {
        version: PATCH_CONTRACT_VERSION_V2,
        files: validated,
        explanation: explanation
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
    })
}

#[cfg(test)]
mod batch_tests {
    use super::*;

    fn originals(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn accepts_valid_multi_file_batch() {
        let raw = r#"{"version":2,"explanation":"batch","files":[
            {"targetPath":"src/a.ts","proposedContent":"a2\n","explanation":"part one"},
            {"targetPath":"src/b.ts","proposedContent":"b2\n"}]}"#;
        let batch = parse_and_validate_batch(
            raw,
            &originals(&[("src/a.ts", "a1\n"), ("src/b.ts", "b1\n")]),
        )
        .unwrap();
        assert_eq!(batch.version, PATCH_CONTRACT_VERSION_V2);
        assert_eq!(batch.files.len(), 2);
        assert_eq!(batch.files[0].target_path, "src/a.ts");
        assert_eq!(batch.files[0].explanation.as_deref(), Some("part one"));
        assert_eq!(batch.explanation.as_deref(), Some("batch"));
    }

    #[test]
    fn wraps_v1_object_as_one_file_batch() {
        let raw = r#"{"version":1,"targetPath":"a.txt","proposedContent":"new"}"#;
        let batch = parse_and_validate_batch(raw, &originals(&[("a.txt", "old")])).unwrap();
        assert_eq!(batch.version, PATCH_CONTRACT_VERSION);
        assert_eq!(batch.files.len(), 1);
        assert_eq!(batch.files[0].target_path, "a.txt");
    }

    #[test]
    fn rejects_duplicates_and_oversize_batches() {
        let dup = r#"{"version":2,"files":[
            {"targetPath":"src/A.ts","proposedContent":"x"},
            {"targetPath":"src/a.ts","proposedContent":"y"}]}"#;
        assert!(parse_and_validate_batch(dup, &HashMap::new()).is_err());

        let empty = r#"{"version":2,"files":[]}"#;
        assert!(parse_and_validate_batch(empty, &HashMap::new()).is_err());

        let mut many = String::from("{\"version\":2,\"files\":[");
        for i in 0..MAX_BATCH_FILES + 1 {
            if i > 0 {
                many.push(',');
            }
            many.push_str(&format!(
                "{{\"targetPath\":\"f{i}.txt\",\"proposedContent\":\"x\"}}"
            ));
        }
        many.push_str("]}");
        assert!(parse_and_validate_batch(&many, &HashMap::new()).is_err());
    }

    #[test]
    fn enforces_per_file_rules() {
        let traversal = r#"{"version":2,"files":[
            {"targetPath":"ok.txt","proposedContent":"x"},
            {"targetPath":"../evil.txt","proposedContent":"x"}]}"#;
        assert!(parse_and_validate_batch(traversal, &HashMap::new()).is_err());

        let sensitive =
            r#"{"version":2,"files":[{"targetPath":"cfg/.env","proposedContent":"x"}]}"#;
        assert!(parse_and_validate_batch(sensitive, &HashMap::new()).is_err());

        let no_change =
            r#"{"version":2,"files":[{"targetPath":"a.txt","proposedContent":"same"}]}"#;
        assert!(parse_and_validate_batch(no_change, &originals(&[("a.txt", "same")])).is_err());

        let nul = "{\"version\":2,\"files\":[{\"targetPath\":\"a.bin\",\"proposedContent\":\"x\\u0000y\"}]}";
        assert!(parse_and_validate_batch(nul, &HashMap::new()).is_err());
    }

    #[test]
    fn rejects_wrong_versions_and_unknown_fields() {
        let wrong_version =
            r#"{"version":1,"files":[{"targetPath":"a.txt","proposedContent":"x"}]}"#;
        assert!(parse_and_validate_batch(wrong_version, &HashMap::new()).is_err());

        let unknown_top =
            r#"{"version":2,"extra":1,"files":[{"targetPath":"a.txt","proposedContent":"x"}]}"#;
        assert!(parse_and_validate_batch(unknown_top, &HashMap::new()).is_err());

        let unknown_file = r#"{"version":2,"files":[{"targetPath":"a.txt","proposedContent":"x","mode":"append"}]}"#;
        assert!(parse_and_validate_batch(unknown_file, &HashMap::new()).is_err());

        let array = r#"[{"targetPath":"a"}]"#;
        assert!(parse_and_validate_batch(array, &HashMap::new()).is_err());
    }

    #[test]
    fn allows_unseen_files_checked_later_against_disk() {
        let raw = r#"{"version":2,"files":[
            {"targetPath":"seen.txt","proposedContent":"new"},
            {"targetPath":"unseen.txt","proposedContent":"anything"}]}"#;
        let batch = parse_and_validate_batch(raw, &originals(&[("seen.txt", "old")])).unwrap();
        assert_eq!(batch.files.len(), 2);
    }
}
