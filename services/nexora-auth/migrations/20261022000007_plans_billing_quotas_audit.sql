-- Plans, versioned pricing, subscriptions, payment orders, signed webhooks,
-- backend-authoritative daily per-model quota accounting, and audit records.

CREATE INDEX IF NOT EXISTS oauth_accounts_user_id_idx
    ON oauth_accounts (user_id);

CREATE TABLE IF NOT EXISTS plans (
    id text PRIMARY KEY CHECK (length(trim(id)) > 0),
    display_name text NOT NULL CHECK (length(trim(display_name)) > 0),
    description text NOT NULL,
    tier_rank integer NOT NULL CHECK (tier_rank >= 0),
    active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS plan_versions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id text NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    version_number integer NOT NULL CHECK (version_number >= 1),
    billing_interval text NOT NULL CHECK (billing_interval IN ('none', 'month', 'year')),
    currency text NOT NULL CHECK (length(currency) = 3),
    amount_minor bigint NOT NULL CHECK (amount_minor >= 0),
    provider_price_id text,
    active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (plan_id, version_number, billing_interval)
);

CREATE INDEX IF NOT EXISTS idx_plan_versions_active
    ON plan_versions (plan_id, active);

CREATE TABLE IF NOT EXISTS plan_model_quotas (
    plan_id text NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    model_id text NOT NULL CHECK (length(trim(model_id)) > 0),
    daily_limit integer NOT NULL CHECK (daily_limit >= 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (plan_id, model_id)
);

CREATE TABLE IF NOT EXISTS subscriptions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    plan_id text NOT NULL REFERENCES plans(id),
    plan_version_id uuid REFERENCES plan_versions(id),
    provider text NOT NULL CHECK (length(trim(provider)) > 0),
    provider_customer_id text,
    provider_subscription_id text UNIQUE,
    status text NOT NULL CHECK (
        status IN (
            'incomplete',
            'trialing',
            'active',
            'past_due',
            'canceled',
            'expired',
            'revoked'
        )
    ),
    cancel_at_period_end boolean NOT NULL DEFAULT false,
    current_period_start timestamptz NOT NULL DEFAULT now(),
    current_period_end timestamptz NOT NULL,
    canceled_at timestamptz,
    ended_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (current_period_end > current_period_start)
);

CREATE INDEX IF NOT EXISTS idx_subscriptions_user_status
    ON subscriptions (user_id, status, current_period_end DESC);

CREATE TABLE IF NOT EXISTS payment_orders (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    plan_id text NOT NULL REFERENCES plans(id),
    plan_version_id uuid NOT NULL REFERENCES plan_versions(id),
    billing_interval text NOT NULL CHECK (billing_interval IN ('month', 'year')),
    currency text NOT NULL CHECK (length(currency) = 3),
    amount_minor bigint NOT NULL CHECK (amount_minor >= 0),
    provider text NOT NULL CHECK (length(trim(provider)) > 0),
    idempotency_key text NOT NULL CHECK (length(trim(idempotency_key)) BETWEEN 8 AND 128),
    provider_checkout_session_id text UNIQUE,
    provider_payment_intent_id text,
    provider_subscription_id text,
    status text NOT NULL CHECK (
        status IN ('pending', 'completed', 'expired', 'failed', 'refunded')
    ),
    checkout_url text,
    expires_at timestamptz NOT NULL,
    completed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_payment_orders_user_created
    ON payment_orders (user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS webhook_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    provider text NOT NULL CHECK (length(trim(provider)) > 0),
    provider_event_id text NOT NULL CHECK (length(trim(provider_event_id)) > 0),
    event_type text NOT NULL CHECK (length(trim(event_type)) > 0),
    payload_sha256 bytea NOT NULL CHECK (octet_length(payload_sha256) = 32),
    status text NOT NULL CHECK (
        status IN ('processing', 'processed', 'ignored', 'failed')
    ),
    error_code text,
    received_at timestamptz NOT NULL DEFAULT now(),
    processed_at timestamptz,
    UNIQUE (provider, provider_event_id)
);

CREATE TABLE IF NOT EXISTS daily_usage_counters (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    period_day date NOT NULL,
    model_id text NOT NULL CHECK (length(trim(model_id)) > 0),
    committed_count integer NOT NULL DEFAULT 0 CHECK (committed_count >= 0),
    reserved_count integer NOT NULL DEFAULT 0 CHECK (reserved_count >= 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, period_day, model_id)
);

CREATE TABLE IF NOT EXISTS quota_reservations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    request_id text NOT NULL CHECK (length(trim(request_id)) BETWEEN 6 AND 128),
    period_day date NOT NULL,
    model_id text NOT NULL CHECK (length(trim(model_id)) > 0),
    plan_id text NOT NULL REFERENCES plans(id),
    status text NOT NULL CHECK (
        status IN ('reserved', 'committed', 'released', 'expired')
    ),
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    settled_at timestamptz,
    UNIQUE (user_id, request_id)
);

CREATE INDEX IF NOT EXISTS idx_quota_reservations_open
    ON quota_reservations (user_id, period_day, status, expires_at);

CREATE TABLE IF NOT EXISTS audit_logs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_user_id uuid REFERENCES users(id) ON DELETE SET NULL,
    subject_id text NOT NULL,
    action text NOT NULL CHECK (length(trim(action)) > 0),
    outcome text NOT NULL CHECK (length(trim(outcome)) > 0),
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_created
    ON audit_logs (created_at DESC);

-- Proposed configurable default catalogue rows (synced from environment config on boot).
INSERT INTO plans (id, display_name, description, tier_rank, active) VALUES
    ('free', 'Free', 'On-device coding assistant with daily starter allowances for Nexora Mini and Sonnet.', 0, true),
    ('pro', 'Pro', 'Expanded daily allowances for sustained software engineering across Nexora Mini and Sonnet.', 1, true),
    ('team', 'Team', 'High-throughput daily allowances for intensive multi-repository engineering.', 2, true)
ON CONFLICT (id) DO NOTHING;

INSERT INTO plan_versions (plan_id, version_number, billing_interval, currency, amount_minor, active) VALUES
    ('free', 1, 'none', 'USD', 0, true),
    ('pro', 1, 'month', 'USD', 1900, true),
    ('pro', 1, 'year', 'USD', 19000, true),
    ('team', 1, 'month', 'USD', 3900, true),
    ('team', 1, 'year', 'USD', 39000, true)
ON CONFLICT (plan_id, version_number, billing_interval) DO NOTHING;

INSERT INTO plan_model_quotas (plan_id, model_id, daily_limit) VALUES
    ('free', 'nexora-mini', 25),
    ('free', 'nexora-sonnet', 5),
    ('pro', 'nexora-mini', 300),
    ('pro', 'nexora-sonnet', 100),
    ('team', 'nexora-mini', 1000),
    ('team', 'nexora-sonnet', 300)
ON CONFLICT (plan_id, model_id) DO NOTHING;
