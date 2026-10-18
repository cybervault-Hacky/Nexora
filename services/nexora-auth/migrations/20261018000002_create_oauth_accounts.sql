-- 20261018000002_create_oauth_accounts.sql — Phase 23B (database foundation)
--
-- Future OAuth identities live HERE — never conflated with password
-- credentials in `users`. This is schema only: account-linking logic is a
-- later phase. Provider access/refresh tokens are NOT stored (and must never
-- be added to this table).
--
-- Order: must run after 20261018000001_create_users (foreign key).
-- Forward-recovery reversal (new migration):
--     DROP TABLE IF EXISTS oauth_accounts;

CREATE TABLE oauth_accounts (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id           uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider          text NOT NULL,
    -- Immutable provider-side subject (the 'sub' claim for Google).
    provider_subject  text NOT NULL,
    -- Provider-reported email (may differ from users.email); informational.
    provider_email    text NULL,
    created_at        timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT oauth_accounts_provider_ck
        CHECK (btrim(provider) <> ''),
    CONSTRAINT oauth_accounts_subject_ck
        CHECK (btrim(provider_subject) <> ''),
    -- One provider identity maps to exactly one account row.
    CONSTRAINT oauth_accounts_provider_subject_key
        UNIQUE (provider, provider_subject)
);

-- No extra indexes beyond the UNIQUE constraint: planned lookups are by
-- (provider, provider_subject) or user_id (served by the FK's index needs
-- when query patterns arrive — add an index on user_id with that migration
-- if measurements justify it).
