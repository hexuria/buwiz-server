//! AI agent clients, connections and tokens (`0007_agent_connections`).
//!
//! Codes and tokens are stored as SHA-256 hashes only. Every token lookup
//! re-checks the connection, the taxpayer's account status and its security
//! revision, so ending a connection or changing a password stops the agent
//! on its next call.

use serde_json::{json, Value};

use crate::contracts::{AgentConnectionView, AgentProfileRef};
use crate::domain::PassPermissions;
use crate::error::{AuthStackError, AuthStackResult};

use super::{
    AtomicSqlStatement, execute_sql, execute_sql_atomic, initialize_schema_async, required_string,
    row_i64, row_string, row_string_list,
};

/// How agent codes and tokens are stored: SHA-256, base64url without
/// padding. The OAuth endpoints and the MCP check must hash the same way.
pub(crate) fn sha256_b64url(value: &str) -> String {
    use base64::Engine as _;
    use sha2::Digest as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(value.as_bytes()))
}

const TIMESTAMP: &str = "'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'";

#[derive(Clone, Debug)]
pub(crate) struct StoredAgentClient {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
}

/// A profile the taxpayer may connect an agent to (one they own).
#[derive(Clone, Debug)]
pub(crate) struct OwnedProfile {
    pub profile_id: String,
    pub profile_name: String,
    pub tin_last4: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ConsumedAgentCode {
    pub connection_id: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
}

/// The agent behind a valid access token.
#[derive(Clone, Debug)]
pub(crate) struct AgentActor {
    pub connection_id: String,
    pub user_id: String,
    #[allow(dead_code)] // kept for audit logs
    pub client_id: String,
    pub client_name: String,
    pub permissions: PassPermissions,
    pub profile_ids: Vec<String>,
}

/// Registrations in the last hour, and a sweep of apps that registered over
/// a week ago and were never allowed.
pub(crate) async fn agent_registrations_last_hour() -> AuthStackResult<i64> {
    initialize_schema_async().await?;
    execute_sql(
        "DELETE FROM buwiz_server.agent_clients c \
         WHERE c.created_at < CURRENT_TIMESTAMP - INTERVAL '7 days' \
           AND NOT EXISTS (SELECT 1 FROM buwiz_server.agent_connections x \
                           WHERE x.client_id = c.client_id)",
        vec![],
    )
    .await?;
    let rows = execute_sql(
        "SELECT count(*)::bigint AS n FROM buwiz_server.agent_clients \
         WHERE created_at > CURRENT_TIMESTAMP - INTERVAL '1 hour'",
        vec![],
    )
    .await?;
    Ok(rows.first().and_then(|row| row_i64(row, "n")).unwrap_or(0))
}

pub(crate) async fn insert_agent_client(
    client_id: &str,
    client_name: &str,
    redirect_uris: &[String],
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql(
        "INSERT INTO buwiz_server.agent_clients (client_id, client_name, redirect_uris) \
         VALUES (?1, ?2, ?3::jsonb)",
        vec![json!(client_id), json!(client_name), json!(redirect_uris)],
    )
    .await?;
    Ok(())
}

pub(crate) async fn load_agent_client(client_id: &str) -> AuthStackResult<Option<StoredAgentClient>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT client_id, client_name, redirect_uris \
         FROM buwiz_server.agent_clients WHERE client_id = ?1",
        vec![json!(client_id)],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(StoredAgentClient {
        client_id: required_string(row, "client_id")?,
        client_name: required_string(row, "client_name")?,
        redirect_uris: row_string_list(row, "redirect_uris"),
    }))
}

/// Active profiles `user_id` owns: held personally, or verified as theirs
/// while a firm holds them.
pub(crate) async fn list_owned_profiles(user_id: &str) -> AuthStackResult<Vec<OwnedProfile>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT profile_id::text AS profile_id, \
                COALESCE(display_name, registered_name) AS profile_name, \
                COALESCE(tin_last4, '') AS tin_last4 \
         FROM buwiz_server.tax_profiles \
         WHERE COALESCE(is_archived, false) = FALSE \
           AND (holder_user_id = ?1::uuid OR verified_owner_user_id = ?1::uuid) \
         ORDER BY profile_name",
        vec![json!(user_id)],
    )
    .await?;
    rows.iter()
        .map(|row| {
            Ok(OwnedProfile {
                profile_id: required_string(row, "profile_id")?,
                profile_name: row_string(row, "profile_name").unwrap_or_default(),
                tin_last4: row_string(row, "tin_last4").unwrap_or_default(),
            })
        })
        .collect()
}

