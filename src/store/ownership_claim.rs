//! Ownership claims with Form 2303 (`0009_ownership_claims`).
//!
//! The file is encrypted with AES-256-GCM under a key derived from the
//! server key (`OWNERSHIP_CLAIM_FILE_INFO`), with the claim id as associated
//! data so a ciphertext cannot be moved to another claim. Every closing
//! statement clears the file in the same row update.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{json, Value};

use crate::contracts::OwnershipClaimView;
use crate::domain::ownership_claim::ClaimCaseStatus;
use crate::domain::provenance::{ScreenedUpload, UploadKind};
use crate::error::{AuthStackError, AuthStackResult};

use super::{
    AtomicSqlStatement, execute_sql, execute_sql_atomic, initialize_schema_async, required_string,
    row_bool, row_i64, row_string,
};

const NONCE_BYTES: usize = 12;
const TIMESTAMP: &str = "'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'";

#[derive(Clone, Debug)]
pub(crate) struct StoredClaim {
    pub claim_id: String,
    pub profile_id: String,
    pub claimant_user_id: String,
    pub status: ClaimCaseStatus,
}

pub(crate) async fn claim_file_key() -> AuthStackResult<[u8; 32]> {
    crate::auth_product::derived_key(crate::auth_product::OWNERSHIP_CLAIM_FILE_INFO).await
}

/// Associated data: the whole file is bound to its claim, and each upload
/// part to its claim and position.
fn aad(claim_id: &str, part: Option<usize>) -> Vec<u8> {
    match part {
        None => format!("buwiz:ownership-claim:{claim_id}").into_bytes(),
        Some(index) => format!("buwiz:ownership-claim:{claim_id}:part:{index}").into_bytes(),
    }
}

/// `(nonce, ciphertext)`, both base64.
pub(crate) fn encrypt_claim_file(
    claim_id: &str,
    plaintext: &[u8],
    key: &[u8; 32],
) -> AuthStackResult<(String, String)> {
    encrypt_bound(&aad(claim_id, None), plaintext, key)
}

pub(crate) fn encrypt_claim_part(
    claim_id: &str,
    index: usize,
    plaintext: &[u8],
    key: &[u8; 32],
) -> AuthStackResult<(String, String)> {
    encrypt_bound(&aad(claim_id, Some(index)), plaintext, key)
}

pub(crate) fn decrypt_claim_part(
    claim_id: &str,
    index: usize,
    nonce_b64: &str,
    ciphertext_b64: &str,
    key: &[u8; 32],
) -> AuthStackResult<Vec<u8>> {
    decrypt_bound(&aad(claim_id, Some(index)), nonce_b64, ciphertext_b64, key)
}

fn encrypt_bound(aad: &[u8], plaintext: &[u8], key: &[u8; 32]) -> AuthStackResult<(String, String)> {
    use aes_gcm::{
        Aes256Gcm, Nonce,
        aead::{Aead, KeyInit, Payload},
    };
    let mut nonce = [0_u8; NONCE_BYTES];
    getrandom::getrandom(&mut nonce)
        .map_err(|error| AuthStackError::store(format!("nonce generation failed: {error}")))?;
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| AuthStackError::configuration("claim file key is invalid"))?;
    let ciphertext = cipher
        .encrypt(&Nonce::from(nonce), Payload { msg: plaintext, aad })
        .map_err(|_| AuthStackError::store("claim file encryption failed"))?;
    Ok((STANDARD.encode(nonce), STANDARD.encode(ciphertext)))
}

pub(crate) fn decrypt_claim_file(
    claim_id: &str,
    nonce_b64: &str,
    ciphertext_b64: &str,
    key: &[u8; 32],
) -> AuthStackResult<Vec<u8>> {
    decrypt_bound(&aad(claim_id, None), nonce_b64, ciphertext_b64, key)
}

