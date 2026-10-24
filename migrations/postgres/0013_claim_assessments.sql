-- AI review aid for ownership claims.
--
-- After a Form 2303 upload, a reader model and Jev look at the file and fixed
-- rules turn what they saw into a recommendation for staff. Staff still
-- decide every claim. `ai_assessment` (findings, which quote the name and
-- address on the document) is cleared when the claim closes, like the file;
-- `ai_recommendation` stays so decisions can be compared with it.
--
-- wasi-auth-migrate re-runs every SQL file on apply: keep this idempotent.

ALTER TABLE buwiz_server.ownership_claims
    ADD COLUMN IF NOT EXISTS ai_assessment JSONB,
    ADD COLUMN IF NOT EXISTS ai_recommendation TEXT,
    ADD COLUMN IF NOT EXISTS ai_assessed_at TIMESTAMPTZ;

ALTER TABLE buwiz_server.ownership_claims
    DROP CONSTRAINT IF EXISTS ownership_claims_ai_recommendation_chk;
ALTER TABLE buwiz_server.ownership_claims
    ADD CONSTRAINT ownership_claims_ai_recommendation_chk CHECK (
        ai_recommendation IS NULL
        OR ai_recommendation IN ('claimable', 'needs_review', 'not_claimable')
    );

-- A closed claim keeps no findings.
ALTER TABLE buwiz_server.ownership_claims
    DROP CONSTRAINT IF EXISTS ownership_claims_closed_no_ai_assessment_chk;
ALTER TABLE buwiz_server.ownership_claims
    ADD CONSTRAINT ownership_claims_closed_no_ai_assessment_chk CHECK (
        status IN ('awaiting_file', 'pending') OR ai_assessment IS NULL
    );

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0013_claim_assessments')
ON CONFLICT (version) DO NOTHING;
