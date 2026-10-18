//! Google OAuth exchange (Phase 23F) — plan §2.5 (planned behaviour) and §6
//! `POST /v1/auth/google/exchange {code, codeVerifier, state, redirectUri}`
//! → `200 {user, session}` | `409 email_conflict`.
//!
//! Security invariants (plan §2.5/§4):
//! - **PKCE S256 + loopback redirect:** the desktop runs the system-browser
//!   flow with a `http://127.0.0.1:<port>/callback` loopback redirect and a
//!   PKCE verifier; this service performs the code exchange and **refuses any
//!   `redirectUri` that is not an exact match** for the configured one.
//! - **State:** 128-bit random, generated with the PKCE verifier in the
//!   desktop process and mirrored here **hashed** — single-use (replays are
//!   refused), 10-minute window (plan §2.5).
//! - **Nonce:** validated against the OIDC ID token whenever the client
//!   supplies one (request field `nonce` — documented contract clarification;
//!   the desktop generates it with the authorization request).
//! - **Code exchange runs on the server** (plan §2.5: client secret lives only
//!   in the server secret store; even secret-less desktop clients route the
//!   exchange through the server for uniform auditing and linking). The
//!   provider seam ([`GoogleProvider`]) is the same honesty pattern as
//!   [`crate::email::EmailProvider`]: [`UnconfiguredGoogleProvider`] never
//!   invents identities, and tests inject a real-logic fake at this boundary.
//! - **Linking / takeover rules (plan §2.5):** identity is keyed by the
//!   immutable Google `sub` (`oauth_accounts (provider, provider_subject)`),
//!   NEVER by email. Auto-link only when the **Google-verified** email exactly
//!   matches an **already-verified** account email; unverified matches are
//!   refused (`409 email_conflict` → the plan's guided linking message);
//!   provider emails that Google itself has not verified are refused outright;
//!   no auto-link ever happens on unverified provider emails.
//! - Tokens (authorization codes, access/ID tokens, PKCE verifiers, states)
//!   are never stored, never logged, and never returned in API responses.

use serde::Serialize;
use sqlx::PgPool;

use crate::login::LoginOutcome;
use crate::registration::{self, validate_email};
use crate::session;

/// Plan §2.5: state is single-use with a 10-minute window.
pub const STATE_WINDOW_SECS: u64 = 600;

/// Identity claims taken from Google's ID token (OIDC). Access/refresh tokens
/// are deliberately NOT part of this struct and must never be stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleIdentity {
    /// Google `sub` — immutable provider subject (the linking key).
    pub subject: String,
    /// Google-reported email (compared only after normalization).
    pub email: String,
    /// Google's `email_verified` claim — the ONLY basis for trusting the email.
    pub email_verified: bool,
    /// OIDC `nonce` claim, when the response type yields one.
    pub nonce: Option<String>,
    /// Optional display name claim.
    pub name: Option<String>,
}

#[derive(Debug)]
pub enum GoogleExchangeError {
    /// The provider is not configured in this deployment (honest 503).
    NotConfigured,
    /// Google refused the exchange (bad/used/expired code, PKCE failure, …).
    /// Deliberately carries no provider detail.
    Refused,
}

/// The one network edge of the flow. Implementations must never log or return
/// raw codes/tokens.
pub trait GoogleProvider: Send + Sync {
    fn exchange_code<'a>(
        &'a self,
        code: &'a str,
        code_verifier: &'a str,
        redirect_uri: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<GoogleIdentity, GoogleExchangeError>> + Send + 'a>,
    >;
}

/// Honest default: no Google client is configured in this build, so the
/// exchange answers `dependency_unavailable` — **no identity is ever invented**.
pub struct UnconfiguredGoogleProvider;

impl GoogleProvider for UnconfiguredGoogleProvider {
    fn exchange_code<'a>(
        &'a self,
        _code: &'a str,
        _code_verifier: &'a str,
        _redirect_uri: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<GoogleIdentity, GoogleExchangeError>> + Send + 'a>,
    > {
        Box::pin(async { Err(GoogleExchangeError::NotConfigured) })
    }
}

/// Real server-side exchange against Google's token endpoint
/// (`https://oauth2.googleapis.com/token` — Google's documented OAuth 2.0
/// endpoint, not an invented value). The response is received directly from
/// Google's token endpoint over TLS with client authentication, so per OIDC
/// core the ID token's signature MAY be skipped at this hop (documented; JWKS
/// verification is step-6 hardening) — the payload claims are still decoded
/// and validated (sub/email/email_verified/nonce).
pub struct HttpGoogleProvider {
    client_id: String,
    /// Client secret — **server secret store only** (plan §8). Absent for the
    /// secret-less desktop client type (unresolved decision #3: this provider
    /// supports both shapes).
    client_secret: Option<String>,
    client: reqwest::Client,
    /// Cached Google JWKS (key rotation: refetched when stale or unknown kid).
    jwks_cache: std::sync::Mutex<Option<(std::time::Instant, JwksMaterial)>>,
}