fn decrypt_bound(
    aad: &[u8],
    nonce_b64: &str,
    ciphertext_b64: &str,
    key: &[u8; 32],
) -> AuthStackResult<Vec<u8>> {
    use aes_gcm::{
        Aes256Gcm, Nonce,
        aead::{Aead, KeyInit, Payload},
    };
    let corrupt = || AuthStackError::store("claim file is corrupt");
    let nonce: [u8; NONCE_BYTES] = STANDARD
        .decode(nonce_b64.trim())
        .map_err(|_| corrupt())?
        .as_slice()
        .try_into()
        .map_err(|_| corrupt())?;
    let ciphertext = STANDARD.decode(ciphertext_b64.trim()).map_err(|_| corrupt())?;
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| AuthStackError::configuration("claim file key is invalid"))?;
    cipher
        .decrypt(&Nonce::from(nonce), Payload { msg: &ciphertext, aad })
        .map_err(|_| corrupt())
}

pub(crate) async fn insert_ownership_claim(
    claim_id: &str,
    profile_id: &str,
    claimant_user_id: &str,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql_atomic(vec![AtomicSqlStatement::guard(
        "INSERT INTO buwiz_server.ownership_claims (claim_id, profile_id, claimant_user_id, status) \
         VALUES (?1::uuid, ?2::uuid, ?3::uuid, 'awaiting_file') \
         RETURNING claim_id::text AS claim_id",
        vec![json!(claim_id), json!(profile_id), json!(claimant_user_id)],
    )])
    .await
    .map_err(|error| {
        let message = error.to_string().to_ascii_lowercase();
        if message.contains("unique") || message.contains("duplicate") {
            AuthStackError::conflict("you already have an open claim for this profile")
        } else {
            error
        }
    })?;
    Ok(())
}

pub(crate) async fn load_ownership_claim(claim_id: &str) -> AuthStackResult<Option<StoredClaim>> {
    initialize_schema_async().await?;
    if claim_id.len() != 36 {
        return Ok(None);
    }
    let rows = execute_sql(
        "SELECT claim_id::text AS claim_id, profile_id::text AS profile_id, \
                claimant_user_id::text AS claimant_user_id, status \
         FROM buwiz_server.ownership_claims WHERE claim_id::text = ?1",
        vec![json!(claim_id)],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(StoredClaim {
        claim_id: required_string(row, "claim_id")?,
        profile_id: required_string(row, "profile_id")?,
        claimant_user_id: required_string(row, "claimant_user_id")?,
        status: ClaimCaseStatus::parse(&required_string(row, "status")?)
            .ok_or_else(|| AuthStackError::store("unknown ownership claim status"))?,
    }))
}

/// Stores (or replaces) the screened, encrypted file; the claim becomes
/// `pending`. Fails if the claim closed meanwhile.
pub(crate) async fn store_ownership_claim_file(
    claim_id: &str,
    screened: &ScreenedUpload,
    nonce_b64: &str,
    ciphertext_b64: &str,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql_atomic(vec![
        delete_parts_statement(claim_id),
        AtomicSqlStatement::guard(
        "UPDATE buwiz_server.ownership_claims SET \
            status = 'pending', file_kind = ?2, file_size = ?3, file_sha256 = ?4, \
            file_nonce = ?5, file_ciphertext = ?6, has_c2pa = ?7, updated_at = CURRENT_TIMESTAMP \
         WHERE claim_id = ?1::uuid AND status IN ('awaiting_file', 'pending') \
         RETURNING claim_id::text AS claim_id",
        vec![
            json!(claim_id),
            json!(screened.kind.as_str()),
            json!(screened.size as i64),
            json!(screened.sha256),
            json!(nonce_b64),
            json!(ciphertext_b64),
            json!(screened.has_c2pa),
        ],
    ),
    ])
    .await
    .map_err(|_| AuthStackError::conflict("this claim is already closed"))?;
    Ok(())
}

