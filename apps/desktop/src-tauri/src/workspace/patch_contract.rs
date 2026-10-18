use serde::{Deserialize, Serialize};

use super::scanner::{is_sensitive_file, validate_relative_path, MAX_PREVIEW_BYTES, MAX_PATH_CHARS};

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
            return Err("Missing 'proposedContent' — patch must contain replacement content.".to_string());
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
        let explanation = self.explanation.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
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
    if trimmed.contains("\"files\"") || trimmed.contains("\"patches\"") || trimmed.contains("\"edits\"") {
        return Err("Only single-file patches are allowed — model returned multiple files.".to_string());
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
            return Some("Only single-file patches are allowed — model returned multiple files.".to_string());
        }
        // If has both targetPath and files array, it's multi
        if map.contains_key("targetPath") && (map.contains_key("file") || map.contains_key("path") && map.get("path") != map.get("targetPath")) {
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
            return Some("Only single-file patches are allowed — model returned an array.".to_string());
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
        assert!(err.contains("Missing") || err.contains("missing field"), "got {}", err);
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
        assert!(err.contains("Malformed") || err.contains("valid JSON"), "got {}", err);
    }

    #[test]
    fn rejects_unknown_fields() {
        let json = r#"{"version":1,"targetPath":"a.txt","proposedContent":"new","extra":"field"}"#;
        let err = parse_and_validate(json, "a.txt", "old").unwrap_err();
        assert!(err.contains("Unexpected") || err.contains("unknown field"), "got {}", err);
    }

    #[test]
    fn rejects_multiple_files() {
        let json = r#"{"version":1,"targetPath":"a.txt","proposedContent":"new","files":[{"path":"b.txt"}]}"#;
        let err = parse_and_validate(json, "a.txt", "old").unwrap_err();
        assert!(err.contains("Only single-file"), "got {}", err);

        let json2 = r#"{"files":[{"path":"a.txt","content":"x"}]}"#;
        let err2 = parse_and_validate(json2, "a.txt", "old").unwrap_err();
        assert!(err2.contains("Only single-file") || err2.contains("Missing"), "got {}", err2);
    }

    #[test]
    fn rejects_empty_proposed() {
        let json = r#"{"version":1,"targetPath":"a.txt","proposedContent":""}"#;
        let err = parse_and_validate(json, "a.txt", "old").unwrap_err();
        assert!(err.contains("Missing") || err.contains("proposedContent"), "got {}", err);
    }

    #[test]
    fn rejects_oversized() {
        let big = "a".repeat(300_000);
        let v = serde_json::json!({"version":1,"targetPath":"a.txt","proposedContent": big});
        let s = v.to_string();
        let err = parse_and_validate(&s, "a.txt", "old").unwrap_err();
        assert!(err.contains("too large") || err.contains("character limit"), "got {}", err);
    }

    #[test]
    fn rejects_invalid_target_traversal() {
        let json = r#"{"version":1,"targetPath":"../outside.txt","proposedContent":"new"}"#;
        let err = parse_and_validate(json, "../outside.txt", "old").unwrap_err();
        assert!(err.contains("Invalid") || err.contains("forbidden"), "got {}", err);
    }

    #[test]
    fn rejects_absolute_path() {
        let json = r#"{"version":1,"targetPath":"/etc/passwd","proposedContent":"new"}"#;
        let err = parse_and_validate(json, "/etc/passwd", "old").unwrap_err();
        assert!(err.contains("Absolute") || err.contains("Invalid"), "got {}", err);
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
