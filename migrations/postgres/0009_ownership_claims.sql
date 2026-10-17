-- Ownership claims with Form 2303.
--
-- A taxpayer proves a TIN is theirs by uploading their Form 2303 (Certificate
-- of Registration). Buwiz staff review every claim. The file is stored
-- encrypted (AES-256-GCM, key derived from the server key, the claim id as
-- associated data) and deleted when the claim closes; the decision, reviewer,
-- note and the file's SHA-256 stay. Files marked as AI-made are refused and
-- never stored (`ai_refusals` counts the attempts for the reviewer).
--
-- Form 2303 is used only to prove ownership, never to choose forms.
--
-- wasi-auth-migrate re-runs every SQL file on apply: keep this idempotent.

CREATE TABLE IF NOT EXISTS buwiz_server.ownership_claims (
    claim_id UUID PRIMARY KEY,
    profile_id UUID NOT NULL REFERENCES buwiz_server.tax_profiles (profile_id) ON DELETE CASCADE,
    claimant_user_id UUID NOT NULL REFERENCES auth_users (user_id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    file_kind TEXT,
    file_size BIGINT,
    file_sha256 TEXT,
    file_nonce TEXT,
    file_ciphertext TEXT,
    has_c2pa BOOLEAN NOT NULL DEFAULT FALSE,
    ai_refusals BIGINT NOT NULL DEFAULT 0,
    reviewer_user_id UUID REFERENCES auth_users (user_id) ON DELETE SET NULL,
    decision_note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    decided_at TIMESTAMPTZ,
    CONSTRAINT ownership_claims_status_chk CHECK (
        status IN ('awaiting_file', 'pending', 'approved', 'rejected', 'withdrawn')
    ),
    CONSTRAINT ownership_claims_file_chk CHECK (
        file_kind IS NULL OR file_kind IN ('jpeg', 'png', 'pdf')
    ),
    -- A closed claim keeps no file.
    CONSTRAINT ownership_claims_closed_no_file_chk CHECK (
        status IN ('awaiting_file', 'pending') OR file_ciphertext IS NULL
    )
);

-- One open claim per person per profile.
CREATE UNIQUE INDEX IF NOT EXISTS ownership_claims_open_uidx
    ON buwiz_server.ownership_claims (profile_id, claimant_user_id)
    WHERE status IN ('awaiting_file', 'pending');

-- Upload parts, each encrypted on arrival (the runtime cannot stream a large
-- request body yet). Joined, screened and replaced by the whole encrypted
-- file after the last part; removed whenever an upload restarts or the claim
-- closes.
CREATE TABLE IF NOT EXISTS buwiz_server.ownership_claim_parts (
    claim_id UUID NOT NULL REFERENCES buwiz_server.ownership_claims (claim_id) ON DELETE CASCADE,
    part_index INTEGER NOT NULL,
    nonce TEXT NOT NULL,
    ciphertext TEXT NOT NULL,
    PRIMARY KEY (claim_id, part_index)
);

CREATE INDEX IF NOT EXISTS ownership_claims_queue_idx
    ON buwiz_server.ownership_claims (updated_at)
    WHERE status = 'pending';

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0009_ownership_claims')
ON CONFLICT (version) DO NOTHING;