/// Counts an upload refused as AI-made.
pub(crate) async fn note_ownership_claim_ai_refusal(claim_id: &str) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql(
        "UPDATE buwiz_server.ownership_claims \
         SET ai_refusals = ai_refusals + 1, updated_at = CURRENT_TIMESTAMP \
         WHERE claim_id = ?1::uuid RETURNING claim_id::text AS claim_id",
        vec![json!(claim_id)],
    )
    .await?;
    Ok(())
}

/// Closes an open claim and deletes its file, in whatever transaction the
/// caller runs it. Aborts that transaction when the claim is not open.
pub(crate) fn close_ownership_claim_statements(
    claim_id: &str,
    status: ClaimCaseStatus,
    reviewer_user_id: Option<&str>,
    note: Option<&str>,
) -> Vec<AtomicSqlStatement> {
    vec![delete_parts_statement(claim_id), AtomicSqlStatement::guard(
        "UPDATE buwiz_server.ownership_claims SET \
            status = ?2, file_nonce = NULL, file_ciphertext = NULL, \
            reviewer_user_id = ?3::uuid, decision_note = ?4, \
            decided_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
         WHERE claim_id = ?1::uuid AND status IN ('awaiting_file', 'pending') \
         RETURNING claim_id::text AS claim_id",
        vec![
            json!(claim_id),
            json!(status.as_str()),
            json!(reviewer_user_id),
            json!(note),
        ],
    )]
}

fn delete_parts_statement(claim_id: &str) -> AtomicSqlStatement {
    AtomicSqlStatement::execute(
        "DELETE FROM buwiz_server.ownership_claim_parts WHERE claim_id = ?1::uuid",
        vec![json!(claim_id)],
    )
}

/// Stores one encrypted part. Part 0 drops any earlier upload first; any
/// other part is stored only if it is the next one (else the transaction
/// aborts).
pub(crate) async fn store_ownership_claim_part(
    claim_id: &str,
    index: usize,
    nonce_b64: &str,
    ciphertext_b64: &str,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    let mut statements = Vec::new();
    if index == 0 {
        statements.push(delete_parts_statement(claim_id));
    }
    statements.push(AtomicSqlStatement::guard(
        "INSERT INTO buwiz_server.ownership_claim_parts (claim_id, part_index, nonce, ciphertext) \
         SELECT ?1::uuid, ?2::bigint, ?3, ?4 \
         WHERE (SELECT count(*) FROM buwiz_server.ownership_claim_parts WHERE claim_id = ?1::uuid) = ?2::bigint \
           AND EXISTS (SELECT 1 FROM buwiz_server.ownership_claims \
                        WHERE claim_id = ?1::uuid AND status IN ('awaiting_file', 'pending')) \
         RETURNING claim_id::text AS claim_id",
        vec![
            json!(claim_id),
            json!(index as i64),
            json!(nonce_b64),
            json!(ciphertext_b64),
        ],
    ));
    execute_sql_atomic(statements)
        .await
        .map_err(|_| AuthStackError::conflict("parts must arrive in order; start the upload again"))?;
    Ok(())
}

/// How many parts of the current upload are stored.
pub(crate) async fn ownership_claim_parts_received(claim_id: &str) -> AuthStackResult<usize> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT count(*) AS parts FROM buwiz_server.ownership_claim_parts WHERE claim_id = ?1::uuid",
        vec![json!(claim_id)],
    )
    .await?;
    Ok(rows
        .first()
        .and_then(|row| row_i64(row, "parts"))
        .and_then(|parts| usize::try_from(parts).ok())
        .unwrap_or(0))
}

/// The stored parts, in order: `(index, nonce, ciphertext)`.
pub(crate) async fn ownership_claim_parts(claim_id: &str) -> AuthStackResult<Vec<(usize, String, String)>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT part_index, nonce, ciphertext FROM buwiz_server.ownership_claim_parts \
         WHERE claim_id = ?1::uuid ORDER BY part_index",
        vec![json!(claim_id)],
    )
    .await?;
    rows.iter()
        .map(|row| {
            let index = row_i64(row, "part_index")
                .and_then(|index| usize::try_from(index).ok())
                .ok_or_else(|| AuthStackError::store("upload part has no index"))?;
            Ok((index, required_string(row, "nonce")?, required_string(row, "ciphertext")?))
        })
        .collect()
}

