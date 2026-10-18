//! Configuration for the standalone nexora-auth service.
//!
//! Design rules (Phase 23A):
//! - Loopback bind by default (development safety).
//! - Non-loopback binds require an explicit, validated opt-in flag.
//! - Invalid addresses/ports produce actionable, secret-free errors that never
//!   echo raw environment values.

use std::fmt;
use std::net::SocketAddr;

/// Default development bind: loopback only. Not a production address.
pub const DEFAULT_BIND: &str = "127.0.0.1:8787";

/// Environment variable holding the bind address (`IP:PORT`).
pub const BIND_VAR: &str = "NEXORA_AUTH_BIND";

/// Environment variable that must be exactly `true` before any non-loopback
/// bind is accepted. Defaults to `false`.
pub const ALLOW_NON_LOOPBACK_VAR: &str = "NEXORA_AUTH_ALLOW_NON_LOOPBACK";

/// Optional database URL (`postgres://` or `postgresql://`). Application
/// configuration only — actual secret storage belongs to the deployment
/// environment, never to this process's logs or this repository. The value is
/// never echoed in any error message (connection strings contain credentials).
pub const DATABASE_URL_VAR: &str = "NEXORA_AUTH_DATABASE_URL";

/// Phase 23C: email-verification token lifetime (plan §7 `VERIFICATION_TTL_SECONDS`).
pub const VERIFICATION_TTL_VAR: &str = "NEXORA_AUTH_VERIFICATION_TTL_SECONDS";
/// Phase 23C: delivery provider selection. Only `unconfigured` exists today.
pub const EMAIL_PROVIDER_VAR: &str = "NEXORA_AUTH_EMAIL_PROVIDER";
/// Phase 23C: from-address for verification mail (treated as sensitive).
pub const EMAIL_FROM_VAR: &str = "NEXORA_AUTH_EMAIL_FROM";
/// Reserved for a future SMTP transport. Any value set today is a startup
/// error — silently ignoring credentials would fake delivery.
pub const SMTP_VARS: [&str; 4] = [
    "NEXORA_AUTH_SMTP_HOST",
    "NEXORA_AUTH_SMTP_PORT",
    "NEXORA_AUTH_SMTP_USERNAME",
    "NEXORA_AUTH_SMTP_PASSWORD",
];
/// Phase 23D: absolute session lifetime (plan §7 `SESSION_TTL_SECONDS`).
pub const SESSION_TTL_VAR: &str = "NEXORA_AUTH_SESSION_TTL_SECONDS";
/// Phase 23D: idle session lifetime (plan §7 `SESSION_IDLE_TTL_SECONDS`).
pub const SESSION_IDLE_TTL_VAR: &str = "NEXORA_AUTH_SESSION_IDLE_TTL_SECONDS";
/// Phase 23D: auth rate-limit windows (plan §7 `AUTH_RATE_LIMIT_*`, `5/15m`).
pub const RATE_LIMIT_LOGIN_VAR: &str = "NEXORA_AUTH_RATE_LIMIT_LOGIN";
pub const RATE_LIMIT_REGISTER_VAR: &str = "NEXORA_AUTH_RATE_LIMIT_REGISTER";
pub const RATE_LIMIT_RESEND_VAR: &str = "NEXORA_AUTH_RATE_LIMIT_RESEND";
/// Phase 23E: password-reset token lifetime (plan §7 `RESET_TTL_SECONDS`).
pub const RESET_TTL_VAR: &str = "NEXORA_AUTH_RESET_TTL_SECONDS";
/// Phase 23E: recovery rate-limit window (plan §7 `AUTH_RATE_LIMIT_RECOVERY`).
pub const RATE_LIMIT_RECOVERY_VAR: &str = "NEXORA_AUTH_RATE_LIMIT_RECOVERY";
/// Phase 23F: Google OAuth client id (plan §8 `GOOGLE_OAUTH_CLIENT_ID`).
pub const GOOGLE_CLIENT_ID_VAR: &str = "NEXORA_AUTH_GOOGLE_CLIENT_ID";
/// Phase 23F: Google OAuth client secret (plan §8: **server secret store
/// only** — never in the repo, binary, or WebView).
pub const GOOGLE_CLIENT_SECRET_VAR: &str = "NEXORA_AUTH_GOOGLE_CLIENT_SECRET";
/// Phase 23F: registered loopback redirect (plan §8 `GOOGLE_OAUTH_REDIRECT_URI`).
pub const GOOGLE_REDIRECT_URI_VAR: &str = "NEXORA_AUTH_GOOGLE_REDIRECT_URI";

