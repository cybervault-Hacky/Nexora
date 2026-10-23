use std::env;

fn is_loopback_host(host: &str) -> bool {
    matches!(
        host.to_ascii_lowercase().as_str(),
        "127.0.0.1" | "localhost" | "[::1]"
    )
}

fn validate_build_time_backend_url(raw: &str, is_release: bool, allow_loopback_http: bool) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }

    if trimmed.len() > 512
        || trimmed
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '"')
    {
        panic!(
            "Invalid NEXORA_AUTH_API_URL at build time: control characters, quotes, or whitespace are not allowed (value withheld)."
        );
    }

    let (scheme, rest) = trimmed.split_once("://").unwrap_or_else(|| {
        panic!(
            "Invalid NEXORA_AUTH_API_URL at build time: URL must include an explicit https:// scheme (value withheld)."
        )
    });

    if rest.contains('?') || rest.contains('#') {
        panic!(
            "Invalid NEXORA_AUTH_API_URL at build time: query strings and URL fragments are not permitted (value withheld)."
        );
    }

    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        panic!(
            "Invalid NEXORA_AUTH_API_URL at build time: host is missing or contains embedded credentials (value withheld)."
        );
    }

    let host = if authority.starts_with('[') {
        let end = authority.find(']').unwrap_or_else(|| {
            panic!(
                "Invalid NEXORA_AUTH_API_URL at build time: malformed IPv6 host (value withheld)."
            )
        });
        &authority[..=end]
    } else {
        authority.split(':').next().unwrap_or("")
    };

    match scheme.to_ascii_lowercase().as_str() {
        "https" => {}
        "http" if is_loopback_host(host) && (!is_release || allow_loopback_http) => {}
        "http" => {
            panic!(
                "Insecure NEXORA_AUTH_API_URL at build time: plaintext http:// is rejected for non-loopback hosts (and in release builds without NEXORA_ALLOW_LOOPBACK_HTTP=true)."
            );
        }
        _ => {
            panic!(
                "Invalid NEXORA_AUTH_API_URL at build time: unsupported URL scheme (expected https://; value withheld)."
            );
        }
    }

    if is_release && !allow_loopback_http {
        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("replace_with")
            || lower.contains("change_me")
            || lower.contains("nexora.local")
        {
            panic!(
                "Invalid NEXORA_AUTH_API_URL for release build: placeholder domain detected (value withheld)."
            );
        }
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=NEXORA_AUTH_API_URL");
    println!("cargo:rerun-if-env-changed=AUTH_API_BASE_URL");
    println!("cargo:rerun-if-env-changed=NEXORA_ALLOW_LOOPBACK_HTTP");
    println!("cargo:rerun-if-env-changed=GOOGLE_OAUTH_CLIENT_ID");

    let is_release = env::var("PROFILE").map(|p| p == "release").unwrap_or(false);
    let allow_loopback_http = env::var("NEXORA_ALLOW_LOOPBACK_HTTP")
        .map(|v| v.trim() == "true")
        .unwrap_or(false);

    if let Ok(raw) = env::var("NEXORA_AUTH_API_URL") {
        validate_build_time_backend_url(&raw, is_release, allow_loopback_http);
    }

    tauri_build::build()
}