/// Drops an upload in progress (a refused or abandoned file).
pub(crate) async fn discard_ownership_claim_parts(claim_id: &str) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql_atomic(vec![delete_parts_statement(claim_id)]).await?;
    Ok(())
}

pub(crate) async fn close_ownership_claim(
    claim_id: &str,
    status: ClaimCaseStatus,
    reviewer_user_id: Option<&str>,
    note: Option<&str>,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql_atomic(close_ownership_claim_statements(
        claim_id,
        status,
        reviewer_user_id,
        note,
    ))
    .await
    .map_err(|_| AuthStackError::conflict("this claim is already closed"))?;
    Ok(())
}

/// `(kind, nonce, ciphertext)` of an open claim's file.
pub(crate) async fn ownership_claim_file(
    claim_id: &str,
) -> AuthStackResult<Option<(UploadKind, String, String)>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT file_kind, file_nonce, file_ciphertext FROM buwiz_server.ownership_claims \
         WHERE claim_id::text = ?1 AND file_ciphertext IS NOT NULL",
        vec![json!(claim_id)],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let kind = row_string(row, "file_kind")
        .and_then(|kind| UploadKind::parse(&kind))
        .ok_or_else(|| AuthStackError::store("claim file kind is missing"))?;
    Ok(Some((
        kind,
        required_string(row, "file_nonce")?,
        required_string(row, "file_ciphertext")?,
    )))
}

fn claim_columns() -> String {
    format!(
        "c.claim_id::text AS claim_id, c.profile_id::text AS profile_id, c.status, \
         c.file_kind, c.file_size, c.file_sha256, c.has_c2pa, c.ai_refusals, c.decision_note, \
         to_char(c.created_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS created_at, \
         to_char(c.updated_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS updated_at, \
         to_char(c.decided_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS decided_at, \
         COALESCE(p.display_name, p.registered_name) AS profile_name, p.tin_last4, \
         u.primary_email AS claimant_email, \
         (SELECT count(*) FROM buwiz_server.ownership_claim_parts pt \
           WHERE pt.claim_id = c.claim_id) AS parts_received"
    )
}

/// The caller's own claims, newest first.
pub(crate) async fn list_ownership_claims_for(user_id: &str) -> AuthStackResult<Vec<OwnershipClaimView>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        &format!(
            "SELECT {} FROM buwiz_server.ownership_claims c \
             JOIN buwiz_server.tax_profiles p ON p.profile_id = c.profile_id \
             JOIN auth_users u ON u.user_id = c.claimant_user_id \
             WHERE c.claimant_user_id = ?1::uuid ORDER BY c.created_at DESC",
            claim_columns()
        ),
        vec![json!(user_id)],
    )
    .await?;
    rows.iter().map(|row| claim_view(row, false)).collect()
}

/// Claims waiting for staff, oldest first.
pub(crate) async fn list_pending_ownership_claims() -> AuthStackResult<Vec<OwnershipClaimView>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        &format!(
            "SELECT {} FROM buwiz_server.ownership_claims c \
             JOIN buwiz_server.tax_profiles p ON p.profile_id = c.profile_id \
             JOIN auth_users u ON u.user_id = c.claimant_user_id \
             WHERE c.status = ?1 ORDER BY c.updated_at ASC",
            claim_columns()
        ),
        vec![json!(ClaimCaseStatus::Pending.as_str())],
    )
    .await?;
    rows.iter().map(|row| claim_view(row, true)).collect()
}