/// Records the taxpayer's Allow: a new connection on the chosen profiles
/// they own, and the one-time code for the app. An earlier connection of the
/// same app for the same taxpayer ends, so re-allowing replaces it.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_agent_connection(
    connection_id: &str,
    user_id: &str,
    client_id: &str,
    permissions: PassPermissions,
    profile_ids: &[String],
    code_hash: &str,
    redirect_uri: &str,
    code_challenge: &str,
    code_ttl_seconds: i64,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql_atomic(vec![
        AtomicSqlStatement::execute(
            "UPDATE buwiz_server.agent_tokens SET revoked_at = CURRENT_TIMESTAMP \
             WHERE revoked_at IS NULL AND connection_id IN ( \
                 SELECT connection_id FROM buwiz_server.agent_connections \
                 WHERE user_id = ?1::uuid AND client_id = ?2 AND revoked_at IS NULL)",
            vec![json!(user_id), json!(client_id)],
        ),
        AtomicSqlStatement::execute(
            "UPDATE buwiz_server.agent_connections SET revoked_at = CURRENT_TIMESTAMP \
             WHERE user_id = ?1::uuid AND client_id = ?2 AND revoked_at IS NULL",
            vec![json!(user_id), json!(client_id)],
        ),
        AtomicSqlStatement::guard(
            "INSERT INTO buwiz_server.agent_connections \
                 (connection_id, client_id, user_id, security_revision, permissions) \
             SELECT ?1::uuid, ?2, u.user_id, u.security_revision, ?4::jsonb \
             FROM auth_users u WHERE u.user_id = ?3::uuid AND u.status = 'active' \
             RETURNING connection_id",
            vec![
                json!(connection_id),
                json!(client_id),
                json!(user_id),
                json!(permissions.ids()),
            ],
        ),
        // Re-checks ownership here so a forged form cannot add someone
        // else's profile.
        AtomicSqlStatement::guard(
            "INSERT INTO buwiz_server.agent_connection_profiles (connection_id, profile_id) \
             SELECT ?1::uuid, p.profile_id FROM buwiz_server.tax_profiles p \
             WHERE p.profile_id::text IN (SELECT jsonb_array_elements_text(?2::jsonb)) \
               AND COALESCE(p.is_archived, false) = FALSE \
               AND (p.holder_user_id = ?3::uuid OR p.verified_owner_user_id = ?3::uuid) \
             RETURNING profile_id",
            vec![json!(connection_id), json!(profile_ids), json!(user_id)],
        ),
        AtomicSqlStatement::execute(
            "INSERT INTO buwiz_server.agent_auth_codes \
                 (code_hash, connection_id, redirect_uri, code_challenge, expires_at) \
             VALUES (?1, ?2::uuid, ?3, ?4, CURRENT_TIMESTAMP + (?5::bigint * INTERVAL '1 second'))",
            vec![
                json!(code_hash),
                json!(connection_id),
                json!(redirect_uri),
                json!(code_challenge),
                json!(code_ttl_seconds),
            ],
        ),
    ])
    .await?;
    Ok(())
}

pub(crate) async fn consume_agent_code(code_hash: &str) -> AuthStackResult<Option<ConsumedAgentCode>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "UPDATE buwiz_server.agent_auth_codes a SET consumed_at = CURRENT_TIMESTAMP \
         FROM buwiz_server.agent_connections c \
         WHERE a.code_hash = ?1 AND a.consumed_at IS NULL AND a.expires_at > CURRENT_TIMESTAMP \
           AND c.connection_id = a.connection_id AND c.revoked_at IS NULL \
         RETURNING a.connection_id::text AS connection_id, c.client_id, a.redirect_uri, a.code_challenge",
        vec![json!(code_hash)],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(ConsumedAgentCode {
        connection_id: required_string(row, "connection_id")?,
        client_id: required_string(row, "client_id")?,
        redirect_uri: required_string(row, "redirect_uri")?,
        code_challenge: required_string(row, "code_challenge")?,
    }))
}

