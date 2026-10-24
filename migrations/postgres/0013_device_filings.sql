-- Device filing queue. An agent connection with may_queue can leave a filing
-- here. The account owner's buwiz-forms app picks it up and sends it to BIR
-- from the user's computer. This server never opens an SFTP session.
--
-- wasi-auth-migrate re-runs every SQL file on apply: keep this idempotent.

ALTER TABLE buwiz_server.agent_connections
    ADD COLUMN IF NOT EXISTS may_queue boolean NOT NULL DEFAULT false;

CREATE TABLE IF NOT EXISTS buwiz_server.device_filings (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES auth_users (user_id) ON DELETE CASCADE,
    agent_connection_id uuid NOT NULL REFERENCES buwiz_server.agent_connections (connection_id) ON DELETE CASCADE,
    profile_id uuid NOT NULL REFERENCES buwiz_server.tax_profiles (profile_id) ON DELETE CASCADE,
    tin text NOT NULL,
    form_code text NOT NULL,
    tax_year integer NOT NULL,
    period integer NOT NULL,
    fields jsonb NOT NULL,
    status text NOT NULL DEFAULT 'queued',
    detail text,
    report jsonb,
    created_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT device_filings_status_check CHECK (status IN ('queued', 'picked', 'invalid', 'failed')),
    CONSTRAINT device_filings_period_check CHECK (period BETWEEN 1 AND 12)
);

CREATE INDEX IF NOT EXISTS device_filings_user_status_idx
    ON buwiz_server.device_filings (user_id, status, created_at);

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0013_device_filings')
ON CONFLICT (version) DO NOTHING;
