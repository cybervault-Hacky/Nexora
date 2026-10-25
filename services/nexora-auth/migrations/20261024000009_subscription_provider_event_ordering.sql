-- Forward-only G-01 hardening: retain the newest provider event creation time
-- accepted for each subscription so stale signed webhook events cannot
-- overwrite newer lifecycle state. Existing subscription rows remain NULL and
-- therefore fail closed for order-sensitive updates until trusted activation;
-- no billing records are rewritten or deleted.

ALTER TABLE subscriptions
    ADD COLUMN IF NOT EXISTS last_provider_event_created_at bigint;

ALTER TABLE webhook_events
    ADD COLUMN IF NOT EXISTS provider_event_created_at bigint,
    ADD COLUMN IF NOT EXISTS provider_payment_intent_id text,
    ADD COLUMN IF NOT EXISTS provider_subscription_id text,
    ADD COLUMN IF NOT EXISTS provider_subscription_status text,
    ADD COLUMN IF NOT EXISTS is_full_refund boolean NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS idx_webhook_events_full_refund_payment_intent
    ON webhook_events (provider_payment_intent_id)
    WHERE is_full_refund = true;

CREATE INDEX IF NOT EXISTS idx_webhook_events_subscription_event_order
    ON webhook_events (provider_subscription_id, provider_event_created_at);

COMMENT ON COLUMN subscriptions.last_provider_event_created_at IS
    'Unix seconds for the newest accepted Stripe lifecycle event; NULL on pre-migration rows, whose order-sensitive updates fail closed.';

COMMENT ON COLUMN webhook_events.provider_event_created_at IS
    'Signed provider event creation time used to reject out-of-order lifecycle transitions.';