fn insert_token_statement(
    token_hash: &str,
    connection_id: &str,
    kind: &str,
    ttl_seconds: i64,
) -> AtomicSqlStatement {
    AtomicSqlStatement::execute(
        "INSERT INTO buwiz_server.agent_tokens (token_hash, connection_id, kind, expires_at) \
         VALUES (?1, ?2::uuid, ?3, CURRENT_TIMESTAMP + (?4::bigint * INTERVAL '1 second'))",
        vec![
            json!(token_hash),
            json!(connection_id),
            json!(kind),
            json!(ttl_seconds),
        ],
    )
}

/// Stores a fresh access + refresh pair for a connection.
pub(crate) async fn insert_agent_tokens(
    connection_id: &str,
    access_hash: &str,
    access_ttl_seconds: i64,
    refresh_hash: &str,
    refresh_ttl_seconds: i64,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql_atomic(vec![
        AtomicSqlStatement::execute(
            "DELETE FROM buwiz_server.agent_tokens \
             WHERE connection_id = ?1::uuid AND expires_at < CURRENT_TIMESTAMP - INTERVAL '1 day'",
            vec![json!(connection_id)],
        ),
        insert_token_statement(access_hash, connection_id, "access", access_ttl_seconds),
        insert_token_statement(refresh_hash, connection_id, "refresh", refresh_ttl_seconds),
    ])
    .await?;
    Ok(())
}

/// Swaps a refresh token for a new pair. `Ok(None)` when the refresh token
/// is unknown, expired, revoked or its connection ended. A refresh token
/// that was already used ends every token of its connection (RFC 9700
/// §4.14.2), so a stolen copy stops working for both sides.
pub(crate) async fn rotate_agent_refresh(
    refresh_hash: &str,
    access_hash: &str,
    access_ttl_seconds: i64,
    new_refresh_hash: &str,
    refresh_ttl_seconds: i64,
) -> AuthStackResult<Option<String>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "UPDATE buwiz_server.agent_tokens t SET revoked_at = CURRENT_TIMESTAMP \
         FROM buwiz_server.agent_connections c, auth_users u \
         WHERE t.token_hash = ?1 AND t.kind = 'refresh' AND t.revoked_at IS NULL \
           AND t.expires_at > CURRENT_TIMESTAMP \
           AND c.connection_id = t.connection_id AND c.revoked_at IS NULL \
           AND u.user_id = c.user_id AND u.status = 'active' \
           AND u.security_revision = c.security_revision \
         RETURNING t.connection_id::text AS connection_id",
        vec![json!(refresh_hash)],
    )
    .await?;
    let Some(connection_id) = rows.first().and_then(|row| row_string(row, "connection_id")) else {
        execute_sql(
            "UPDATE buwiz_server.agent_tokens SET revoked_at = CURRENT_TIMESTAMP \
             WHERE revoked_at IS NULL AND connection_id = ( \
                 SELECT connection_id FROM buwiz_server.agent_tokens \
                 WHERE token_hash = ?1 AND kind = 'refresh' AND revoked_at IS NOT NULL)",
            vec![json!(refresh_hash)],
        )
        .await?;
        return Ok(None);
    };
    insert_agent_tokens(
        &connection_id,
        access_hash,
        access_ttl_seconds,
        new_refresh_hash,
        refresh_ttl_seconds,
    )
    .await?;
    Ok(Some(connection_id))
}

/// The agent behind an access token, or `None` when the token is unknown,
/// expired or revoked, its connection ended, or the taxpayer's account is
/// disabled or had a security change since the Allow.
pub(crate) async fn resolve_agent_access_token(
    access_hash: &str,
) -> AuthStackResult<Option<AgentActor>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT c.connection_id::text AS connection_id, c.user_id::text AS user_id, \
                c.client_id, ac.client_name, c.permissions, \
                COALESCE((SELECT jsonb_agg(cp.profile_id::text) \
                          FROM buwiz_server.agent_connection_profiles cp \
                          WHERE cp.connection_id = c.connection_id), '[]'::jsonb) AS profile_ids \
         FROM buwiz_server.agent_tokens t \
         JOIN buwiz_server.agent_connections c ON c.connection_id = t.connection_id \
         JOIN buwiz_server.agent_clients ac ON ac.client_id = c.client_id \
         JOIN auth_users u ON u.user_id = c.user_id \
         WHERE t.token_hash = ?1 AND t.kind = 'access' AND t.revoked_at IS NULL \
           AND t.expires_at > CURRENT_TIMESTAMP AND c.revoked_at IS NULL \
           AND u.status = 'active' AND u.security_revision = c.security_revision",
        vec![json!(access_hash)],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let actor = AgentActor {
        connection_id: required_string(row, "connection_id")?,
        user_id: required_string(row, "user_id")?,
        client_id: required_string(row, "client_id")?,
        client_name: required_string(row, "client_name")?,
        permissions: PassPermissions::from_stored(&row_string_list(row, "permissions")),
        profile_ids: row_string_list(row, "profile_ids"),
    };
    // Best effort: a failed timestamp must not fail the agent's call.
    let _ = execute_sql(
        "UPDATE buwiz_server.agent_connections SET last_used_at = CURRENT_TIMESTAMP \
         WHERE connection_id = ?1::uuid \
           AND (last_used_at IS NULL OR last_used_at < CURRENT_TIMESTAMP - INTERVAL '1 minute')",
        vec![json!(actor.connection_id)],
    )
    .await;
    Ok(Some(actor))
}

