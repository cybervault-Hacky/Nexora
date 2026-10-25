use std::env;
use std::net::IpAddr;
use url::Url;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BuildEnvironment {
    Development,
    Test,
    Production,
}

impl BuildEnvironment {
    fn parse(raw: Option<&str>, is_release: bool) -> Result<Self, &'static str> {
        match raw.map(str::trim).filter(|value| !value.is_empty()) {
            Some("development") => Ok(Self::Development),
            Some("test") => Ok(Self::Test),
            Some("production") => Ok(Self::Production),
            Some(_) => Err("NEXORA_ENV must be development, test, or production (value withheld)."),
            None if is_release => Ok(Self::Production),
            None => Ok(Self::Development),
        }
    }

    fn permits_loopback_http(self, is_release: bool, explicit_opt_in: bool) -> bool {
        match self {
            Self::Production => false,
            Self::Development | Self::Test => !is_release || explicit_opt_in,
        }
    }

    fn is_production(self) -> bool {
        self == Self::Production
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Test => "test",
            Self::Production => "production",
        }
    }
}

fn read_optional_env(name: &str) -> Option<String> {
    match env::var(name) {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => {
            panic!("{name} must contain valid UTF-8 text (value withheld).")
        }
    }
}

fn parse_bool(name: &str, raw: Option<&str>) -> Result<bool, String> {
    match raw.map(str::trim) {
        None | Some("") => Ok(false),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(_) => Err(format!("{name} must be exactly true or false (value withheld).")),
    }
}

fn is_loopback_host(host: &str) -> bool {
    let address = host
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(host);
    address.eq_ignore_ascii_case("localhost")
        || address
            .parse::<IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false)
}

fn is_placeholder(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    [
        "replace_with",
        "change_me",
        "changeme",
        "placeholder",
        "nexora.local",
        "example.invalid",
        "example.test",
        "example.com",
        "example.org",
        ".invalid",
        ".test",
        "your_",
        "your-",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn validate_build_time_backend_url(
    raw: &str,
    environment: BuildEnvironment,
    is_release: bool,
    allow_loopback_http: bool,
) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }

    if trimmed.len() > 512
        || trimmed
            .chars()
            .any(|character| character.is_control() || character.is_whitespace() || character == '\\')
    {
        panic!(
            "Invalid NEXORA_AUTH_API_URL at build time: control characters or whitespace are not allowed (value withheld)."
        );
    }

    let parsed = Url::parse(trimmed).unwrap_or_else(|_| {
        panic!(
            "Invalid NEXORA_AUTH_API_URL at build time: expected a valid absolute HTTP(S) URL (value withheld)."
        )
    });
    if parsed.query().is_some() || parsed.fragment().is_some() {
        panic!(
            "Invalid NEXORA_AUTH_API_URL at build time: query strings and fragments are not permitted (value withheld)."
        );
    }

    let raw_authority = trimmed
        .split_once("://")
        .map(|(_, rest)| rest.split('/').next().unwrap_or(""))
        .unwrap_or("");
    if raw_authority.contains('@') || !parsed.username().is_empty() || parsed.password().is_some() {
        panic!(
            "Invalid NEXORA_AUTH_API_URL at build time: embedded credentials are not permitted (value withheld)."
        );
    }

    let host = parsed.host_str().unwrap_or("");
    if host.is_empty() {
        panic!("Invalid NEXORA_AUTH_API_URL at build time: host is required (value withheld).");
    }

    let loopback = is_loopback_host(host);
    match parsed.scheme() {
        "https" => {}
        "http" if loopback && environment.permits_loopback_http(is_release, allow_loopback_http) => {}
        "http" => {
            panic!(
                "Insecure NEXORA_AUTH_API_URL at build time: HTTP is permitted only for loopback services in development/test; production always requires HTTPS, and release QA also requires NEXORA_ALLOW_LOOPBACK_HTTP=true."
            );
        }
        _ => panic!(
            "Invalid NEXORA_AUTH_API_URL at build time: only https:// is supported, with an explicit loopback HTTP development/test exception (value withheld)."
        ),
    }

    if environment.is_production() && is_placeholder(trimmed) {
        panic!(
            "Invalid NEXORA_AUTH_API_URL for production: placeholder domain detected (value withheld)."
        );
    }
}

fn main() {
    for variable in [
        "NEXORA_ENV",
        "NEXORA_AUTH_API_URL",
        "AUTH_API_BASE_URL",
        "NEXORA_ALLOW_LOOPBACK_HTTP",
        "GOOGLE_OAUTH_CLIENT_ID",
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
    }

    let is_release = env::var("PROFILE")
        .map(|profile| profile == "release")
        .unwrap_or(false);
    let environment_value = read_optional_env("NEXORA_ENV");
    let environment = BuildEnvironment::parse(environment_value.as_deref(), is_release)
        .unwrap_or_else(|message| panic!("{message}"));
    println!("cargo:rustc-env=NEXORA_BUILD_ENV={}", environment.as_str());

    let loopback_opt_in = read_optional_env("NEXORA_ALLOW_LOOPBACK_HTTP");
    let allow_loopback_http = parse_bool(
        "NEXORA_ALLOW_LOOPBACK_HTTP",
        loopback_opt_in.as_deref(),
    )
    .unwrap_or_else(|message| panic!("{message}"));

    // Packaged desktop apps launched from a Start menu or Dock do not inherit
    // the shell that built them. The installed-app OAuth client ID is public,
    // unlike a client secret, so embed only a well-formed public identifier.
    // The runtime command validates the value again and permits a process
    // environment override for the public client ID.
    if let Some(raw) = read_optional_env("GOOGLE_OAUTH_CLIENT_ID") {
        let value = raw.trim();
        let safe_to_embed = !value.is_empty()
            && value.len() <= 512
            && value.ends_with(".apps.googleusercontent.com")
            && !value
                .chars()
                .any(|character| character.is_control() || character.is_whitespace())
            && !is_placeholder(value);
        if safe_to_embed {
            println!("cargo:rustc-env=NEXORA_GOOGLE_OAUTH_CLIENT_ID={value}");
        }
    }

    if let Some(raw) = read_optional_env("NEXORA_AUTH_API_URL") {
        validate_build_time_backend_url(
            &raw,
            environment,
            is_release,
            allow_loopback_http,
        );
    }

    tauri_build::build()
}
