-- Hashed TIN identity (UUID remains PK) + stub sync entities for Grok Bot / headless-bir.
-- Effective-dated COR version ledger is not modeled; per-year forms stay Manual-only.

ALTER TABLE buwiz_server.tax_profiles
    ADD COLUMN IF NOT EXISTS tin_identity_hash TEXT,
    ADD COLUMN IF NOT EXISTS tin_last4 TEXT,
    ADD COLUMN IF NOT EXISTS display_name TEXT,
    ADD COLUMN IF NOT EXISTS claim_status TEXT NOT NULL DEFAULT 'owned',
    ADD COLUMN IF NOT EXISTS org_id UUID REFERENCES auth_organizations (organization_id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS owner_user_id UUID REFERENCES auth_users (user_id) ON DELETE SET NULL;

UPDATE buwiz_server.tax_profiles
SET
    tin_last4 = COALESCE(tin_last4, RIGHT(tin_root, 4)),
    display_name = COALESCE(display_name, registered_name),
    owner_user_id = COALESCE(owner_user_id, holder_user_id),
    org_id = COALESCE(org_id, holder_organization_id),
    claim_status = CASE
        WHEN ownership_status = 'company_managed' THEN 'pending_claim'
        ELSE 'owned'
    END
WHERE tin_root IS NOT NULL;

-- Exclusive uniqueness on hashed identity, not raw TIN.
CREATE UNIQUE INDEX IF NOT EXISTS tax_profiles_tin_identity_hash_uidx
    ON buwiz_server.tax_profiles (tin_identity_hash)
    WHERE tin_identity_hash IS NOT NULL;

ALTER TABLE buwiz_server.tax_profiles
    ALTER COLUMN tin_root DROP NOT NULL,
    ALTER COLUMN branch_code DROP NOT NULL;

ALTER TABLE buwiz_server.tax_profiles
    DROP CONSTRAINT IF EXISTS tax_profiles_tin_branch_unique;

DROP INDEX IF EXISTS buwiz_server.tax_profiles_tin_uidx;

-- Per-year forms set (Manual source only in v1). Full sync is stubbed.
CREATE TABLE IF NOT EXISTS buwiz_server.per_year_forms_sets (
    forms_set_id UUID PRIMARY KEY,
    profile_id UUID NOT NULL REFERENCES buwiz_server.tax_profiles (profile_id) ON DELETE CASCADE,
    tax_year SMALLINT NOT NULL,
    entries_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (profile_id, tax_year)
);

-- Form drafts: form_code + period + status + payload JSON. Full draft sync is stubbed.
CREATE TABLE IF NOT EXISTS buwiz_server.form_drafts (
    draft_id UUID PRIMARY KEY,
    profile_id UUID NOT NULL REFERENCES buwiz_server.tax_profiles (profile_id) ON DELETE CASCADE,
    form_code TEXT NOT NULL,
    period TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS form_drafts_profile_idx
    ON buwiz_server.form_drafts (profile_id, form_code, period);

-- Filing / submission jobs: Draft → Queued → Submitted → Confirmed → Paid.
-- Append-only metadata. Never store BIR credentials, profile_pin_hash, totp_secret,
-- IMAP passwords, or mailbox OAuth tokens.
CREATE TABLE IF NOT EXISTS buwiz_server.filing_jobs (
    filing_id UUID PRIMARY KEY,
    profile_id UUID NOT NULL REFERENCES buwiz_server.tax_profiles (profile_id) ON DELETE CASCADE,
    draft_id UUID REFERENCES buwiz_server.form_drafts (draft_id) ON DELETE SET NULL,
    form_code TEXT NOT NULL,
    period TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'Draft',
    receipt_match_keys JSONB NOT NULL DEFAULT '{}'::jsonb,
    metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS buwiz_server.filing_job_events (
    event_id BIGSERIAL PRIMARY KEY,
    filing_id UUID NOT NULL REFERENCES buwiz_server.filing_jobs (filing_id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS filing_jobs_profile_idx
    ON buwiz_server.filing_jobs (profile_id, status);

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0003_hashed_tin_and_sync_stubs')
ON CONFLICT (version) DO NOTHING;
