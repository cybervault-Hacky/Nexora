//! Log scrubbing (Phase 23G — plan §4 "Log leakage" control, step 6).
//!
//! Plan §4: *"structured logging with a scrub list (password, token,
//! authorization, cookie, code_verifier); token values appear only in email
//! links"*. This module is the single scrub list every log/audit line must
//! pass through before it reaches stderr, plus the [`REDACTED`] placeholder
//! used by the custom `Debug` impls of secret-bearing types.
//!
//! The service deliberately logs almost nothing (startup capability line and
//! the plan-mandated password-change audit metadata); scrubbing is applied
//! anyway so that future log lines cannot casually leak. The
//! `tests/secret_leakage.rs` suite asserts the list works and that errors,
//! bodies, and `Debug` output never carry secret material.

/// Placeholder printed in place of any secret-bearing value.
pub const REDACTED: &str = "<redacted>";

/// Plan §4 scrub list, lowercase: `password`, `token`, `authorization`,
/// `cookie`, `code_verifier`. Extended with the equivalent names that carry
/// the same classes of material (`code`, `secret`, `client_secret`, `state`,
/// `nonce`, `key`, `hash`, `session`) — same intent, no scope expansion.
const SCRUB_KEYS: &[&str] = &[
    "password",
    "passwd",
    "token",
    "authorization",
    "cookie",
    "code_verifier",
    "code",
    "secret",
    "client_secret",
    "state",
    "nonce",
    "key",
    "hash",
    "session",
];

/// One `key=value` / `key: value` / `key="value"` pair to redact, if the line
/// contains a scrub-list key at a word boundary followed by a separator.
/// Returns (value_start, value_end) in `line`.
fn find_scrub_pair(line: &str) -> Option<(usize, usize)> {
    // ASCII lowercase preserves byte lengths, so offsets stay aligned with
    // `line` even for non-ASCII content.
    let lowered = line.to_ascii_lowercase();
    for key in SCRUB_KEYS {
        let mut search_from = 0;
        while let Some(found) = lowered[search_from..].find(key) {
            let key_start = search_from + found;
            let after_key = key_start + key.len();
            let boundary_before = key_start == 0
                || !lowered[..key_start]
                    .chars()
                    .next_back()
                    .map(|c| c.is_ascii_alphanumeric() || c == '_')
                    .unwrap_or(false);
            let boundary_after = !lowered[after_key..]
                .chars()
                .next()
                .map(|c| c.is_ascii_alphanumeric() || c == '_')
                .unwrap_or(false);
            if boundary_before && boundary_after {
                let rest = &line[after_key..];
                if let Some(first) = rest.chars().next() {
                    if first == '=' || first == ':' {
                        let trimmed = rest.trim_start_matches(['=', ':', ' ', '\t', '"', '\'']);
                        let value_start = line.len() - trimmed.len();
                        let after_value = line[value_start..]
                            .find(|c: char| {
                                matches!(c, '"' | '\'' | ',' | ' ' | '\n' | '\t' | ';' | '}' | ')' | '&')
                            })
                            .map(|offset| value_start + offset)
                            .unwrap_or(line.len());
                        if after_value > value_start {
                            return Some((value_start, after_value));
                        }
                    }
                }
            }
            search_from = after_key;
        }
    }
    None
}

/// Redact `key=value` / `key: value` / `key="value"` pairs whose key is on the
/// scrub list (case-insensitive), and any `Bearer <...>` credential.
/// Deliberately over-eager: a false-positive redaction is harmless, a leak is
/// not.
pub fn scrub(line: &str) -> String {
    let mut out = line.to_string();

    // Authorization-style credentials anywhere in the line.
    let lowered = out.to_ascii_lowercase();
    if let Some(index) = lowered.find("bearer ") {
        let start = index + "bearer ".len();
        let end = out[start..]
            .find(char::is_whitespace)
            .map(|offset| start + offset)
            .unwrap_or(out.len());
        out.replace_range(start..end, REDACTED);
    }

    // One pair per pass (offsets stay valid because each pass re-scans).
    while let Some((value_start, after_value)) = find_scrub_pair(&out) {
        out.replace_range(value_start..after_value, REDACTED);
    }
    out
}

/// Audit metadata line (plan §4: password change is "audit-logged (metadata
/// only)"). Values pass through [`scrub`] first; callers must only pass
/// non-secret metadata (action names, ids, outcomes) — never emails, tokens,
/// or credentials. Action names use underscores (`password_change`) so they
/// are not misread as `key=value` pairs.
pub fn audit(action: &str, subject_id: &str, outcome: &str) {
    eprintln!(
        "{}",
        scrub(&format!(
            "nexora-auth: audit action={action} subject={subject_id} outcome={outcome}"
        ))
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_list_matches_the_plan_and_covers_pairs() {
        assert_eq!(scrub("password=hunter2"), "password=<redacted>");
        assert_eq!(scrub("token: abc123"), "token: <redacted>");
        assert_eq!(scrub("code_verifier=\"xyz\""), "code_verifier=\"<redacted>\"");
        assert_eq!(scrub("cookie: a=b"), "cookie: <redacted>");
    }

    #[test]
    fn bearer_credentials_are_redacted_anywhere() {
        assert_eq!(
            scrub("failed with Bearer deadbeefdeadbeef on the wire"),
            "failed with Bearer <redacted> on the wire"
        );
        assert!(!scrub("Authorization: Bearer SECRETVALUE").contains("SECRETVALUE"));
    }

    #[test]
    fn scrub_is_case_insensitive_and_boundary_safe() {
        assert_eq!(scrub("Password=abc"), "Password=<redacted>");
        assert_eq!(scrub("CLIENT_SECRET: zzz"), "CLIENT_SECRET: <redacted>");
        // Not a key boundary: "password" inside another word is untouched.
        assert_eq!(scrub("mypasswordish=1"), "mypasswordish=1");
    }

    #[test]
    fn multiple_pairs_in_one_line_are_all_redacted() {
        let line = "login failed password=hunter2 and token=tok99 plus code_verifier=v1";
        let scrubbed = scrub(line);
        assert!(!scrubbed.contains("hunter2"));
        assert!(!scrubbed.contains("tok99"));
        assert!(!scrubbed.contains("v1"));
    }

    #[test]
    fn audit_metadata_passes_through_scrubbed() {
        assert_eq!(
            scrub("nexora-auth: audit action=password_change subject=abc outcome=ok"),
            "nexora-auth: audit action=password_change subject=abc outcome=ok"
        );
        // A careless caller passing a secret-looking value is still safe.
        let careless = scrub("nexora-auth: audit action=password_change token=LEAK outcome=ok");
        assert!(!careless.contains("LEAK"));
    }
}
