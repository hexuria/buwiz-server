-- Owner invites and firm publishers.
--
-- tax_profile_owner_invites: read model of `tax_profile.owner_invite*`
--   events. A firm that created a draft invites the taxpayer by email; only
--   an account with that verified email, holding the link's secret, can
--   accept. Only the secret's SHA-256 is stored.
-- firm_publishers: the members a firm picked to publish to BIR. Publishing
--   needs this *and* the owner's `filings.publish` switch on the firm's pass.
--   The server never files with BIR; the installed buwiz-forms app checks.
--
-- wasi-auth-migrate re-runs every SQL file on apply: keep this idempotent.

CREATE TABLE IF NOT EXISTS buwiz_server.tax_profile_owner_invites (
    invite_id UUID PRIMARY KEY,
    profile_id UUID NOT NULL REFERENCES buwiz_server.tax_profiles (profile_id) ON DELETE CASCADE,
    organization_id UUID NOT NULL,
    email TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_by UUID REFERENCES auth_users (user_id) ON DELETE SET NULL,
    decided_by UUID REFERENCES auth_users (user_id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT tax_profile_owner_invites_status_chk CHECK (
        status IN ('open', 'accepted', 'withdrawn')
    )
);

-- At most one open invite per profile.
CREATE UNIQUE INDEX IF NOT EXISTS tax_profile_owner_invites_open_uidx
    ON buwiz_server.tax_profile_owner_invites (profile_id)
    WHERE status = 'open';

CREATE INDEX IF NOT EXISTS tax_profile_owner_invites_org_idx
    ON buwiz_server.tax_profile_owner_invites (organization_id)
    WHERE status = 'open';

CREATE TABLE IF NOT EXISTS buwiz_server.firm_publishers (
    organization_id UUID NOT NULL REFERENCES auth_organizations (organization_id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES auth_users (user_id) ON DELETE CASCADE,
    added_by UUID REFERENCES auth_users (user_id) ON DELETE SET NULL,
    added_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (organization_id, user_id)
);

CREATE INDEX IF NOT EXISTS firm_publishers_user_idx
    ON buwiz_server.firm_publishers (user_id);

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0008_owner_invites_and_publishers')
ON CONFLICT (version) DO NOTHING;