#[derive(Clone, PartialEq, Eq)]
pub struct AuthConfig {
    pub bind: SocketAddr,
    pub allow_non_loopback: bool,
    /// Optional database connectivity setting (Phase 23B). Used by Phase 23C
    /// account storage; treated as a secret-bearing connection string.
    pub database_url: Option<String>,
    /// Email-verification token lifetime in seconds (Phase 23C).
    pub verification_ttl_secs: u64,
    /// From-address for verification mail (Phase 23C; none = unconfigured).
    pub email_from: Option<String>,
    /// Whether a real email transport is configured. Always false in this
    /// build — only the honest `unconfigured` provider exists.
    pub email_provider_configured: bool,
    /// Phase 23D session lifetimes (seconds).
    pub session_ttl_secs: u64,
    pub session_idle_ttl_secs: u64,
    /// Phase 23E password-reset token lifetime.
    pub reset_ttl_secs: u64,
    /// Phase 23D/23E DB-backed rate-limit windows (plan §4/§5 `login_attempts`).
    pub rate_limit_login: crate::rate_limit::RateLimitRule,
    pub rate_limit_register: crate::rate_limit::RateLimitRule,
    pub rate_limit_resend: crate::rate_limit::RateLimitRule,
    pub rate_limit_recovery: crate::rate_limit::RateLimitRule,
    /// Phase 23F Google OAuth settings (all-or-nothing; secret never echoed).
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub google_redirect_uri: Option<String>,
}

/// Redacting `Debug`: connection strings embed credentials, so they are never
/// rendered in logs or error output. From-addresses are redacted too.
impl fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthConfig")
            .field("bind", &self.bind)
            .field("allow_non_loopback", &self.allow_non_loopback)
            .field(
                "database_url",
                &self.database_url.as_ref().map(|_| "<redacted>"),
            )
            .field("verification_ttl_secs", &self.verification_ttl_secs)
            .field("email_from", &self.email_from.as_ref().map(|_| "<redacted>"))
            .field("email_provider_configured", &self.email_provider_configured)
            .field(
                "google_client_id",
                &self.google_client_id.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "google_client_secret",
                &self.google_client_secret.as_ref().map(|_| "<redacted>"),
            )
            .field("google_redirect_uri", &self.google_redirect_uri)
            .finish()
    }
}

/// Configuration failure with an actionable, secret-free message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

impl AuthConfig {
    /// Resolve configuration from optional raw strings (testable without
    /// touching process environment). `None` or empty values use defaults.
    /// Preserved Phase 23A signature: no database URL is configured.
    pub fn resolve(
        bind: Option<&str>,
        allow_non_loopback: Option<&str>,
    ) -> Result<Self, ConfigError> {
        Self::resolve_with_database(bind, allow_non_loopback, None)
    }

    /// Full resolution including the optional database URL (Phase 23B).
    /// Database values are validated but never echoed anywhere.
    pub fn resolve_with_database(
        bind: Option<&str>,
        allow_non_loopback: Option<&str>,
        database_url: Option<&str>,
    ) -> Result<Self, ConfigError> {
        let bind_raw = bind
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_BIND);

        // Parse first so invalid addresses/ports are rejected before any bind.
        let bind: SocketAddr = bind_raw.parse().map_err(|_| {
            ConfigError::new(format!(
                "{BIND_VAR} must be an IP:PORT pair such as {DEFAULT_BIND} \
                 (IPv6 example: [::1]:8787). The provided value could not be parsed."
            ))
        })?;

        if bind.port() == 0 {
            return Err(ConfigError::new(format!(
                "{BIND_VAR} must use a concrete port between 1 and 65535; port 0 is not allowed."
            )));
        }

        let allow = parse_flag(ALLOW_NON_LOOPBACK_VAR, allow_non_loopback)?;

        if !bind.ip().is_loopback() && !allow {
            return Err(ConfigError::new(format!(
                "{BIND_VAR} resolves to the non-loopback address {bind}, which is disabled by \
                 default. Keep {DEFAULT_BIND} for local development. For a deliberate deployment \
                 behind a firewall, set {ALLOW_NON_LOOPBACK_VAR}=true explicitly."
            )));
        }

        let database_url = validate_database_url(database_url)?;

