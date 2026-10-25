//! nexora-auth — standalone authentication service.
//!
//! Phase 23A foundation (health-only scaffold) + Phase 23B (Argon2id password
//! hashing, users/oauth schema, DB configuration) + Phase 23C (registration,
//! email verification, resend) + Phase 23D (login, sessions, logout, refresh
//! with family reuse detection, DB-backed rate limiting). Password recovery,
//! Google OAuth, and desktop IPC wiring are later plan steps and are not
//! implemented. See `docs/phase-23a/README.md` … `docs/phase-23d/README.md`.

pub mod account;
pub mod app;
pub mod audit;
pub mod billing;
pub mod config;
pub mod email;
pub mod login;
pub mod oauth;
pub mod password;
pub mod quota;
pub mod rate_limit;
pub mod recovery;
pub mod registration;
pub mod scrub;
pub mod session;

/// Migrations are embedded in the service binary so startup is independent of
/// the process working directory and deployment-time source files.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Apply the package's embedded SQL migrations idempotently against `pool`.
pub async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}
