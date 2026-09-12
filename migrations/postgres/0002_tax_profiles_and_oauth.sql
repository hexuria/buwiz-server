-- Cloud tax-profile event log + exclusive-ownership read model, plus
-- first-party desktop OAuth / device-code tables.

CREATE TABLE IF NOT EXISTS buwiz_server.tax_profile_events (
    stream_id TEXT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (stream_id, revision)
);

CREATE TABLE IF NOT EXISTS buwiz_server.tax_profiles (
    profile_id UUID PRIMARY KEY,
    stream_id TEXT NOT NULL UNIQUE,
    tin_root CHAR(9) NOT NULL,
    branch_code CHAR(5) NOT NULL,
    registered_name TEXT NOT NULL,
    rdo_code TEXT NOT NULL,
    line_of_business TEXT NOT NULL DEFAULT '',
    registered_address TEXT NOT NULL DEFAULT '',
    zip_code TEXT NOT NULL DEFAULT '',
    phone TEXT NOT NULL DEFAULT '',
    email TEXT NOT NULL DEFAULT '',
    taxpayer_type TEXT NOT NULL,
    tax_classification TEXT,
    is_vat_registered BOOLEAN NOT NULL DEFAULT FALSE,
    holder_kind TEXT NOT NULL,
    holder_user_id UUID REFERENCES auth_users(user_id) ON DELETE RESTRICT,
    holder_organization_id UUID REFERENCES auth_organizations(organization_id) ON DELETE RESTRICT,
    ownership_status TEXT NOT NULL,
    verification_status TEXT NOT NULL,
    verified_owner_user_id UUID REFERENCES auth_users(user_id) ON DELETE SET NULL,
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT tax_profiles_tin_branch_unique UNIQUE (tin_root, branch_code),
    CONSTRAINT tax_profiles_holder_kind_chk CHECK (
        (holder_kind = 'personal' AND holder_user_id IS NOT NULL AND holder_organization_id IS NULL)
        OR (holder_kind = 'organization' AND holder_organization_id IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS tax_profiles_holder_user_idx
    ON buwiz_server.tax_profiles (holder_user_id)
    WHERE holder_user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS tax_profiles_holder_org_idx
    ON buwiz_server.tax_profiles (holder_organization_id)
    WHERE holder_organization_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS buwiz_server.oauth_auth_codes (
    code_hash TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES auth_users(user_id) ON DELETE CASCADE,
    session_id TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    code_challenge TEXT NOT NULL,
    code_challenge_method TEXT NOT NULL DEFAULT 'S256',
    scope TEXT NOT NULL DEFAULT 'openid profile email tax_profiles',
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS buwiz_server.oauth_device_codes (
    device_code_hash TEXT PRIMARY KEY,
    user_code TEXT NOT NULL UNIQUE,
    client_id TEXT NOT NULL,
    user_id UUID REFERENCES auth_users(user_id) ON DELETE CASCADE,
    session_id TEXT,
    verification_uri TEXT NOT NULL,
    interval_seconds INTEGER NOT NULL DEFAULT 5,
    expires_at TIMESTAMPTZ NOT NULL,
    authorized_at TIMESTAMPTZ,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS oauth_device_codes_user_code_idx
    ON buwiz_server.oauth_device_codes (user_code);

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0002_tax_profiles_and_oauth')
ON CONFLICT (version) DO NOTHING;
