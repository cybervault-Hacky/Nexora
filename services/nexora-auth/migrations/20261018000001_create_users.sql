-- 20261018000001_create_users.sql — Phase 23B (database foundation)
--
-- Account core table: a schema proposal for future account flows. No API,
-- registration, or user-management logic exists yet (see docs/phase-23b).
--
-- Migration convention: SQLx (versioned files, applied in lexicographic
-- order, recorded in `_sqlx_migrations`). Apply with `sqlx migrate run`
-- or `sqlx::migrate!()` — never against a production database from a dev
-- machine. Reversal strategy is forward-recovery: SQLx has no down step, so
-- to undo this migration ship a NEW migration containing:
--     DROP TABLE IF EXISTS users;
--
-- Requires PostgreSQL 13+ (built-in gen_random_uuid()).
--
-- Storage rules enforced by design:
--   * password_hash holds an Argon2id PHC string only — NEVER plaintext.
--   * verification/reset tokens and OAuth access tokens are not stored here
--     (future tables must store digests only).

CREATE TABLE users (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name      text NOT NULL,
    email             text NOT NULL,
    -- Argon2id PHC string; NULL = future OAuth-only account (no password).
    password_hash     text NULL,
    email_verified_at timestamptz NULL,
    status            text NOT NULL DEFAULT 'active',
    created_at        timestamptz NOT NULL DEFAULT now(),
    updated_at        timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT users_display_name_ck
        CHECK (btrim(display_name) <> '' AND char_length(display_name) <= 128),
    -- Emails are stored normalized (trim + lowercase) by the future writer;
    -- the CHECK enforces that invariant at the database level so the UNIQUE
    -- guarantee applies to the normalized value.
    CONSTRAINT users_email_normalized_ck
        CHECK (email = lower(btrim(email)) AND char_length(email) <= 254),
    CONSTRAINT users_email_key UNIQUE (email),
    CONSTRAINT users_status_ck
        CHECK (status IN ('active', 'locked', 'deleted'))
);

-- Intentionally no extra indexes: every planned lookup is by primary key or
-- the unique email constraint (which creates its own index). Indexes for
-- sessions/tokens arrive with their tables in later migrations, justified by
-- those queries.
