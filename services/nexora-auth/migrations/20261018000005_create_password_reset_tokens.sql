-- Phase 23E — password reset tokens (plan §5 `password_reset_tokens`).
-- Forward-only. Does not modify any previous migration.
--
-- The raw reset token exists only transiently in outbound email links and is
-- NEVER stored: `token_hash` is sha256(raw 32-byte token), exactly like
-- `email_verification_tokens` (Phase 23C). Single use (`used_at`), revocable
-- (`revoked_at` — outstanding tokens are invalidated when a newer recovery is
-- requested and on every password change), with absolute expiry (`expires_at`).
CREATE TABLE IF NOT EXISTS password_reset_tokens (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  bytea NOT NULL UNIQUE,
    created_at  timestamptz NOT NULL DEFAULT now(),
    expires_at  timestamptz NOT NULL,
    used_at     timestamptz,
    revoked_at  timestamptz,
    CONSTRAINT password_reset_tokens_expiry_after_creation
        CHECK (expires_at > created_at),
    CONSTRAINT password_reset_tokens_usage_xor_revocation
        CHECK ((used_at IS NULL) OR (revoked_at IS NULL))
);

CREATE INDEX IF NOT EXISTS password_reset_tokens_user_id_idx
    ON password_reset_tokens (user_id);
