-- AI agents (Claude Code, Codex, Grok and the like) reaching a taxpayer's
-- profiles through MCP.
--
-- agent_clients: apps that registered themselves (OAuth Dynamic Client
--   Registration). Registering grants nothing.
-- agent_connections: a taxpayer allowed one app on some of their own
--   profiles, with chosen permissions. Ending it (revoked_at) stops every
--   token at once and leaves the taxpayer's own sign-in alone.
-- agent_tokens / agent_auth_codes: only SHA-256 hashes are stored.
--
-- wasi-auth-migrate re-runs every SQL file on apply: keep this idempotent.

CREATE TABLE IF NOT EXISTS buwiz_server.agent_clients (
    client_id TEXT PRIMARY KEY,
    client_name TEXT NOT NULL,
    redirect_uris JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT agent_clients_redirects_chk CHECK (jsonb_typeof(redirect_uris) = 'array')
);

CREATE TABLE IF NOT EXISTS buwiz_server.agent_connections (
    connection_id UUID PRIMARY KEY,
    client_id TEXT NOT NULL REFERENCES buwiz_server.agent_clients (client_id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES auth_users (user_id) ON DELETE CASCADE,
    -- auth_users.security_revision when allowed: a password change or other
    -- security event on the account ends the connection's tokens.
    security_revision BIGINT NOT NULL,
    permissions JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    CONSTRAINT agent_connections_permissions_chk CHECK (jsonb_typeof(permissions) = 'array')
);

CREATE INDEX IF NOT EXISTS agent_connections_user_idx
    ON buwiz_server.agent_connections (user_id)
    WHERE revoked_at IS NULL;

CREATE TABLE IF NOT EXISTS buwiz_server.agent_connection_profiles (
    connection_id UUID NOT NULL REFERENCES buwiz_server.agent_connections (connection_id) ON DELETE CASCADE,
    profile_id UUID NOT NULL REFERENCES buwiz_server.tax_profiles (profile_id) ON DELETE CASCADE,
    PRIMARY KEY (connection_id, profile_id)
);

CREATE TABLE IF NOT EXISTS buwiz_server.agent_auth_codes (
    code_hash TEXT PRIMARY KEY,
    connection_id UUID NOT NULL REFERENCES buwiz_server.agent_connections (connection_id) ON DELETE CASCADE,
    redirect_uri TEXT NOT NULL,
    code_challenge TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS buwiz_server.agent_tokens (
    token_hash TEXT PRIMARY KEY,
    connection_id UUID NOT NULL REFERENCES buwiz_server.agent_connections (connection_id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    revoked_at TIMESTAMPTZ,
    CONSTRAINT agent_tokens_kind_chk CHECK (kind IN ('access', 'refresh'))
);

CREATE INDEX IF NOT EXISTS agent_tokens_connection_idx
    ON buwiz_server.agent_tokens (connection_id);

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0007_agent_connections')
ON CONFLICT (version) DO NOTHING;
