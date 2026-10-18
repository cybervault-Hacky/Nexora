-- Phase 23C — email verification tokens.
--
-- Plan entity outline (docs/phase-23/README.md §5): tokens exist ONLY as
-- sha256 digests of a 32-byte random value; the raw token never touches the
-- database, logs, or error messages — it appears only in the verification
-- email body. Single-use: `used_at` marks consumption. `revoked_at` marks
-- invalidation (a resend revokes the account's earlier live tokens).
--
-- Applied with sqlx (`sqlx migrate run` or `sqlx::migrate!()`), in order after
-- 20261018000002_create_oauth_accounts. Forward-recovery reversal (sqlx has no
-- down step): if this table must ever be removed, ship a NEW migration with:
--   DROP TABLE IF EXISTS email_verification_tokens;
-- Never edit this applied migration in place. Never run against production
-- from a dev machine.

CREATE TABLE email_verification_tokens (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- sha256(raw token) — 32 bytes. UNIQUE gives the consume-by-hash lookup.
    token_hash bytea NOT NULL UNIQUE,
    expires_at timestamptz NOT NULL,
    used_at timestamptz NULL,
    revoked_at timestamptz NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    -- A token is consumed or revoked, never both.
    CONSTRAINT email_verification_tokens_lifecycle_ck
        CHECK ((used_at IS NULL) OR (revoked_at IS NULL)),
    CONSTRAINT email_verification_tokens_expiry_ck
        CHECK (expires_at > created_at)
);

-- Resend revokes prior live tokens by user; cascade deletes with the user.
CREATE INDEX email_verification_tokens_user_id_idx
    ON email_verification_tokens (user_id);