        Ok(Self {
            bind,
            allow_non_loopback: allow,
            database_url,
            verification_ttl_secs: crate::registration::DEFAULT_VERIFICATION_TTL_SECS,
            email_from: None,
            email_provider_configured: false,
            session_ttl_secs: crate::session::DEFAULT_SESSION_TTL_SECS,
            session_idle_ttl_secs: crate::session::DEFAULT_SESSION_IDLE_TTL_SECS,
            reset_ttl_secs: crate::recovery::DEFAULT_RESET_TTL_SECS,
            rate_limit_login: crate::rate_limit::default_login_rule(),
            rate_limit_register: crate::rate_limit::default_register_rule(),
            rate_limit_resend: crate::rate_limit::default_resend_rule(),
            rate_limit_recovery: crate::rate_limit::default_recovery_rule(),
            google_client_id: None,
            google_client_secret: None,
            google_redirect_uri: None,
        })
    }

    /// Phase 23C email/verification settings (validated; values never echoed).
    pub fn apply_email_settings(&mut self, settings: EmailSettings) -> Result<(), ConfigError> {
        self.verification_ttl_secs = settings.verification_ttl_secs;
        self.reset_ttl_secs = settings.reset_ttl_secs;
        self.email_from = settings.email_from;
        self.email_provider_configured = settings.email_provider_configured;
        Ok(())
    }

    /// Phase 23D session + rate-limit settings (validated; values never echoed).
    /// Phase 23F Google OAuth settings (validated; secret never echoed).
    pub fn apply_google_settings(&mut self, settings: GoogleSettings) -> Result<(), ConfigError> {
        self.google_client_id = settings.client_id;
        self.google_client_secret = settings.client_secret;
        self.google_redirect_uri = settings.redirect_uri;
        Ok(())
    }

    pub fn apply_session_settings(&mut self, settings: SessionSettings) -> Result<(), ConfigError> {
        self.session_ttl_secs = settings.session_ttl_secs;
        self.session_idle_ttl_secs = settings.session_idle_ttl_secs;
        self.rate_limit_login = settings.rate_limit_login;
        self.rate_limit_register = settings.rate_limit_register;
        self.rate_limit_resend = settings.rate_limit_resend;
        self.rate_limit_recovery = settings.rate_limit_recovery;
        Ok(())
    }

    /// Resolve from the process environment. Never logs or prints values.
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind = std::env::var(BIND_VAR).ok();
        let allow = std::env::var(ALLOW_NON_LOOPBACK_VAR).ok();
        let database_url = std::env::var(DATABASE_URL_VAR).ok();
        let mut config =
            Self::resolve_with_database(bind.as_deref(), allow.as_deref(), database_url.as_deref())?;
        config.apply_email_settings(EmailSettings::from_env()?)?;
        config.apply_session_settings(SessionSettings::from_env()?)?;
        config.apply_google_settings(GoogleSettings::from_env()?)?;
        Ok(config)
    }
}

/// Phase 23C email settings after validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailSettings {
    pub verification_ttl_secs: u64,
    /// Password-reset token lifetime (seconds) — Phase 23E.
    pub reset_ttl_secs: u64,
    pub email_from: Option<String>,
    pub email_provider_configured: bool,
}

impl EmailSettings {
    /// Read and validate from the process environment. Errors name variables
    /// and expected shapes — never secret values.
    pub fn from_env() -> Result<Self, ConfigError> {
        let smtp_set = SMTP_VARS.iter().any(|var| {
            std::env::var(var)
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
        });
        let ttl = std::env::var(VERIFICATION_TTL_VAR).ok();
        let reset_ttl = std::env::var(RESET_TTL_VAR).ok();
        let provider = std::env::var(EMAIL_PROVIDER_VAR).ok();
        let from = std::env::var(EMAIL_FROM_VAR).ok();
        Self::resolve(
            ttl.as_deref(),
            reset_ttl.as_deref(),
            provider.as_deref(),
            from.as_deref(),
            smtp_set,
        )
    }

    /// Testable resolution. `smtp_configured` reports whether any reserved
    /// `NEXORA_AUTH_SMTP_*` variable is set (computed by `from_env`).
    /// `email_provider_configured` is always false in this build: only the
    /// honest `unconfigured` transport exists (no mail).
    pub fn resolve(
        verification_ttl: Option<&str>,
        reset_ttl: Option<&str>,
        provider: Option<&str>,
        email_from: Option<&str>,
        smtp_configured: bool,
    ) -> Result<Self, ConfigError> {
        // Reserved SMTP credentials: reject loudly instead of ignoring them,
        // which would look like configured delivery.
        if smtp_configured {
            return Err(ConfigError::new(format!(
                "One of {} is set, but an SMTP transport is not implemented in this build — \
                 no mail is sent. Remove the SMTP settings or deploy a build that includes \
                 a real provider. (Value withheld.)",
                SMTP_VARS.join(", ")
            )));
        }

        let verification_ttl_secs = match verification_ttl.map(str::trim) {
            None | Some("") => crate::registration::DEFAULT_VERIFICATION_TTL_SECS,
            Some(raw) => {
                let parsed: u64 = raw.parse().map_err(|_| {
                    ConfigError::new(format!(
                        "{VERIFICATION_TTL_VAR} must be a whole number of seconds. \
                         The provided value could not be parsed. (Value withheld.)"
                    ))
                })?;
                if !(crate::registration::MIN_VERIFICATION_TTL_SECS
                    ..=crate::registration::MAX_VERIFICATION_TTL_SECS)
                    .contains(&parsed)
                {
                    return Err(ConfigError::new(format!(
                        "{VERIFICATION_TTL_VAR} must be between \
                         {} and {} seconds. (Value withheld.)",
                        crate::registration::MIN_VERIFICATION_TTL_SECS,
                        crate::registration::MAX_VERIFICATION_TTL_SECS
                    )));
                }
                parsed
            }
        };

        let reset_ttl_secs = match reset_ttl.map(str::trim) {
            None | Some("") => crate::recovery::DEFAULT_RESET_TTL_SECS,
            Some(raw) => {
                let parsed: u64 = raw.parse().map_err(|_| {
                    ConfigError::new(format!(
                        "{RESET_TTL_VAR} must be a whole number of seconds. \
                         The provided value could not be parsed. (Value withheld.)"
                    ))
                })?;
                if !(crate::recovery::MIN_RESET_TTL_SECS..=crate::recovery::MAX_RESET_TTL_SECS)
                    .contains(&parsed)
                {
                    return Err(ConfigError::new(format!(
                        "{RESET_TTL_VAR} must be between \
                         {} and {} seconds. (Value withheld.)",
                        crate::recovery::MIN_RESET_TTL_SECS,
                        crate::recovery::MAX_RESET_TTL_SECS
                    )));
                }
                parsed
            }
        };

        let email_from = match email_from.map(str::trim) {
            None | Some("") => None,
            Some(raw) => {
                if raw.split_whitespace().count() != 1 || !raw.contains('@') || raw.len() > 254 {
                    return Err(ConfigError::new(format!(
                        "{EMAIL_FROM_VAR} must be a single email-shaped address. \
                         The provided value could not be accepted. (Value withheld.)"
                    )));
                }
                Some(raw.to_string())
            }
        };

        let email_provider_configured = match provider.map(str::trim) {
            None | Some("") | Some("unconfigured") => false,
            Some(other) => {
                return Err(ConfigError::new(format!(
                    "{EMAIL_PROVIDER_VAR} value '{other}' is not supported in this build. \
                     Supported: 'unconfigured' (no mail is sent). Credentials for a real \
                     provider must never be placed in the repository."
                )));
            }
        };

        Ok(Self {
            verification_ttl_secs,
            reset_ttl_secs,
            email_from,
            email_provider_configured,
        })
    }
}

