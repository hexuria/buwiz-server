-- Access passes: a firm (later, an AI agent) may work on someone else's tax
-- profile with the permissions the owner chose. Read model only; the events
-- (`tax_profile.pass_*`) in tax_profile_events are the source of truth.
--
-- The server never files with BIR. `filings.publish` in `permissions` is a
-- right the grantee's installed buwiz-forms app checks before it sends.
--
-- wasi-auth-migrate re-runs every SQL file on apply: keep this idempotent.

CREATE TABLE IF NOT EXISTS buwiz_server.tax_profile_grants (
    grant_id UUID PRIMARY KEY,
    profile_id UUID NOT NULL REFERENCES buwiz_server.tax_profiles (profile_id) ON DELETE CASCADE,
    grantee_kind TEXT NOT NULL,
    grantee_id TEXT NOT NULL,
    permissions JSONB NOT NULL DEFAULT '[]'::jsonb,
    status TEXT NOT NULL,
    expires_at TIMESTAMPTZ,
    created_by UUID REFERENCES auth_users (user_id) ON DELETE SET NULL,
    ended_by UUID REFERENCES auth_users (user_id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT tax_profile_grants_kind_chk CHECK (grantee_kind IN ('organization', 'agent')),
    CONSTRAINT tax_profile_grants_status_chk CHECK (
        status IN ('requested', 'offered', 'active', 'declined', 'revoked')
    ),
    CONSTRAINT tax_profile_grants_permissions_chk CHECK (jsonb_typeof(permissions) = 'array')
);

-- At most one open pass per grantee on a profile.
CREATE UNIQUE INDEX IF NOT EXISTS tax_profile_grants_open_uidx
    ON buwiz_server.tax_profile_grants (profile_id, grantee_kind, grantee_id)
    WHERE status IN ('requested', 'offered', 'active');

CREATE INDEX IF NOT EXISTS tax_profile_grants_grantee_idx
    ON buwiz_server.tax_profile_grants (grantee_kind, grantee_id)
    WHERE status IN ('requested', 'offered', 'active');

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0006_tax_profile_passes')
ON CONFLICT (version) DO NOTHING;