/// JWKS cache lifetime (Google rotates keys; unknown kids force a refetch).
const JWKS_TTL: std::time::Duration = std::time::Duration::from_secs(3_600);
/// Google's documented JWKS endpoint for its OIDC ID tokens.
const GOOGLE_JWKS_URI: &str = "https://www.googleapis.com/oauth2/v3/certs";

impl HttpGoogleProvider {
    pub fn new(client_id: String, client_secret: Option<String>) -> Self {
        Self {
            client_id,
            client_secret,
            client: reqwest::Client::new(),
            jwks_cache: std::sync::Mutex::new(None),
        }
    }

    /// Fetch (or reuse) the JWKS document. `force` bypasses the cache — used
    /// when a token references a key we have not seen (rotation).
    async fn jwks_material(&self, force: bool) -> Result<JwksMaterial, GoogleExchangeError> {
        {
            let cache = self.jwks_cache.lock().unwrap();
            if let Some((fetched_at, material)) = cache.as_ref() {
                if !force && fetched_at.elapsed() < JWKS_TTL {
                    return Ok(material.clone());
                }
            }
        }
        let response = self
            .client
            .get(GOOGLE_JWKS_URI)
            .send()
            .await
            .map_err(|_| GoogleExchangeError::Refused)?;
        if !response.status().is_success() {
            return Err(GoogleExchangeError::Refused);
        }
        let material: JwksMaterial = response.json().await.map_err(|_| GoogleExchangeError::Refused)?;
        *self.jwks_cache.lock().unwrap() = Some((std::time::Instant::now(), material.clone()));
        Ok(material)
    }
}

/// ID-token claims payload (deserialized only AFTER the RS256 signature and
/// registered claims — iss/aud/exp — are verified; Phase 23G step 6).
#[derive(serde::Deserialize)]
struct IdTokenClaims {
    sub: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    email_verified: Option<bool>,
    #[serde(default)]
    nonce: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

/// Redacting `Debug` (Phase 23G): the nonce claim is CSRF material.
impl std::fmt::Debug for IdTokenClaims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdTokenClaims")
            .field("sub", &self.sub)
            .field("email", &self.email)
            .field("email_verified", &self.email_verified)
            .field("nonce", &self.nonce.as_ref().map(|_| crate::scrub::REDACTED))
            .field("name", &self.name)
            .finish()
    }
}

/// One Google verification key from the JWKS document (`kid`, RSA modulus,
/// exponent). Standard `serde` parsing — unknown fields ignored.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct JwkKey {
    pub kid: String,
    pub n: String,
    pub e: String,
}

/// JWKS document (Google's `oauth2/v3/certs`).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct JwksMaterial {
    pub keys: Vec<JwkKey>,
}

/// **Proper OIDC ID-token verification (Phase 23G, plan step 6):** RS256
/// signature against JWKS key material + registered-claim checks
/// (`iss` ∈ Google issuers, `aud` == this client, `exp` valid). Unverified
/// token payloads are never trusted — any failure is the same generic
/// refusal. Testable against injected key material (see unit tests).
pub fn verify_id_token_with(
    id_token: &str,
    client_id: &str,
    jwks: &JwksMaterial,
) -> Result<GoogleIdentity, GoogleExchangeError> {
    use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};

    let header = decode_header(id_token).map_err(|_| GoogleExchangeError::Refused)?;
    // Never accept "none" or symmetric algs: Google signs with RS256 only.
    if header.alg != Algorithm::RS256 {
        return Err(GoogleExchangeError::Refused);
    }
    let kid = header.kid.ok_or(GoogleExchangeError::Refused)?;
    let key = jwks
        .keys
        .iter()
        .find(|key| key.kid == kid)
        .ok_or(GoogleExchangeError::Refused)?;
    let decoding =
        DecodingKey::from_rsa_components(&key.n, &key.e).map_err(|_| GoogleExchangeError::Refused)?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
    validation.set_audience(&[client_id]);
    let data = decode::<IdTokenClaims>(id_token, &decoding, &validation)
        .map_err(|_| GoogleExchangeError::Refused)?;

    let claims = data.claims;
    let email = claims.email.ok_or(GoogleExchangeError::Refused)?;
    Ok(GoogleIdentity {
        subject: claims.sub,
        email,
        email_verified: claims.email_verified.unwrap_or(false),
        nonce: claims.nonce,
        name: claims.name,
    })
}