/// Validate an optional database URL: `postgres://` or `postgresql://` scheme
/// with a non-empty authority. Errors name the variable and expected shape but
/// **never** include any part of the provided value (it may embed a password).
fn validate_database_url(value: Option<&str>) -> Result<Option<String>, ConfigError> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(raw) => {
            let rest = raw
                .strip_prefix("postgres://")
                .or_else(|| raw.strip_prefix("postgresql://"))
                .ok_or_else(|| {
                    ConfigError::new(format!(
                        "{DATABASE_URL_VAR} must start with postgres:// or postgresql://. \
                         The provided value cannot be used (value withheld)."
                    ))
                })?;
            let authority = rest.split(['/', '?']).next().unwrap_or("");
            if authority.is_empty() {
                return Err(ConfigError::new(format!(
                    "{DATABASE_URL_VAR} must include a host (value withheld)."
                )));
            }
            Ok(Some(raw.to_string()))
        }
    }
}

/// Strict boolean parsing: exactly `true` or `false` (after trim); unset or
/// empty means `false`. Anything else is rejected with an actionable message
/// that names the variable but never echoes the provided value.
fn parse_flag(var: &str, value: Option<&str>) -> Result<bool, ConfigError> {
    match value.map(str::trim) {
        None | Some("") => Ok(false),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(_) => Err(ConfigError::new(format!(
            "{var} must be exactly 'true' or 'false'."
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_loopback_bind() {
        let config = AuthConfig::resolve(None, None).unwrap();
        assert_eq!(config.bind, DEFAULT_BIND.parse::<SocketAddr>().unwrap());
        assert!(config.bind.ip().is_loopback());
        assert!(!config.allow_non_loopback);
    }

    #[test]
    fn empty_values_fall_back_to_defaults() {
        let config = AuthConfig::resolve(Some("   "), Some("")).unwrap();
        assert_eq!(config.bind, DEFAULT_BIND.parse::<SocketAddr>().unwrap());
        assert!(!config.allow_non_loopback);
    }

    #[test]
    fn explicit_loopback_binds_are_accepted() {
        let config = AuthConfig::resolve(Some("127.0.0.1:9000"), None).unwrap();
        assert_eq!(config.bind.port(), 9000);

        let v6 = AuthConfig::resolve(Some("[::1]:8787"), None).unwrap();
        assert!(v6.bind.ip().is_loopback());
    }

    #[test]
    fn rejects_unparseable_addresses_and_ports() {
        for bad in [
            "not-an-address",
            "127.0.0.1",
            "127.0.0.1:",
            ":8787",
            "256.0.0.1:80",
            "127.0.0.1:70000",
            "127.0.0.1:-1",
            "http://127.0.0.1:8787",
        ] {
            let error = AuthConfig::resolve(Some(bad), None).unwrap_err();
            assert!(error.message().contains(BIND_VAR), "case: {bad}");
            // Errors must never echo the provided value (it could be anything).
            assert!(
                !error.message().contains(bad),
                "error must not echo input: {bad}"
            );
        }
    }

    #[test]
    fn rejects_port_zero() {
        let error = AuthConfig::resolve(Some("127.0.0.1:0"), None).unwrap_err();
        assert!(error.message().contains("port"));
    }

    #[test]
    fn rejects_non_loopback_without_explicit_opt_in() {
        for bad in ["0.0.0.0:8787", "192.168.1.10:8787", "[::]:8787"] {
            let error = AuthConfig::resolve(Some(bad), None).unwrap_err();
            assert!(error.message().contains(ALLOW_NON_LOOPBACK_VAR), "case: {bad}");
            assert!(!error.message().contains("0.0.0.0:8787=something"));
        }
    }

    #[test]
    fn allows_non_loopback_with_explicit_opt_in() {
        let config = AuthConfig::resolve(Some("0.0.0.0:8787"), Some("true")).unwrap();
        assert_eq!(config.bind.ip().to_string(), "0.0.0.0");
        assert!(config.allow_non_loopback);
    }

    #[test]
    fn rejects_invalid_opt_in_values() {
        for bad in ["yes", "1", "TRUE", "on"] {
            let error = AuthConfig::resolve(None, Some(bad)).unwrap_err();
            assert!(error.message().contains(ALLOW_NON_LOOPBACK_VAR), "case: {bad}");
            assert!(!error.message().contains(bad), "must not echo input");
        }
        assert!(AuthConfig::resolve(None, Some("false")).is_ok());
    }

    #[test]
    fn error_messages_are_secret_free() {
        let error = AuthConfig::resolve(Some("hunter2-should-not-appear"), None).unwrap_err();
        assert!(!error.message().contains("hunter2"));
        assert!(!error.message().contains("NEXORA_AUTH_ALLOW_NON_LOOPBACK=true hunter2"));
    }

    #[test]
    fn database_url_is_optional_and_validated() {
        assert_eq!(AuthConfig::resolve(None, None).unwrap().database_url, None);
        assert_eq!(
            AuthConfig::resolve_with_database(None, None, Some("   "))
                .unwrap()
                .database_url,
            None
        );

        let with_db = AuthConfig::resolve_with_database(
            None,
            None,
            Some("postgres://user@localhost:5432/nexora_auth_test"),
        )
        .unwrap();
        assert_eq!(
            with_db.database_url.as_deref(),
            Some("postgres://user@localhost:5432/nexora_auth_test")
        );

        let alt_scheme =
            AuthConfig::resolve_with_database(None, None, Some("postgresql://db.internal/app"))
                .unwrap();
        assert!(alt_scheme.database_url.is_some());
    }

    #[test]
    fn rejects_invalid_database_urls_without_echoing_values() {
        for bad in [
            "sqlite://data.db",
            "mysql://user:pass@host/db",
            "postgres://",
            "postgresql://",
            "just-a-string",
            "hunter2://secret-value@host/db",
        ] {
            let error = AuthConfig::resolve_with_database(None, None, Some(bad)).unwrap_err();
            assert!(error.message().contains(DATABASE_URL_VAR), "case: {bad}");
            assert!(!error.message().contains(bad), "must not echo input");
            assert!(!error.message().contains("hunter2"));
            assert!(!error.message().contains("secret-value"));
        }
    }

    #[test]
    fn debug_output_redacts_database_url() {
        let config = AuthConfig::resolve_with_database(
            None,
            None,
            Some("postgres://user:super-secret@localhost:5432/app"),
        )
        .unwrap();
        let rendered = format!("{config:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("super-secret"));
        assert!(!rendered.contains("postgres://user"));
    }
}

#[cfg(test)]
mod email_settings_tests {
    use super::*;

    #[test]
    fn email_settings_default_to_honest_unconfigured() {
        let settings = EmailSettings::resolve(None, None, None, None, false).unwrap();
        assert_eq!(
            settings.verification_ttl_secs,
            crate::registration::DEFAULT_VERIFICATION_TTL_SECS
        );
        assert_eq!(settings.email_from, None);
        assert!(!settings.email_provider_configured);
    }

    #[test]
    fn verification_ttl_is_validated_without_echoing_values() {
        assert_eq!(
            EmailSettings::resolve(Some("3600"), None, None, None, false)
                .unwrap()
                .verification_ttl_secs,
            3600
        );
        let bad = EmailSettings::resolve(Some("not-a-number"), None, None, None, false).unwrap_err();
        assert!(bad.message().contains(VERIFICATION_TTL_VAR));
        assert!(bad.message().contains("withheld"));
        assert!(!bad.message().contains("not-a-number"));
        let too_small =
            EmailSettings::resolve(Some("5"), None, None, None, false).unwrap_err();
        assert!(too_small.message().contains(VERIFICATION_TTL_VAR));
        assert!(!too_small.message().contains("5 seconds"));
    }

    #[test]
    fn only_unconfigured_provider_is_accepted_and_smtp_is_rejected_honestly() {
        assert!(EmailSettings::resolve(None, None, Some("unconfigured"), None, false).is_ok());
        let unsupported =
            EmailSettings::resolve(None, None, Some("smtp"), None, false).unwrap_err();
        assert!(unsupported.message().contains(EMAIL_PROVIDER_VAR));
        assert!(unsupported.message().contains("unconfigured"));

        let smtp = EmailSettings::resolve(None, None, None, None, true).unwrap_err();
        assert!(smtp.message().contains("not implemented"));
        assert!(smtp.message().contains("no mail is sent"));
        assert!(smtp.message().contains("withheld"));
    }

    #[test]
    fn email_from_shape_is_checked_and_redacted() {
        assert_eq!(
            EmailSettings::resolve(None, None, None, Some("ops@example.test"), false)
                .unwrap()
                .email_from
                .as_deref(),
            Some("ops@example.test")
        );
        let bad = EmailSettings::resolve(None, None, None, Some("two words@example.test"), false)
            .unwrap_err();
        assert!(bad.message().contains(EMAIL_FROM_VAR));
        assert!(!bad.message().contains("two words"));

        let mut config = AuthConfig::resolve(None, None).unwrap();
        config
            .apply_email_settings(
                EmailSettings::resolve(None, None, None, Some("ops@example.test"), false).unwrap(),
            )
            .unwrap();
        let rendered = format!("{config:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("ops@example.test"));
    }
}

/// Phase 23D session + rate-limit settings after validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSettings {
    pub session_ttl_secs: u64,
    pub session_idle_ttl_secs: u64,
    pub rate_limit_login: crate::rate_limit::RateLimitRule,
    pub rate_limit_register: crate::rate_limit::RateLimitRule,
    pub rate_limit_resend: crate::rate_limit::RateLimitRule,
    pub rate_limit_recovery: crate::rate_limit::RateLimitRule,
}

impl SessionSettings {
    /// Read and validate from the process environment. Errors name variables
    /// and expected shapes — never secret values.
    pub fn from_env() -> Result<Self, ConfigError> {
        let ttl = std::env::var(SESSION_TTL_VAR).ok();
        let idle = std::env::var(SESSION_IDLE_TTL_VAR).ok();
        let login = std::env::var(RATE_LIMIT_LOGIN_VAR).ok();
        let register = std::env::var(RATE_LIMIT_REGISTER_VAR).ok();
        let resend = std::env::var(RATE_LIMIT_RESEND_VAR).ok();
        let recovery = std::env::var(RATE_LIMIT_RECOVERY_VAR).ok();
        Self::resolve(
            ttl.as_deref(),
            idle.as_deref(),
            login.as_deref(),
            register.as_deref(),
            resend.as_deref(),
            recovery.as_deref(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        session_ttl: Option<&str>,
        session_idle_ttl: Option<&str>,
        rate_limit_login: Option<&str>,
        rate_limit_register: Option<&str>,
        rate_limit_resend: Option<&str>,
        rate_limit_recovery: Option<&str>,
    ) -> Result<Self, ConfigError> {
        let session_ttl_secs = parse_ttl(SESSION_TTL_VAR, session_ttl)?;
        let session_idle_ttl_secs = parse_ttl(SESSION_IDLE_TTL_VAR, session_idle_ttl)?;
        let rate_limit_login = parse_rule(RATE_LIMIT_LOGIN_VAR, rate_limit_login)?;
        let rate_limit_register = parse_rule(RATE_LIMIT_REGISTER_VAR, rate_limit_register)?;
        let rate_limit_resend = parse_rule(RATE_LIMIT_RESEND_VAR, rate_limit_resend)?;
        let rate_limit_recovery = parse_rule(RATE_LIMIT_RECOVERY_VAR, rate_limit_recovery)?;
        Ok(Self {
            session_ttl_secs,
            session_idle_ttl_secs,
            rate_limit_login,
            rate_limit_register,
            rate_limit_resend,
            rate_limit_recovery,
        })
    }
}

fn parse_ttl(var: &str, raw: Option<&str>) -> Result<u64, ConfigError> {
    match raw.map(str::trim) {
        None | Some("") => Ok(match var {
            SESSION_TTL_VAR => crate::session::DEFAULT_SESSION_TTL_SECS,
            _ => crate::session::DEFAULT_SESSION_IDLE_TTL_SECS,
        }),
        Some(value) => {
            let parsed: u64 = value.parse().map_err(|_| {
                ConfigError::new(format!(
                    "{var} must be a whole number of seconds. The provided value could not \
                     be parsed. (Value withheld.)"
                ))
            })?;
            if !(crate::session::MIN_SESSION_TTL_SECS..=crate::session::MAX_SESSION_TTL_SECS)
                .contains(&parsed)
            {
                return Err(ConfigError::new(format!(
                    "{var} must be between {} and {} seconds. (Value withheld.)",
                    crate::session::MIN_SESSION_TTL_SECS,
                    crate::session::MAX_SESSION_TTL_SECS
                )));
            }
            Ok(parsed)
        }
    }
}

fn parse_rule(
    var: &str,
    raw: Option<&str>,
) -> Result<crate::rate_limit::RateLimitRule, ConfigError> {
    match raw.map(str::trim) {
        None | Some("") => Ok(match var {
            RATE_LIMIT_LOGIN_VAR => crate::rate_limit::default_login_rule(),
            RATE_LIMIT_REGISTER_VAR => crate::rate_limit::default_register_rule(),
            RATE_LIMIT_RECOVERY_VAR => crate::rate_limit::default_recovery_rule(),
            _ => crate::rate_limit::default_resend_rule(),
        }),
        Some(value) => crate::rate_limit::parse_rate_limit(value).map_err(ConfigError::new),
    }
}

#[cfg(test)]
mod session_settings_tests {
    use super::*;

    #[test]
    fn session_settings_default_to_plan_values() {
        let settings = SessionSettings::resolve(None, None, None, None, None, None).unwrap();
        assert_eq!(settings.session_ttl_secs, crate::session::DEFAULT_SESSION_TTL_SECS);
        assert_eq!(
            settings.session_idle_ttl_secs,
            crate::session::DEFAULT_SESSION_IDLE_TTL_SECS
        );
        assert_eq!(settings.rate_limit_login, crate::rate_limit::default_login_rule());
    }

    #[test]
    fn session_ttls_are_validated_without_echoing_values() {
        assert_eq!(
            SessionSettings::resolve(Some("3600"), Some("600"), None, None, None, None)
                .unwrap()
                .session_ttl_secs,
            3_600
        );
        let bad = SessionSettings::resolve(Some("abc"), None, None, None, None, None).unwrap_err();
        assert!(bad.message().contains(SESSION_TTL_VAR));
        assert!(bad.message().contains("withheld"));
        assert!(!bad.message().contains("abc"));
        let out_of_range = SessionSettings::resolve(Some("5"), None, None, None, None, None).unwrap_err();
        assert!(out_of_range.message().contains(SESSION_TTL_VAR));
    }

    #[test]
    fn rate_limit_rules_parse_from_env_shape() {
        let settings =
            SessionSettings::resolve(None, None, Some("5/15m"), Some("3/60m"), None, None).unwrap();
        assert_eq!(
            settings.rate_limit_login,
            crate::rate_limit::RateLimitRule { max_failures: 5, window_secs: 900 }
        );
        assert_eq!(
            settings.rate_limit_register,
            crate::rate_limit::RateLimitRule { max_failures: 3, window_secs: 3_600 }
        );
        let bad = SessionSettings::resolve(None, None, Some("nope"), None, None, None).unwrap_err();
        assert!(bad.message().contains("withheld"));
        assert!(!bad.message().contains("nope"));
    }
}

#[cfg(test)]
mod recovery_settings_tests {
    use super::*;

    #[test]
    fn reset_ttl_defaults_to_the_plan_hour_and_is_validated() {
        let settings = EmailSettings::resolve(None, None, None, None, false).unwrap();
        assert_eq!(settings.reset_ttl_secs, crate::recovery::DEFAULT_RESET_TTL_SECS);

        let custom =
            EmailSettings::resolve(None, Some("7200"), None, None, false).unwrap();
        assert_eq!(custom.reset_ttl_secs, 7_200);

        let bad = EmailSettings::resolve(None, Some("abc"), None, None, false).unwrap_err();
        assert!(bad.message().contains(RESET_TTL_VAR));
        assert!(bad.message().contains("withheld"));
        assert!(!bad.message().contains("abc"));

        let out_of_range =
            EmailSettings::resolve(None, Some("5"), None, None, false).unwrap_err();
        assert!(out_of_range.message().contains(RESET_TTL_VAR));
    }

    #[test]
    fn recovery_rate_rule_parses_from_env_shape() {
        let settings =
            SessionSettings::resolve(None, None, None, None, None, Some("5/15m")).unwrap();
        assert_eq!(
            settings.rate_limit_recovery,
            crate::rate_limit::RateLimitRule { max_failures: 5, window_secs: 900 }
        );
        let defaults = SessionSettings::resolve(None, None, None, None, None, None).unwrap();
        assert_eq!(defaults.rate_limit_recovery, crate::rate_limit::default_recovery_rule());
        let bad =
            SessionSettings::resolve(None, None, None, None, None, Some("nope")).unwrap_err();
        assert!(bad.message().contains("withheld"));
    }
}

/// Phase 23F Google OAuth settings after validation (plan §8). The client
/// secret is treated like `DATABASE_URL`: present-or-not in `Debug`, never in
/// errors. Supports both plan client types (unresolved decision #3): the
/// secret-less Desktop-app client (secret absent) and the web client with a
/// server-side secret.
#[derive(Clone, PartialEq, Eq)]
pub struct GoogleSettings {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_uri: Option<String>,
}

impl fmt::Debug for GoogleSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GoogleSettings")
            .field("client_id", &self.client_id.as_ref().map(|_| "<redacted>"))
            .field("client_secret", &self.client_secret.as_ref().map(|_| "<redacted>"))
            .field("redirect_uri", &self.redirect_uri)
            .finish()
    }
}

impl GoogleSettings {
    /// Read and validate from the process environment. Errors name variables
    /// and expected shapes — never secret values.
    pub fn from_env() -> Result<Self, ConfigError> {
        let client_id = std::env::var(GOOGLE_CLIENT_ID_VAR).ok();
        let client_secret = std::env::var(GOOGLE_CLIENT_SECRET_VAR).ok();
        let redirect_uri = std::env::var(GOOGLE_REDIRECT_URI_VAR).ok();
        Self::resolve(client_id.as_deref(), client_secret.as_deref(), redirect_uri.as_deref())
    }

    /// All-or-nothing: nothing set = unconfigured (honest 503 on the route);
    /// partially set = startup error (loud, values withheld). When configured,
    /// the redirect URI must be the plan's exact loopback callback shape.
    pub fn resolve(
        client_id: Option<&str>,
        client_secret: Option<&str>,
        redirect_uri: Option<&str>,
    ) -> Result<Self, ConfigError> {
        let client_id = client_id.map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
        let client_secret = client_secret.map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
        let redirect_uri = redirect_uri.map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);

        match (&client_id, &client_secret, &redirect_uri) {
            (None, None, None) => Ok(Self {
                client_id: None,
                client_secret: None,
                redirect_uri: None,
            }),
            (Some(_), _, Some(uri)) => {
                if !crate::oauth::is_loopback_redirect_uri(uri) {
                    return Err(ConfigError::new(format!(
                        "{GOOGLE_REDIRECT_URI_VAR} must be an exact loopback callback \
                         (http://127.0.0.1:<port>/callback). (Value withheld.)"
                    )));
                }
                Ok(Self {
                    client_id: client_id.clone(),
                    client_secret,
                    redirect_uri: redirect_uri.clone(),
                })
            }
            _ => Err(ConfigError::new(format!(
                "{GOOGLE_CLIENT_ID_VAR} and {GOOGLE_REDIRECT_URI_VAR} must be set together to \
                 enable Google sign-in (the client secret {GOOGLE_CLIENT_SECRET_VAR} is optional \
                 for the secret-less desktop client type). Partial configuration is rejected \
                 instead of silently disabling sign-in. (Values withheld.)"
            ))),
        }
    }
}

#[cfg(test)]
mod google_settings_tests {
    use super::*;

    #[test]
    fn google_settings_default_to_unconfigured() {
        let settings = GoogleSettings::resolve(None, None, None).unwrap();
        assert_eq!(settings.client_id, None);
        assert_eq!(settings.redirect_uri, None);
    }

    #[test]
    fn google_settings_require_the_pair_and_loopback_shape() {
        let ok = GoogleSettings::resolve(
            Some("client-id-value"),
            None,
            Some("http://127.0.0.1:49152/callback"),
        )
        .unwrap();
        assert_eq!(ok.client_id.as_deref(), Some("client-id-value"));
        assert_eq!(ok.client_secret, None, "secret-less desktop client type");

        let with_secret = GoogleSettings::resolve(
            Some("client-id-value"),
            Some("sekrit"),
            Some("http://127.0.0.1:49152/callback"),
        )
        .unwrap();
        assert_eq!(with_secret.client_secret.as_deref(), Some("sekrit"));

        // Partial configuration is a loud startup error.
        let partial = GoogleSettings::resolve(Some("client-id-value"), None, None).unwrap_err();
        assert!(partial.message().contains("together"));
        assert!(partial.message().contains("withheld"));
        assert!(!partial.message().contains("client-id-value"));

        let secret_only = GoogleSettings::resolve(None, Some("sekrit"), None).unwrap_err();
        assert!(!secret_only.message().contains("sekrit"));

        // Non-loopback redirects are refused.
        let bad_uri = GoogleSettings::resolve(
            Some("client-id-value"),
            None,
            Some("https://example.test/callback"),
        )
        .unwrap_err();
        assert!(bad_uri.message().contains(GOOGLE_REDIRECT_URI_VAR));
        assert!(!bad_uri.message().contains("example.test"));
    }

    #[test]
    fn google_debug_output_never_renders_secrets() {
        let settings = GoogleSettings::resolve(
            Some("client-id-value"),
            Some("sekrit"),
            Some("http://127.0.0.1:1/callback"),
        )
        .unwrap();
        let rendered = format!("{settings:?}");
        assert!(!rendered.contains("sekrit"));
        assert!(!rendered.contains("client-id-value"));
        assert!(rendered.contains("<redacted>"));
    }
}
