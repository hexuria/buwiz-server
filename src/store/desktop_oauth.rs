//! Durable first-party OAuth authorization and device codes.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::error::{AuthStackError, AuthStackResult};

use super::{
    execute_sql, initialize_schema_async, required_string, row_i64, row_string,
};

#[derive(Clone, Debug)]
pub(crate) struct StoredAuthCode {
    pub client_id: String,
    pub user_id: String,
    pub session_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub scope: String,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredDeviceCode {
    pub client_id: String,
    pub user_code: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub interval_seconds: i64,
    pub authorized: bool,
}

pub(crate) async fn insert_auth_code(
    code_hash: &str,
    client_id: &str,
    user_id: &str,
    session_id: &str,
    redirect_uri: &str,
    code_challenge: &str,
    scope: &str,
    ttl_seconds: i64,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql(
        "INSERT INTO buwiz_server.oauth_auth_codes (\
            code_hash, client_id, user_id, session_id, redirect_uri, code_challenge, \
            code_challenge_method, scope, expires_at \
         ) VALUES (?1, ?2, ?3::uuid, ?4, ?5, ?6, 'S256', ?7, to_timestamp(?8))",
        vec![
            json!(code_hash),
            json!(client_id),
            json!(user_id),
            json!(session_id),
            json!(redirect_uri),
            json!(code_challenge),
            json!(scope),
            json!(unix_now() + ttl_seconds),
        ],
    )
    .await?;
    Ok(())
}

pub(crate) async fn consume_auth_code(code_hash: &str) -> AuthStackResult<StoredAuthCode> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "UPDATE buwiz_server.oauth_auth_codes \
         SET consumed_at = CURRENT_TIMESTAMP \
         WHERE code_hash = ?1 AND consumed_at IS NULL AND expires_at > CURRENT_TIMESTAMP \
         RETURNING client_id, user_id::text AS user_id, session_id, redirect_uri, \
                   code_challenge, code_challenge_method, scope",
        vec![json!(code_hash)],
    )
    .await?;
    let row = rows.first().ok_or(AuthStackError::InvalidToken)?;
    Ok(StoredAuthCode {
        client_id: required_string(row, "client_id")?,
        user_id: required_string(row, "user_id")?,
        session_id: required_string(row, "session_id")?,
        redirect_uri: required_string(row, "redirect_uri")?,
        code_challenge: required_string(row, "code_challenge")?,
        code_challenge_method: required_string(row, "code_challenge_method")?,
        scope: required_string(row, "scope")?,
    })
}

pub(crate) async fn insert_device_code(
    device_code_hash: &str,
    user_code: &str,
    client_id: &str,
    verification_uri: &str,
    interval_seconds: i64,
    ttl_seconds: i64,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql(
        "INSERT INTO buwiz_server.oauth_device_codes (\
            device_code_hash, user_code, client_id, verification_uri, interval_seconds, expires_at \
         ) VALUES (?1, ?2, ?3, ?4, ?5, to_timestamp(?6))",
        vec![
            json!(device_code_hash),
            json!(user_code),
            json!(client_id),
            json!(verification_uri),
            json!(interval_seconds),
            json!(unix_now() + ttl_seconds),
        ],
    )
    .await?;
    Ok(())
}

pub(crate) async fn load_device_by_user_code(user_code: &str) -> AuthStackResult<StoredDeviceCode> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT device_code_hash, client_id, user_code, user_id::text AS user_id, session_id, \
                interval_seconds, authorized_at IS NOT NULL AS authorized \
         FROM buwiz_server.oauth_device_codes \
         WHERE user_code = ?1 AND consumed_at IS NULL AND expires_at > CURRENT_TIMESTAMP",
        vec![json!(user_code.trim().to_ascii_uppercase())],
    )
    .await?;
    let row = rows
        .first()
        .ok_or_else(|| AuthStackError::not_found("device code is unknown or expired"))?;
    device_from_row(row)
}

pub(crate) async fn authorize_device_code(
    user_code: &str,
    user_id: &str,
    session_id: &str,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "UPDATE buwiz_server.oauth_device_codes \
         SET user_id = ?2::uuid, session_id = ?3, authorized_at = CURRENT_TIMESTAMP \
         WHERE user_code = ?1 AND consumed_at IS NULL AND expires_at > CURRENT_TIMESTAMP \
         RETURNING user_code",
        vec![
            json!(user_code.trim().to_ascii_uppercase()),
            json!(user_id),
            json!(session_id),
        ],
    )
    .await?;
    if rows.is_empty() {
        return Err(AuthStackError::not_found(
            "device code is unknown or expired",
        ));
    }
    Ok(())
}

pub(crate) async fn poll_device_code(device_code_hash: &str) -> AuthStackResult<StoredDeviceCode> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT client_id, user_code, user_id::text AS user_id, session_id, interval_seconds, \
                authorized_at IS NOT NULL AS authorized, \
                expires_at < CURRENT_TIMESTAMP AS expired \
         FROM buwiz_server.oauth_device_codes \
         WHERE device_code_hash = ?1 AND consumed_at IS NULL",
        vec![json!(device_code_hash)],
    )
    .await?;
    let row = rows.first().ok_or(AuthStackError::InvalidToken)?;
    if row_string(row, "expired").as_deref() == Some("true")
        || super::row_bool(row, "expired") == Some(true)
    {
        return Err(AuthStackError::InvalidToken);
    }
    device_from_row(row)
}

pub(crate) async fn consume_device_code(device_code_hash: &str) -> AuthStackResult<StoredDeviceCode> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "UPDATE buwiz_server.oauth_device_codes \
         SET consumed_at = CURRENT_TIMESTAMP \
         WHERE device_code_hash = ?1 AND consumed_at IS NULL AND authorized_at IS NOT NULL \
           AND expires_at > CURRENT_TIMESTAMP \
         RETURNING client_id, user_code, user_id::text AS user_id, session_id, interval_seconds, \
                   TRUE AS authorized",
        vec![json!(device_code_hash)],
    )
    .await?;
    let row = rows.first().ok_or(AuthStackError::InvalidToken)?;
    device_from_row(row)
}

fn device_from_row(row: &serde_json::Value) -> AuthStackResult<StoredDeviceCode> {
    Ok(StoredDeviceCode {
        client_id: required_string(row, "client_id")?,
        user_code: required_string(row, "user_code")?,
        user_id: row_string(row, "user_id"),
        session_id: row_string(row, "session_id"),
        interval_seconds: row_i64(row, "interval_seconds").unwrap_or(5),
        authorized: super::row_bool(row, "authorized").unwrap_or(false),
    })
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
