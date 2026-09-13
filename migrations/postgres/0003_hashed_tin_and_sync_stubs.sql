-- Hashed TIN identity (UUID remains PK).
-- Effective-dated COR version ledger is not modeled.
-- Per-year forms / drafts / filings live in 0004 (canonical V1).
--
-- wasi-auth-migrate re-runs every SQL file on apply. Do not CREATE the old
-- stub tables here: 0004 replaces them, and recreating the draft_id schema
-- would fail after 0004's form_drafts(id) rewrite.

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

-- Exclusive uniqueness on hashed identity, not raw TIN. 0004 replaces this
-- with UNIQUE (tin_hash) BYTEA after backfill.
CREATE UNIQUE INDEX IF NOT EXISTS tax_profiles_tin_identity_hash_uidx
    ON buwiz_server.tax_profiles (tin_identity_hash)
    WHERE tin_identity_hash IS NOT NULL;

ALTER TABLE buwiz_server.tax_profiles
    ALTER COLUMN tin_root DROP NOT NULL,
    ALTER COLUMN branch_code DROP NOT NULL;

ALTER TABLE buwiz_server.tax_profiles
    DROP CONSTRAINT IF EXISTS tax_profiles_tin_branch_unique;

DROP INDEX IF EXISTS buwiz_server.tax_profiles_tin_uidx;

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0003_hashed_tin_and_sync_stubs')
ON CONFLICT (version) DO NOTHING;