impl GoogleProvider for HttpGoogleProvider {
    fn exchange_code<'a>(
        &'a self,
        code: &'a str,
        code_verifier: &'a str,
        redirect_uri: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<GoogleIdentity, GoogleExchangeError>> + Send + 'a>,
    > {
        Box::pin(async move {
            #[derive(Serialize)]
            struct TokenForm<'a> {
                code: &'a str,
                client_id: &'a str,
                #[serde(skip_serializing_if = "Option::is_none")]
                client_secret: Option<&'a str>,
                redirect_uri: &'a str,
                grant_type: &'a str,
                code_verifier: &'a str,
            }
            #[derive(serde::Deserialize)]
            struct TokenResponse {
                id_token: String,
            }

            let form = TokenForm {
                code,
                client_id: &self.client_id,
                client_secret: self.client_secret.as_deref(),
                redirect_uri,
                grant_type: "authorization_code",
                code_verifier,
            };
            let response = self
                .client
                .post("https://oauth2.googleapis.com/token")
                .form(&form)
                .send()
                .await
                .map_err(|_| GoogleExchangeError::Refused)?;
            if !response.status().is_success() {
                return Err(GoogleExchangeError::Refused);
            }
            let body: TokenResponse = response.json().await.map_err(|_| GoogleExchangeError::Refused)?;

            // Proper signature + claim verification (step 6). On an unknown
            // kid (key rotation) refetch the JWKS once and retry.
            let verify = |material: &JwksMaterial| -> Result<GoogleIdentity, GoogleExchangeError> {
                verify_id_token_with(&body.id_token, &self.client_id, material)
            };
            match verify(&self.jwks_material(false).await?) {
                Ok(identity) => Ok(identity),
                Err(GoogleExchangeError::Refused) => {
                    verify(&self.jwks_material(true).await?)
                }
                Err(other) => Err(other),
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthError {
    /// 400 `validation_failed`.
    Validation(String),
    /// 400 `invalid_or_expired_token` — one generic failure for state
    /// mismatch/replay, redirect-URI mismatch, nonce mismatch, and any
    /// provider refusal (bad/reused code included).
    InvalidOrExpiredToken,
    /// 403 `email_unverified` — Google has not verified the provider email.
    EmailUnverified,
    /// 401 `invalid_credentials` — the linked/created account is not active.
    InvalidCredentials,
    /// 409 `email_conflict` — unverified account match; no auto-link (plan).
    EmailConflict,
    /// 503 `dependency_unavailable`.
    DependencyUnavailable,
    /// 500 `internal`.
    Internal,
}

impl OAuthError {
    pub fn code(&self) -> &'static str {
        match self {
            OAuthError::Validation(_) => "validation_failed",
            OAuthError::InvalidOrExpiredToken => "invalid_or_expired_token",
            OAuthError::EmailUnverified => "email_unverified",
            OAuthError::InvalidCredentials => "invalid_credentials",
            OAuthError::EmailConflict => "email_conflict",
            OAuthError::DependencyUnavailable => "dependency_unavailable",
            OAuthError::Internal => "internal",
        }
    }
    pub fn message(&self) -> &str {
        match self {
            OAuthError::Validation(message) => message,
            OAuthError::InvalidOrExpiredToken => {
                "Google sign-in could not be completed. Try again."
            }
            OAuthError::EmailUnverified => {
                "Your Google account's email address is not verified by Google. \
                 Verify it with Google and try again."
            }
            OAuthError::InvalidCredentials => "This account cannot sign in.",
            OAuthError::EmailConflict => {
                "An account with this email already exists. Sign in with your password to link Google."
            }
            OAuthError::DependencyUnavailable => {
                "Google sign-in is unavailable: it is not configured in this build."
            }
            OAuthError::Internal => "The request could not be completed.",
        }
    }
    pub fn status_code(&self) -> u16 {
        match self {
            OAuthError::Validation(_) => 400,
            OAuthError::InvalidOrExpiredToken => 400,
            OAuthError::EmailUnverified => 403,
            OAuthError::InvalidCredentials => 401,
            OAuthError::EmailConflict => 409,
            OAuthError::DependencyUnavailable => 503,
            OAuthError::Internal => 500,
        }
    }
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoogleExchangeRequest {
    pub code: String,
    pub code_verifier: String,
    pub state: String,
    pub redirect_uri: String,
    /// Contract clarification: OIDC nonce (validated against the ID token
    /// whenever supplied; omitted when the response type yields none).
    #[serde(default)]
    pub nonce: Option<String>,
}

/// Redacting `Debug` (Phase 23G): codes, PKCE verifiers, state, and nonce
/// must never reach logs/traces.
impl std::fmt::Debug for GoogleExchangeRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleExchangeRequest")
            .field("code", &crate::scrub::REDACTED)
            .field("code_verifier", &crate::scrub::REDACTED)
            .field("state", &crate::scrub::REDACTED)
            .field("redirect_uri", &self.redirect_uri)
            .field("nonce", &self.nonce.as_ref().map(|_| crate::scrub::REDACTED))
            .finish()
    }
}

/// PKCE `code_verifier`: RFC 7636 charset, 43–128 chars.
pub fn is_valid_code_verifier(raw: &str) -> bool {
    (43..=128).contains(&raw.len())
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~'))
}

/// Plan §2.5 state: 128-bit random — accept the URL-safe character set the
/// desktop generates (32–128 chars).
pub fn is_valid_state_shape(raw: &str) -> bool {
    (32..=128).contains(&raw.len())
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

/// Loopback redirect shape from plan §2.5/§2.5: `http://127.0.0.1:<port>/callback`.
pub fn is_loopback_redirect_uri(raw: &str) -> bool {
    let Some(rest) = raw.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    let Some((port, path)) = rest.split_once('/') else {
        return false;
    };
    !port.is_empty()
        && port.chars().all(|c| c.is_ascii_digit())
        && path == "callback"
}

/// Validate request shapes WITHOUT touching the state store or provider
/// (invalid input must not burn state or reach Google).
pub fn validate_exchange_request(
    request: &GoogleExchangeRequest,
    expected_redirect_uri: &str,
) -> Result<(), OAuthError> {
    if request.code.trim().is_empty() || request.code.len() > 4_096 {
        return Err(OAuthError::Validation(
            "Authorization code is missing or malformed.".to_string(),
        ));
    }
    if !is_valid_code_verifier(request.code_verifier.trim()) {
        return Err(OAuthError::Validation(
            "PKCE code verifier is malformed.".to_string(),
        ));
    }
    if !is_valid_state_shape(request.state.trim()) {
        return Err(OAuthError::Validation(
            "OAuth state is malformed.".to_string(),
        ));
    }
    // Plan §2.5: the exchange refuses any redirect URI that is not an exact
    // match for the registered loopback URI (hard generic failure).
    if request.redirect_uri != expected_redirect_uri || !is_loopback_redirect_uri(&request.redirect_uri)
    {
        return Err(OAuthError::InvalidOrExpiredToken);
    }
    if let Some(nonce) = &request.nonce {
        if nonce.is_empty() || nonce.len() > 256 {
            return Err(OAuthError::Validation(
                "OAuth nonce is malformed.".to_string(),
            ));
        }
    }
    Ok(())
}

/// Mirror the state hash server-side as single-use (plan §2.5). First use
/// records the sha256 digest; any replay within the window is refused. Old
/// rows are pruned after [`STATE_WINDOW_SECS`].
async fn consume_state(pool: &PgPool, raw_state: &str) -> Result<(), OAuthError> {
    let digest = session::hash_session_token(raw_state.trim());
    let mut tx = pool.begin().await.map_err(|_| OAuthError::Internal)?;
    sqlx::query("DELETE FROM google_oauth_states WHERE seen_at < now() - make_interval(secs => $1)")
        .bind(STATE_WINDOW_SECS as f64)
        .execute(&mut *tx)
        .await
        .map_err(|_| OAuthError::Internal)?;
    let inserted = sqlx::query("INSERT INTO google_oauth_states (state_hash) VALUES ($1)")
        .bind(&digest)
        .execute(&mut *tx)
        .await;
    match inserted {
        Ok(_) => {
            tx.commit().await.map_err(|_| OAuthError::Internal)?;
            Ok(())
        }
        Err(error) => {
            // Unique violation = state replay (hard generic failure); anything
            // else is a database failure.
            let replay = error
                .as_database_error()
                .and_then(|db| db.code())
                .map(|code| code.as_deref() == Some("23505"))
                .unwrap_or(false);
            if replay {
                Err(OAuthError::InvalidOrExpiredToken)
            } else {
                Err(OAuthError::Internal)
            }
        }
    }
}

fn display_name_for(identity: &GoogleIdentity) -> String {
    identity
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty() && name.len() <= 128)
        .map(str::to_string)
        .unwrap_or_else(|| {
            let local = identity.email.split('@').next().unwrap_or("Nexora user");
            if local.is_empty() || local.len() > 128 {
                "Nexora user".to_string()
            } else {
                local.to_string()
            }
        })
}

/// Full exchange flow: validate → state one-time-ness → provider exchange →
/// nonce/claims checks → linking rules → session. Success returns the same
/// outcome shape as password login.
pub async fn google_exchange(
    pool: Option<&PgPool>,
    google: &dyn GoogleProvider,
    expected_redirect_uri: &str,
    request: &GoogleExchangeRequest,
) -> Result<LoginOutcome, OAuthError> {
    validate_exchange_request(request, expected_redirect_uri)?;
    let pool = pool.ok_or(OAuthError::DependencyUnavailable)?;

    // Single-use state (replay ⇒ hard generic failure) BEFORE the exchange.
    consume_state(pool, &request.state).await?;

    let identity = match google
        .exchange_code(
            request.code.trim(),
            request.code_verifier.trim(),
            &request.redirect_uri,
        )
        .await
    {
        Ok(identity) => identity,
        Err(GoogleExchangeError::NotConfigured) => {
            return Err(OAuthError::DependencyUnavailable)
        }
        Err(GoogleExchangeError::Refused) => return Err(OAuthError::InvalidOrExpiredToken),
    };

    // Nonce (OIDC): hard failure on mismatch or on a missing claim when the
    // client supplied one.
    if let Some(expected_nonce) = request.nonce.as_deref() {
        match identity.nonce.as_deref() {
            Some(actual) if actual == expected_nonce => {}
            _ => return Err(OAuthError::InvalidOrExpiredToken),
        }
    }

    // Claim hygiene: subject + Google-verified email are required.
    if identity.subject.trim().is_empty() || identity.subject.len() > 255 {
        return Err(OAuthError::InvalidOrExpiredToken);
    }
    // Plan §2.5: NEVER auto-link (or create) on unverified provider emails.
    if !identity.email_verified {
        return Err(OAuthError::EmailUnverified);
    }
    let email = validate_email(&identity.email).map_err(OAuthError::Validation)?;

    let mut tx = pool.begin().await.map_err(|_| OAuthError::Internal)?;

    // Identity is keyed by immutable `sub`, never by email (plan §2.5).
    let linked: Option<(String,)> = sqlx::query_as(
        "SELECT user_id::text FROM oauth_accounts \
         WHERE provider = 'google' AND provider_subject = $1",
    )
    .bind(identity.subject.trim())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| OAuthError::Internal)?;

    let user_id = if let Some((user_id,)) = linked {
        // Existing Google identity: sign in (account lock serializes against
        // password changes — Phase 23E discipline).
        let locked: Option<(String,)> = sqlx::query_as(
            "SELECT id::text FROM users \
             WHERE id = CAST($1 AS uuid) AND status = 'active' FOR UPDATE",
        )
        .bind(&user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| OAuthError::Internal)?;
        let Some((user_id,)) = locked else {
            return Err(OAuthError::InvalidCredentials);
        };
        user_id
    } else {
        // New subject: find-or-create by normalized email under the row lock.
        let existing: Option<(String, bool, String)> = sqlx::query_as(
            "SELECT id::text, (email_verified_at IS NOT NULL) AS verified, status \
             FROM users WHERE email = $1 FOR UPDATE",
        )
        .bind(&email)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| OAuthError::Internal)?;

        match existing {
            // Verified + active account with the same normalized email:
            // auto-link (plan §2.5 verified-match rule).
            Some((user_id, true, status)) if status == "active" => user_id,
            Some((_, true, _)) => return Err(OAuthError::InvalidCredentials),
            // Unverified match: proof of existing-account control required —
            // never auto-link (plan §2.5); the plan's guided message.
            Some((_, false, _)) => return Err(OAuthError::EmailConflict),
            None => {
                // Brand-new account: Google-verified email is proof enough.
                let display_name = display_name_for(&identity);
                let created: Option<(String,)> = sqlx::query_as(
                    "INSERT INTO users (display_name, email, password_hash, email_verified_at) \
                     VALUES ($1, $2, NULL, now()) \
                     ON CONFLICT (email) DO NOTHING \
                     RETURNING id::text",
                )
                .bind(&display_name)
                .bind(&email)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|_| OAuthError::Internal)?;
                match created {
                    Some((user_id,)) => user_id,
                    // Lost a creation race: treat as the existing-account path.
                    None => {
                        let raced: Option<(String, bool, String)> = sqlx::query_as(
                            "SELECT id::text, (email_verified_at IS NOT NULL) AS verified, status \
                             FROM users WHERE email = $1 FOR UPDATE",
                        )
                        .bind(&email)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|_| OAuthError::Internal)?;
                        match raced {
                            Some((user_id, true, status)) if status == "active" => user_id,
                            Some((_, true, _)) => return Err(OAuthError::InvalidCredentials),
                            Some((_, false, _)) => return Err(OAuthError::EmailConflict),
                            None => return Err(OAuthError::Internal),
                        }
                    }
                }
            }
        }
    };

    // Link the immutable subject (idempotent under concurrency).
    sqlx::query(
        "INSERT INTO oauth_accounts (user_id, provider, provider_subject, provider_email) \
         VALUES (CAST($1 AS uuid), 'google', $2, $3) \
         ON CONFLICT (provider, provider_subject) DO NOTHING",
    )
    .bind(&user_id)
    .bind(identity.subject.trim())
    .bind(&email)
    .execute(&mut *tx)
    .await
    .map_err(|_| OAuthError::Internal)?;

    let session = session::insert_session(&mut *tx, &user_id, session::DEFAULT_SESSION_TTL_SECS, None, None)
        .await
        .map_err(|_| OAuthError::Internal)?;
    tx.commit().await.map_err(|_| OAuthError::Internal)?;

    // Profile DTO only — no tokens, no provider detail.
    let (name, email_out): (String, String) = sqlx::query_as(
        "SELECT display_name, email FROM users WHERE id = CAST($1 AS uuid)",
    )
    .bind(&user_id)
    .fetch_one(pool)
    .await
    .map_err(|_| OAuthError::Internal)?;

    Ok(LoginOutcome {
        user_id,
        name,
        email: email_out,
        session,
    })
}

