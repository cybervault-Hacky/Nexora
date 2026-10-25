//! Log scrubbing (Phase 23G — plan §4 "Log leakage" control, step 6).
//!
//! Plan §4: *"structured logging with a scrub list (password, token,
//! authorization, cookie, code_verifier); token values appear only in email
//! links"*. This module is the single scrub list every log/audit line must
//! pass through before it reaches stderr, plus the [`REDACTED`] placeholder
//! used by the custom `Debug` impls of secret-bearing types.
//!
//! The service deliberately logs almost nothing (startup capability and
//! password-change audit action/outcome); subject identifiers are withheld
//! from stderr. Scrubbing is applied anyway so future log lines cannot
//! casually leak. The `tests/secret_leakage.rs` suite asserts the list works
//! and that errors, bodies, and `Debug` output never carry secret material.

/// Placeholder printed in place of any secret-bearing value.
pub const REDACTED: &str = "<redacted>";

/// Plan §4 scrub list, lowercase: `password`, `token`, `authorization`,
/// `cookie`, `code_verifier`. Extended with the equivalent names that carry
/// the same classes of material (`code`, `secret`, `client_secret`, `state`,
/// `nonce`, `key`, `hash`, `session`) — same intent, no scope expansion.
const SCRUB_KEYS: &[&str] = &[
    "password",
    "passwd",
    "access_token",
    "refresh_token",
    "id_token",
    "token",
    "authorization",
    "cookie",
    "code_verifier",
    "authorization_code",
    "code",
    "client_secret",
    "webhook_secret",
    "webhook_signature",
    "secret_key",
    "private_key",
    "api_key",
    "signature",
    "v1",
    "v0",
    "secret",
    "state",
    "nonce",
    "key",
    "hash",
    "session",
    "database_url",
    "connection_string",
    "url",
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
                    .map(|c| c.is_ascii_alphanumeric())
                    .unwrap_or(false);
            let boundary_after = !lowered[after_key..]
                .chars()
                .next()
                .map(|c| c.is_ascii_alphanumeric())
                .unwrap_or(false);
            if boundary_before && boundary_after {
                let rest = &line[after_key..];
                if let Some(first) = rest.chars().next() {
                    if first == '=' || first == ':' {
                        let trimmed = rest.trim_start_matches(['=', ':', ' ', '\t', '"', '\'']);
                        let value_start = line.len() - trimmed.len();
                        let after_value = line[value_start..]
                            .find(|c: char| {
                                matches!(
                                    c,
                                    '"' | '\'' | ',' | ' ' | '\n' | '\t' | ';' | '}' | ')' | '&'
                                )
                            })
                            .map(|offset| value_start + offset)
                            .unwrap_or(line.len());
                        if after_value > value_start && &line[value_start..after_value] != REDACTED
                        {
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

    // Connection URIs carrying userinfo (`scheme://user:pass@host` or `scheme://user@host`).
    let mut uri_scan = 0;
    while let Some(pos) = out[uri_scan..].find("://") {
        let after_scheme = uri_scan + pos + 3;
        let authority_end = out[after_scheme..]
            .find(['/', '?', '#', '"', '\'', ' ', '\t', '\n', ',', ';'])
            .map(|o| after_scheme + o)
            .unwrap_or(out.len());
        if let Some(at_rel) = out[after_scheme..authority_end].rfind('@') {
            let at_pos = after_scheme + at_rel;
            if &out[after_scheme..at_pos] != REDACTED {
                out.replace_range(after_scheme..at_pos, REDACTED);
                uri_scan = after_scheme + REDACTED.len() + 1;
                continue;
            }
        }
        uri_scan = authority_end;
    }

    // One pair per pass (offsets stay valid because each pass re-scans).
    while let Some((value_start, after_value)) = find_scrub_pair(&out) {
        out.replace_range(value_start..after_value, REDACTED);
    }
    out
}

/// Format the plan-mandated password-change audit line without writing an
/// account identifier to stderr. Action names use underscores
/// (`password_change`) so they are not misread as scrub-list key/value pairs.
fn audit_line(action: &str, _subject_id: &str, outcome: &str) -> String {
    scrub(&format!(
        "nexora-auth: audit action={action} subject={REDACTED} outcome={outcome}"
    ))
}

pub fn audit(action: &str, subject_id: &str, outcome: &str) {
    eprintln!("{}", audit_line(action, subject_id, outcome));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_list_matches_the_plan_and_covers_pairs() {
        assert_eq!(scrub("password=hunter2"), "password=<redacted>");
        assert_eq!(scrub("token: abc123"), "token: <redacted>");
        assert_eq!(
            scrub("code_verifier=\"xyz\""),
            "code_verifier=\"<redacted>\""
        );
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
    fn database_urls_and_payment_signatures_are_scrubbed() {
        let database = scrub(
            "NEXORA_AUTH_DATABASE_URL=postgres://db_user:SENTINEL_db_pass@db.internal/app",
        );
        assert!(!database.contains("SENTINEL_db_pass"));
        assert!(!database.contains("db_user"));

        let signature = scrub(
            "Stripe-Signature: t=123456,v1=SENTINEL_payment_signature",
        );
        assert!(!signature.contains("SENTINEL_payment_signature"));

        let environment_secret = scrub("STRIPE_WEBHOOK_SECRET=SENTINEL_webhook_secret");
        assert!(!environment_secret.contains("SENTINEL_webhook_secret"));
    }

    #[test]
    fn audit_metadata_redacts_subject_identifiers() {
        let line = audit_line("password_change", "SENTINEL_user_id", "ok");
        assert_eq!(
            line,
            "nexora-auth: audit action=password_change subject=<redacted> outcome=ok"
        );
        assert!(!line.contains("SENTINEL"));

        // A careless caller passing a secret-looking value is still safe.
        let careless = scrub("nexora-auth: audit action=password_change token=LEAK outcome=ok");
        assert!(!careless.contains("LEAK"));
    }
}
