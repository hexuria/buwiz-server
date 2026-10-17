//! Ownership claims with Form 2303: the claimant's steps and the staff
//! review queue. See `domain::ownership_claim` and `domain::provenance`.
//!
//! Buwiz staff are system administrators with `system.user.manage` and a
//! recent step-up; nobody decides their own claim.

use crate::contracts::{
    OwnershipClaimDecisionRequest, OwnershipClaimListResponse, OwnershipClaimPartRequest,
    OwnershipClaimStartRequest, OwnershipClaimView,
};
use crate::domain::ownership_claim::{check_part, next_status, ClaimCaseStatus, ClaimStep};
use crate::domain::provenance::{screen_upload, UploadKind, UploadRefusal};
use crate::domain::{AccountHolder, ProofMethod, TaxProfileCommand, TinIdentityHash};
use crate::error::{AuthStackError, AuthStackResult};

use super::{parse_registration, require_step_up_permission_for, require_verified_actor, RequestAuth};

const STAFF_PERMISSION: &str = "system.user.manage";

/// Starts a claim on the profile registered for this TIN and branch. The
/// file follows with [`upload_ownership_claim_file`].
pub async fn start_ownership_claim(
    request: OwnershipClaimStartRequest,
    auth: RequestAuth,
) -> AuthStackResult<OwnershipClaimView> {
    let (user_id, _) = require_verified_actor(auth).await?;
    let registration = parse_registration(&request.tin_root, &request.branch_code)?;
    let pepper = crate::store::tin_identity_pepper().await?;
    let identity_hash =
        TinIdentityHash::compute(&pepper, &registration.tin_root, &registration.branch_code);
    let loaded = crate::store::load_tax_profile_by_identity_hash(&identity_hash)
        .await?
        .filter(|loaded| loaded.state.exists)
        .ok_or_else(|| AuthStackError::not_found("no tax profile for that TIN and branch"))?;
    if loaded.from_projection {
        return Err(AuthStackError::validation(
            "this profile was stored before claims existed and cannot be claimed yet",
        ));
    }
    if matches!(&loaded.state.holder, Some(AccountHolder::Personal { user_id: holder }) if holder == &user_id)
    {
        return Err(AuthStackError::validation("you already own this tax profile"));
    }
    let profile_id = loaded
        .state
        .profile_id
        .clone()
        .ok_or_else(|| AuthStackError::not_found("tax profile not found"))?;
    let claim_id = crate::store::new_uuid_v4()?;
    crate::store::insert_ownership_claim(&claim_id, &profile_id, &user_id).await?;
    my_claim(&user_id, &claim_id).await
}

/// One part of the Form 2303 (the browser sends the file in order, in
/// parts of at most 4 KiB; see `PART_BYTES`). Each part is encrypted on arrival. After the
/// last part the whole file is screened: an AI-made file is refused,
/// counted and dropped; anything else is stored encrypted and the claim
/// waits for staff.
pub async fn upload_ownership_claim_part(
    claim_id: String,
    request: OwnershipClaimPartRequest,
    auth: RequestAuth,
) -> AuthStackResult<OwnershipClaimView> {
    use base64::Engine as _;
    let (user_id, _) = require_verified_actor(auth).await?;
    let claim = claim_for_step(&claim_id, &user_id, ClaimStep::Upload).await?;
    let part = base64::engine::general_purpose::STANDARD
        .decode(request.data.trim())
        .map_err(|_| AuthStackError::validation("upload part is not valid base64"))?;
    let (index, total) = (request.index as usize, request.total as usize);
    let received = crate::store::ownership_claim_parts_received(&claim.claim_id).await?;
    check_part(index, total, part.len(), received).map_err(AuthStackError::validation)?;
    let key = crate::store::claim_file_key().await?;
    let (nonce, ciphertext) = crate::store::encrypt_claim_part(&claim.claim_id, index, &part, &key)?;
    crate::store::store_ownership_claim_part(&claim.claim_id, index, &nonce, &ciphertext).await?;
    if index + 1 == total {
        finish_upload(&claim.claim_id, total, &key).await?;
    }
    my_claim(&user_id, &claim.claim_id).await
}

/// Joins the parts, screens the file, and stores it or refuses it.
async fn finish_upload(claim_id: &str, total: usize, key: &[u8; 32]) -> AuthStackResult<()> {
    let parts = crate::store::ownership_claim_parts(claim_id).await?;
    if parts.len() != total || parts.iter().enumerate().any(|(at, (index, _, _))| *index != at) {
        crate::store::discard_ownership_claim_parts(claim_id).await?;
        return Err(AuthStackError::validation(
            "some parts are missing; start the upload again",
        ));
    }
    let mut file = Vec::new();
    for (index, nonce, ciphertext) in &parts {
        file.extend(crate::store::decrypt_claim_part(claim_id, *index, nonce, ciphertext, key)?);
    }
    let screened = match screen_upload(&file) {
        Ok(screened) => screened,
        Err(refusal) => {
            crate::store::discard_ownership_claim_parts(claim_id).await?;
            if matches!(refusal, UploadRefusal::AiMade { .. }) {
                crate::store::note_ownership_claim_ai_refusal(claim_id).await?;
            }
            return Err(AuthStackError::validation(refusal.to_string()));
        }
    };
    let (nonce, ciphertext) = crate::store::encrypt_claim_file(claim_id, &file, key)?;
    crate::store::store_ownership_claim_file(claim_id, &screened, &nonce, &ciphertext).await
}