pub(crate) async fn list_agent_connections(
    user_id: &str,
) -> AuthStackResult<Vec<AgentConnectionView>> {
    initialize_schema_async().await?;
    let sql = format!(
        "SELECT c.connection_id::text AS connection_id, c.client_id, ac.client_name, c.permissions, \
                to_char(c.created_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS created_at, \
                to_char(c.last_used_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS last_used_at, \
                COALESCE((SELECT jsonb_agg(jsonb_build_object( \
                              'profile_id', p.profile_id::text, \
                              'profile_name', COALESCE(p.display_name, p.registered_name), \
                              'tin_last4', COALESCE(p.tin_last4, '')) \
                              ORDER BY COALESCE(p.display_name, p.registered_name)) \
                          FROM buwiz_server.agent_connection_profiles cp \
                          JOIN buwiz_server.tax_profiles p ON p.profile_id = cp.profile_id \
                          WHERE cp.connection_id = c.connection_id), '[]'::jsonb) AS profiles \
         FROM buwiz_server.agent_connections c \
         JOIN buwiz_server.agent_clients ac ON ac.client_id = c.client_id \
         WHERE c.user_id = ?1::uuid AND c.revoked_at IS NULL \
         ORDER BY c.created_at DESC"
    );
    let rows = execute_sql(&sql, vec![json!(user_id)]).await?;
    rows.iter()
        .map(|row| {
            let permissions = PassPermissions::from_stored(&row_string_list(row, "permissions"));
            Ok(AgentConnectionView {
                connection_id: required_string(row, "connection_id")?,
                client_id: required_string(row, "client_id")?,
                client_name: required_string(row, "client_name")?,
                permissions: permissions.ids(),
                preset: permissions.preset().map(|preset| preset.as_str().to_owned()),
                profiles: profile_refs(row.get("profiles")),
                created_at: required_string(row, "created_at")?,
                last_used_at: row_string(row, "last_used_at"),
            })
        })
        .collect()
}

fn profile_refs(value: Option<&Value>) -> Vec<AgentProfileRef> {
    let parsed;
    let items = match value {
        Some(Value::Array(items)) => items,
        Some(Value::String(raw)) => {
            parsed = serde_json::from_str::<Vec<Value>>(raw).unwrap_or_default();
            &parsed
        }
        _ => return Vec::new(),
    };
    items
        .iter()
        .map(|item| AgentProfileRef {
            profile_id: row_string(item, "profile_id").unwrap_or_default(),
            profile_name: row_string(item, "profile_name").unwrap_or_default(),
            tin_last4: row_string(item, "tin_last4").unwrap_or_default(),
        })
        .collect()
}

/// Ends one of the caller's agent connections and every token it holds.
pub(crate) async fn revoke_agent_connection(
    user_id: &str,
    connection_id: &str,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "UPDATE buwiz_server.agent_connections SET revoked_at = CURRENT_TIMESTAMP \
         WHERE connection_id::text = ?1 AND user_id = ?2::uuid AND revoked_at IS NULL \
         RETURNING connection_id",
        vec![json!(connection_id), json!(user_id)],
    )
    .await?;
    if rows.is_empty() {
        return Err(AuthStackError::not_found("agent connection not found"));
    }
    execute_sql(
        "UPDATE buwiz_server.agent_tokens SET revoked_at = CURRENT_TIMESTAMP \
         WHERE connection_id::text = ?1 AND revoked_at IS NULL",
        vec![json!(connection_id)],
    )
    .await?;
    Ok(())
}