fn claim_view(row: &Value, for_staff: bool) -> AuthStackResult<OwnershipClaimView> {
    let status = required_string(row, "status")?;
    // Until the claim is approved, the claimant sees only the TIN's last
    // four digits, not how someone else named the profile.
    let profile_name = if for_staff || status == ClaimCaseStatus::Approved.as_str() {
        row_string(row, "profile_name").unwrap_or_default()
    } else {
        String::new()
    };
    Ok(OwnershipClaimView {
        claim_id: required_string(row, "claim_id")?,
        profile_id: required_string(row, "profile_id")?,
        profile_name,
        tin_last4: row_string(row, "tin_last4").unwrap_or_default(),
        status,
        file_kind: row_string(row, "file_kind"),
        file_size: row_i64(row, "file_size").and_then(|size| u64::try_from(size).ok()),
        file_sha256: row_string(row, "file_sha256"),
        has_c2pa: row_bool(row, "has_c2pa").unwrap_or(false),
        ai_refusals: row_i64(row, "ai_refusals")
            .and_then(|count| u64::try_from(count).ok())
            .unwrap_or(0),
        parts_received: row_i64(row, "parts_received")
            .and_then(|count| u64::try_from(count).ok())
            .unwrap_or(0),
        claimant_email: if for_staff {
            row_string(row, "claimant_email")
        } else {
            None
        },
        decision_note: row_string(row, "decision_note"),
        created_at: row_string(row, "created_at").unwrap_or_default(),
        updated_at: row_string(row, "updated_at").unwrap_or_default(),
        decided_at: row_string(row, "decided_at"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7; 32];
    const CLAIM: &str = "11111111-1111-4111-8111-111111111111";

    #[test]
    fn files_round_trip_and_stay_bound_to_their_claim() {
        let file = b"\xFF\xD8\xFF scanned 2303".to_vec();
        let (nonce, ciphertext) = encrypt_claim_file(CLAIM, &file, &KEY).unwrap();
        assert_eq!(decrypt_claim_file(CLAIM, &nonce, &ciphertext, &KEY).unwrap(), file);
        // Another claim, another key or a changed byte: refused.
        let other = "22222222-2222-4222-8222-222222222222";
        assert!(decrypt_claim_file(other, &nonce, &ciphertext, &KEY).is_err());
        assert!(decrypt_claim_file(CLAIM, &nonce, &ciphertext, &[8; 32]).is_err());
        let mut raw = STANDARD.decode(&ciphertext).unwrap();
        raw[0] ^= 1;
        assert!(decrypt_claim_file(CLAIM, &nonce, &STANDARD.encode(raw), &KEY).is_err());
        assert!(decrypt_claim_file(CLAIM, "short", &ciphertext, &KEY).is_err());
        assert!(decrypt_claim_file(CLAIM, &nonce, "%%%", &KEY).is_err());
    }

    #[test]
    fn parts_are_bound_to_their_claim_and_position() {
        let (nonce, ciphertext) = encrypt_claim_part(CLAIM, 3, b"part three", &KEY).unwrap();
        assert_eq!(
            decrypt_claim_part(CLAIM, 3, &nonce, &ciphertext, &KEY).unwrap(),
            b"part three"
        );
        assert!(decrypt_claim_part(CLAIM, 4, &nonce, &ciphertext, &KEY).is_err());
        // A part can't pass as the whole file, or the file as a part.
        assert!(decrypt_claim_file(CLAIM, &nonce, &ciphertext, &KEY).is_err());
        let (nonce, ciphertext) = encrypt_claim_file(CLAIM, b"whole", &KEY).unwrap();
        assert!(decrypt_claim_part(CLAIM, 0, &nonce, &ciphertext, &KEY).is_err());
    }

    #[test]
    fn every_encryption_uses_a_fresh_nonce() {
        let (a, _) = encrypt_claim_file(CLAIM, b"same", &KEY).unwrap();
        let (b, _) = encrypt_claim_file(CLAIM, b"same", &KEY).unwrap();
        assert_ne!(a, b);
    }
}