pub async fn withdraw_ownership_claim(
    claim_id: String,
    auth: RequestAuth,
) -> AuthStackResult<OwnershipClaimListResponse> {
    let (user_id, _) = require_verified_actor(auth).await?;
    let claim = claim_for_step(&claim_id, &user_id, ClaimStep::Withdraw).await?;
    crate::store::close_ownership_claim(&claim.claim_id, ClaimCaseStatus::Withdrawn, None, None)
        .await?;
    Ok(OwnershipClaimListResponse {
        claims: crate::store::list_ownership_claims_for(&user_id).await?,
    })
}

pub async fn list_my_ownership_claims(auth: RequestAuth) -> AuthStackResult<OwnershipClaimListResponse> {
    let (user_id, _) = require_verified_actor(auth).await?;
    Ok(OwnershipClaimListResponse {
        claims: crate::store::list_ownership_claims_for(&user_id).await?,
    })
}

// ── Staff ───────────────────────────────────────────────────────────────────

pub async fn list_ownership_claim_queue(
    auth: RequestAuth,
) -> AuthStackResult<OwnershipClaimListResponse> {
    require_step_up_permission_for(STAFF_PERMISSION, auth).await?;
    Ok(OwnershipClaimListResponse {
        claims: crate::store::list_pending_ownership_claims().await?,
    })
}

/// The decrypted file for a reviewer: `(content type, bytes)`.
pub async fn ownership_claim_file_for_review(
    claim_id: String,
    auth: RequestAuth,
) -> AuthStackResult<(&'static str, Vec<u8>)> {
    let session = require_step_up_permission_for(STAFF_PERMISSION, auth).await?;
    let staff_id = session.user_id.ok_or(AuthStackError::AuthRequired)?;
    let claim = crate::store::load_ownership_claim(&claim_id)
        .await?
        .ok_or_else(|| AuthStackError::not_found("claim not found"))?;
    if claim.claimant_user_id == staff_id {
        return Err(AuthStackError::Forbidden);
    }
    let (kind, nonce, ciphertext): (UploadKind, String, String) =
        crate::store::ownership_claim_file(&claim.claim_id)
            .await?
            .ok_or_else(|| AuthStackError::not_found("this claim has no file"))?;
    let key = crate::store::claim_file_key().await?;
    let bytes = crate::store::decrypt_claim_file(&claim.claim_id, &nonce, &ciphertext, &key)?;
    Ok((kind.mime(), bytes))
}

/// Approve: the claimant becomes the owner (every open pass and invite
/// ends), and the claim closes in the same transaction. Reject: needs a
/// note for the claimant. Either way the file is deleted.
pub async fn decide_ownership_claim(
    claim_id: String,
    approve: bool,
    request: OwnershipClaimDecisionRequest,
    auth: RequestAuth,
) -> AuthStackResult<OwnershipClaimListResponse> {
    let session = require_step_up_permission_for(STAFF_PERMISSION, auth).await?;
    let staff_id = session.user_id.ok_or(AuthStackError::AuthRequired)?;
    let claim = crate::store::load_ownership_claim(&claim_id)
        .await?
        .ok_or_else(|| AuthStackError::not_found("claim not found"))?;
    if claim.claimant_user_id == staff_id {
        return Err(AuthStackError::validation("you cannot decide your own claim"));
    }
    let step = if approve { ClaimStep::Approve } else { ClaimStep::Reject };
    let status = next_status(claim.status, step, false).map_err(AuthStackError::conflict)?;
    let note = request
        .note
        .as_deref()
        .map(str::trim)
        .filter(|note| !note.is_empty())
        .map(|note| note.chars().take(1000).collect::<String>());
    if !approve {
        let note = note.ok_or_else(|| {
            AuthStackError::validation("say why, so the claimant knows what to fix")
        })?;
        crate::store::close_ownership_claim(&claim.claim_id, status, Some(&staff_id), Some(&note))
            .await?;
    } else {
        let loaded = crate::store::load_tax_profile_by_id(&claim.profile_id).await?;
        let identity_hash = loaded
            .state
            .identity_hash
            .clone()
            .ok_or_else(|| AuthStackError::store("tax profile has no identity"))?;
        let claimant = claim.claimant_user_id.clone();
        let command = match &loaded.state.holder {
            Some(AccountHolder::Organization { .. }) => TaxProfileCommand::Claim {
                claimant: AccountHolder::Personal {
                    user_id: claimant.clone(),
                },
                actor_user_id: claimant,
                identity_hash,
                proof_method: ProofMethod::Form2303Review,
            },
            _ => TaxProfileCommand::Reclaim {
                actor_user_id: claimant,
                identity_hash,
                proof_method: ProofMethod::Form2303Review,
            },
        };
        crate::store::commit_tax_profile_command_with(
            &claim.profile_id,
            command,
            loaded.revision,
            crate::store::close_ownership_claim_statements(
                &claim.claim_id,
                status,
                Some(&staff_id),
                note.as_deref(),
            ),
        )
        .await?;
    }
    Ok(OwnershipClaimListResponse {
        claims: crate::store::list_pending_ownership_claims().await?,
    })
}

async fn claim_for_step(
    claim_id: &str,
    user_id: &str,
    step: ClaimStep,
) -> AuthStackResult<crate::store::StoredClaim> {
    let claim = crate::store::load_ownership_claim(claim_id)
        .await?
        .filter(|claim| claim.claimant_user_id == user_id)
        .ok_or_else(|| AuthStackError::not_found("claim not found"))?;
    next_status(claim.status, step, true).map_err(AuthStackError::conflict)?;
    Ok(claim)
}

async fn my_claim(user_id: &str, claim_id: &str) -> AuthStackResult<OwnershipClaimView> {
    crate::store::list_ownership_claims_for(user_id)
        .await?
        .into_iter()
        .find(|claim| claim.claim_id == claim_id)
        .ok_or_else(|| AuthStackError::store("claim missing after write"))
}