// Re-export for handlers/tests.
pub use registration::normalize_email;

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> GoogleExchangeRequest {
        GoogleExchangeRequest {
            code: "4/0AeanS0b-secret-code".to_string(),
            code_verifier: "a".repeat(43),
            state: "b".repeat(32),
            redirect_uri: "http://127.0.0.1:49152/callback".to_string(),
            nonce: None,
        }
    }

    #[test]
    fn code_verifier_follows_rfc7636() {
        assert!(is_valid_code_verifier(&"a".repeat(43)));
        assert!(is_valid_code_verifier(&"aZ09-._~".repeat(16))); // 128
        assert!(!is_valid_code_verifier(&"a".repeat(42)));
        assert!(!is_valid_code_verifier(&"a".repeat(129)));
        assert!(!is_valid_code_verifier(&"a".repeat(42) + "!"));
    }

    #[test]
    fn state_shape_requires_the_desktop_generator_format() {
        assert!(is_valid_state_shape(&"0123456789abcdef".repeat(2))); // 32
        assert!(!is_valid_state_shape("short"));
        assert!(!is_valid_state_shape(&"a".repeat(31)));
        assert!(!is_valid_state_shape(&"a".repeat(32) + "+"));
    }

    #[test]
    fn loopback_redirect_requires_exact_127_0_0_1_callback() {
        assert!(is_loopback_redirect_uri("http://127.0.0.1:1/callback"));
        assert!(is_loopback_redirect_uri("http://127.0.0.1:49152/callback"));
        assert!(!is_loopback_redirect_uri("http://localhost:1/callback"));
        assert!(!is_loopback_redirect_uri("https://127.0.0.1:1/callback"));
        assert!(!is_loopback_redirect_uri("http://127.0.0.1:1/callback/"));
        assert!(!is_loopback_redirect_uri("http://127.0.0.1:1/cb"));
        assert!(!is_loopback_redirect_uri("http://10.0.0.1:1/callback"));
    }

    #[test]
    fn redirect_uri_mismatch_is_a_hard_generic_failure() {
        let mut r = request();
        r.redirect_uri = "http://127.0.0.1:49152/callback".to_string();
        assert!(validate_exchange_request(&r, "http://127.0.0.1:49152/callback").is_ok());
        let error =
            validate_exchange_request(&r, "http://127.0.0.1:5555/callback").unwrap_err();
        assert_eq!(error.code(), "invalid_or_expired_token");
    }

    #[test]
    fn malformed_requests_validate_before_any_state_or_provider_work() {
        let mut r = request();
        r.code = " ".to_string();
        assert_eq!(validate_exchange_request(&r, &r.redirect_uri).unwrap_err().code(), "validation_failed");
        let mut r = request();
        r.code_verifier = "x".repeat(42);
        assert_eq!(validate_exchange_request(&r, &r.redirect_uri).unwrap_err().code(), "validation_failed");
        let mut r = request();
        r.state = "zzz".to_string();
        assert_eq!(validate_exchange_request(&r, &r.redirect_uri).unwrap_err().code(), "validation_failed");
    }

    #[test]
    fn error_codes_and_messages_leak_nothing() {
        assert_eq!(OAuthError::EmailConflict.status_code(), 409);
        assert_eq!(OAuthError::EmailConflict.code(), "email_conflict");
        assert_eq!(OAuthError::EmailUnverified.status_code(), 403);
        assert_eq!(OAuthError::InvalidOrExpiredToken.status_code(), 400);
        for error in [
            OAuthError::InvalidOrExpiredToken,
            OAuthError::EmailUnverified,
            OAuthError::EmailConflict,
            OAuthError::DependencyUnavailable,
        ] {
            let message = error.message();
            assert!(!message.contains('@'));
            assert!(!message.contains("token="));
            assert!(!message.to_lowercase().contains("secret"));
        }
    }

    // ---- OIDC ID-token signature verification (Phase 23G step 6) ----------
    //
    // TEST-ONLY RSA keypair: generated in-sandbox purely to SIGN synthetic ID
    // tokens so the real RS256 verification path is exercised end to end. It
    // is not a credential and has no access to any system (synthetic fixture,
    // same standing as the `example.test` accounts).

    const TEST_JWK_N: &str = "oD051OXkPqHe3zavB9GOFMaxfs1RQ84dZBWCCTlh7OMbJU5RBg-qkkxd9iyzo2mm32t03UDJH9J-p4Sih2p0aR7m7OVT6R_PS3PPU5r48I9WEDatlzv5vzRyzqQDIz3I5Eavokl3aWrPI0NYDyJvwHmEdKXKrr4ZF0Bns9Cw9WiwrbrkmeO6wGvUw1Lo6MDYNScFUSnKdQgLL4-NeY6W2Z_1T8-D5tdu5xp28G4z8_Vn7zOhmSz_GIlsXLdWdFPY1U_UkvChB9nmBOdKf-QFAcIxgadw69oJtDH1tTvlAmtzSZHaimTIRJ39SkjZLiDPBLHlzuhQOulG5Gm5C-r95Q";
    const TEST_JWK_E: &str = "AQAB";
    const TEST_RSA_PRIVATE_PEM: &str = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEAoD051OXkPqHe3zavB9GOFMaxfs1RQ84dZBWCCTlh7OMbJU5R\nBg+qkkxd9iyzo2mm32t03UDJH9J+p4Sih2p0aR7m7OVT6R/PS3PPU5r48I9WEDat\nlzv5vzRyzqQDIz3I5Eavokl3aWrPI0NYDyJvwHmEdKXKrr4ZF0Bns9Cw9Wiwrbrk\nmeO6wGvUw1Lo6MDYNScFUSnKdQgLL4+NeY6W2Z/1T8+D5tdu5xp28G4z8/Vn7zOh\nmSz/GIlsXLdWdFPY1U/UkvChB9nmBOdKf+QFAcIxgadw69oJtDH1tTvlAmtzSZHa\nimTIRJ39SkjZLiDPBLHlzuhQOulG5Gm5C+r95QIDAQABAoIBAAyzvcb08FDP53xq\nd8zjw8kKCFDxn2/QdQwvF+J5gRwHoFCNP3P+piCE9QY9YrVHlaa5STrIVqK/2DIy\ntYJZiHlt+mB5PChLN4hjv+G0DU7FQs1abQyDgLEzZaegYO7lIdMiqh9WhosKbSzk\n7j3I44xU6N1cTrXnNAYsh1JXbs4NPbR1P70Lp3PntbVFeyv+1P9HhDwer5XHL4fU\nHmamaLUULqgimPPdCkBeAkFTjjEJO5ZHlGveOdNQP7AlUCVzVNs3EIEWuGFVG+Y2\noXPnd8Cgw9y/flZBa7byAL3gW7+3Xmsjhgzaobo+42haTkpeSyN2bej2AhvMgC9o\nWB36ANMCgYEA3ulrW/8HenGvwtFiP30usq8KsmI7WQAZ8Dnaj1xN0jL3fCdtF8oH\nB8AH7cs1/gcfI/emdC/rGb9HX8L21By+ALAz+czJPhNUvD+b7AJFPk22PKPiU8hV\n+f9Jon+uVbXsYrEKP45laCG5rF9zjQj9GCcNxWJpVQ7A6R1ZTeMKTw8CgYEAuAZD\n+7OzjF5yMWydwDKAtS5fNQt/E5Na9At9ciddO5eM20hXzb4S90Iuh2yDaCJUw4zM\nibhEIiZdo7MV2QmNEVVy30uJQzA/5cbidGXLLpg6loeEcDAoVVPBs8OTJYRFuB+H\nB4dCwM82pVO/pdh8E+7Q2NliCYbOvJ77CTcc48sCgYAsFZuu4Pv4oj0xWTmF9tri\naa3C45x7GIMIuFhaG9YkKQI64o03tDO5u6eVxqeR6pSlB1g7CDMFr4eND8DC6EBH\nYJGW6IIwLaCHbkkG3tgU30eYLI0ZJriiLXPBmGDBAK6UQhEVyrKRaoIO7f2rOcjc\nGvUlQTY1pSpPsAc2WR1zzwKBgQC2PRYz/CjR2HITufGa9Sh6xHpqaIEm3KWUypAn\nbQ4qRpGqUsEJMbNMg5Dacb7McDsrEJM5y25Ft2LCC0aWkZvOz6Ubto0vKWgN3+BA\nztHgycYzbe7cvwQiQ/iuUf61h4EXrrkk3nFfyZTfMsm2rmVpw/W02cn5jzPKnVIT\nNS358wKBgDfi1CTkpIDBb9PzxUDmE8pmYs7+03GnUIVmkjvNurE/2DCyOCSaJpcG\nvgPPdggvM8sudgGz5E6ESgx1BlWwCEkWJMscZU4USN9zrR9paRZXYkCX5hDypSyY\nReeMm5yNT5NBKES9Njx2kW0jZtfrTV0HWZBX1wEwzKb2CbUSRMig\n-----END RSA PRIVATE KEY-----";

    fn test_jwks(kid: &str) -> JwksMaterial {
        JwksMaterial {
            keys: vec![JwkKey {
                kid: kid.to_string(),
                n: TEST_JWK_N.to_string(),
                e: TEST_JWK_E.to_string(),
            }],
        }
    }

    fn sign_test_token(kid: Option<&str>, client_id: &str, mutate: impl FnOnce(&mut serde_json::Value)) -> String {
        use jsonwebtoken::{encode, EncodingKey, Header};
        let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = kid.map(str::to_string);
        let exp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 600;
        let mut claims = serde_json::json!({
            "sub": "google-sub-1",
            "email": "ada@example.test",
            "email_verified": true,
            "nonce": "n-abc",
            "name": "Ada",
            "iss": "https://accounts.google.com",
            "aud": client_id,
            "exp": exp,
        });
        mutate(&mut claims);
        encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_PEM.as_bytes()).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn valid_signed_id_token_verifies_and_yields_identity() {
        let token = sign_test_token(Some("test-kid"), "client-123", |_| {});
        let identity =
            verify_id_token_with(&token, "client-123", &test_jwks("test-kid")).unwrap();
        assert_eq!(identity.subject, "google-sub-1");
        assert_eq!(identity.email, "ada@example.test");
        assert!(identity.email_verified);
        assert_eq!(identity.nonce.as_deref(), Some("n-abc"));
    }

    #[test]
    fn id_token_signature_and_claim_checks_reject_forgeries() {
        let good = sign_test_token(Some("test-kid"), "client-123", |_| {});

        // Tampered payload (signature no longer matches).
        let mut segments: Vec<String> = good.split('.').map(str::to_string).collect();
        let flipped = if segments[1].ends_with('A') { 'B' } else { 'A' };
        segments[1].pop();
        segments[1].push(flipped);
        let tampered = segments.join(".");
        assert!(verify_id_token_with(&tampered, "client-123", &test_jwks("test-kid")).is_err());

        // Wrong audience (token not for this client).
        let wrong_aud = sign_test_token(Some("test-kid"), "someone-else", |_| {});
        assert!(verify_id_token_with(&wrong_aud, "client-123", &test_jwks("test-kid")).is_err());

        // Wrong issuer.
        let wrong_iss = sign_test_token(Some("test-kid"), "client-123", |c| {
            c["iss"] = serde_json::json!("https://evil.example.test");
        });
        assert!(verify_id_token_with(&wrong_iss, "client-123", &test_jwks("test-kid")).is_err());

        // Expired.
        let expired = sign_test_token(Some("test-kid"), "client-123", |c| {
            c["exp"] = serde_json::json!(1);
        });
        assert!(verify_id_token_with(&expired, "client-123", &test_jwks("test-kid")).is_err());

        // Unknown kid (not in the JWKS material).
        assert!(verify_id_token_with(&good, "client-123", &test_jwks("other-kid")).is_err());

        // Missing kid in header.
        let no_kid = sign_test_token(None, "client-123", |_| {});
        assert!(verify_id_token_with(&no_kid, "client-123", &test_jwks("test-kid")).is_err());

        // Alg confusion: an "alg":"none" token is refused outright.
        let unsigned = "eyJhbGciOiJub25lIn0.eyJzdWIiOiJoYWNrZXIifQ.";
        assert!(verify_id_token_with(unsigned, "client-123", &test_jwks("test-kid")).is_err());

        // Malformed inputs never panic.
        for junk in ["", "a", "a.b", "a.b.c", "!!!.???.$$$"] {
            assert!(verify_id_token_with(junk, "client-123", &test_jwks("test-kid")).is_err());
        }
    }

    #[test]
    fn display_name_falls_back_to_the_email_local_part() {
        let identity = GoogleIdentity {
            subject: "s".to_string(),
            email: "ada@example.test".to_string(),
            email_verified: true,
            nonce: None,
            name: Some("  ".to_string()),
        };
        assert_eq!(display_name_for(&identity), "ada");
        let named = GoogleIdentity { name: Some("Ada Lovelace".to_string()), ..identity };
        assert_eq!(display_name_for(&named), "Ada Lovelace");
    }
}
