//! Ownership claims with Form 2303: the claimant's steps and the staff
//! review queue. See `domain::ownership_claim` and `domain::provenance`.
//!
//! Buwiz staff are system administrators with `system.user.manage` and a
//! recent step-up; nobody decides their own claim.

use crate::contracts::{
    ClaimAssessmentView, ClaimFindingView, OwnershipClaimDecisionRequest, OwnershipClaimListResponse, OwnershipClaimPartRequest,
    OwnershipClaimStartRequest, OwnershipClaimView,
};
use crate::domain::claim_assessment::{
    DocumentReading, ProfileExpectation, TinCheck, UploadSignals, assess,
};
use crate::domain::ownership_claim::{check_part, next_status, ClaimCaseStatus, ClaimStep};
use crate::domain::provenance::{screen_upload, UploadKind, UploadRefusal};
use crate::domain::{AccountHolder, ProofMethod, RegistrationKey, TaxProfileCommand, TinIdentityHash};
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
        let (kind, file) = finish_upload(&claim.claim_id, total, &key).await?;
        // Best effort: a failed or unconfigured AI check leaves the claim
        // pending for a plain human review.
        if let Err(error) = run_ai_review(&claim, kind, &file).await {
            tracing::warn!(claim = %claim.claim_id, error = %error, "AI review not stored");
        }
    }
    my_claim(&user_id, &claim.claim_id).await
}

/// Joins the parts, screens the file, and stores it or refuses it.
async fn finish_upload(
    claim_id: &str,
    total: usize,
    key: &[u8; 32],
) -> AuthStackResult<(UploadKind, Vec<u8>)> {
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
    crate::store::store_ownership_claim_file(claim_id, &screened, &nonce, &ciphertext).await?;
    Ok((screened.kind, file))
}

/// Has the configured models look at the file and stores their
/// recommendation for staff. Returns whether AI review is configured.
async fn run_ai_review(
    claim: &crate::store::StoredClaim,
    kind: UploadKind,
    file: &[u8],
) -> AuthStackResult<bool> {
    let mut view = ClaimAssessmentView::default();
    let config = match crate::claim_ai::load_config().await {
        Ok(config) if !config.is_enabled() => return Ok(false),
        Ok(config) => Some(config),
        Err(problem) => {
            view.problems.push(format!("AI review is misconfigured: {problem}"));
            None
        }
    };
    let mut recommendation = None;
    if let Some(config) = config {
        let run = crate::claim_ai::read_and_classify(&config, kind, file).await;
        view.reader_model = run.reader_model;
        view.classifier_model = run.classifier_model;
        view.problems = run.problems;
        if let Some(reading) = &run.reading {
            let loaded = crate::store::load_tax_profile_by_id(&claim.profile_id).await?;
            let state = &loaded.state;
            let facts = state.facts.as_ref();
            let known = |value: Option<&String>| value.filter(|v| !v.trim().is_empty()).cloned();
            let expected = ProfileExpectation {
                branch_code: state.branch_code.clone().unwrap_or_else(|| "00000".to_owned()),
                registered_name: known(facts.map(|f| &f.registered_name)),
                rdo_code: known(facts.map(|f| &f.rdo_code)),
                registered_address: known(facts.map(|f| &f.registered_address)),
            };
            let tin = match (read_registration(reading, &expected.branch_code), &state.identity_hash) {
                (Some(key), Some(profile_hash)) => {
                    let pepper = crate::store::tin_identity_pepper().await?;
                    if &TinIdentityHash::compute(&pepper, &key.tin_root, &key.branch_code) == profile_hash {
                        TinCheck::Matches
                    } else {
                        TinCheck::Differs
                    }
                }
                _ => TinCheck::Unreadable,
            };
            let assessment =
                assess(reading, tin, &expected, UploadSignals { ai_refusals: claim.ai_refusals });
            recommendation = Some(snake_case(&assessment.recommendation));
            view.findings = assessment
                .findings
                .iter()
                .map(|finding| ClaimFindingView {
                    check: finding.check.clone(),
                    outcome: snake_case(&finding.outcome),
                    detail: finding.detail.clone(),
                })
                .collect();
        }
    }
    view.recommendation = recommendation.clone();
    let stored = serde_json::to_value(&view)
        .map_err(|_| AuthStackError::store("AI review could not be encoded"))?;
    crate::store::store_ownership_claim_assessment(&claim.claim_id, recommendation.as_deref(), &stored)
        .await?;
    Ok(true)
}

/// The TIN and branch the model read. A branch the document doesn't show
/// is taken to be the profile's, so only the TIN root is compared.
fn read_registration(reading: &DocumentReading, profile_branch: &str) -> Option<RegistrationKey> {
    let digits: String = reading.tin.as_deref()?.chars().filter(char::is_ascii_digit).collect();
    if digits.len() < 9 {
        return None;
    }
    let (root, rest) = digits.split_at(9);
    let branch = reading
        .branch_code
        .as_deref()
        .filter(|code| code.chars().any(|ch| ch.is_ascii_digit()))
        .map(str::to_owned)
        .or_else(|| matches!(rest.len(), 3 | 5).then(|| rest.to_owned()))
        .unwrap_or_else(|| profile_branch.to_owned());
    RegistrationKey::parse(root, &branch).ok()
}

fn snake_case(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
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

/// Staff: runs the AI review aid again on a pending claim's file (after a
/// configuration change, or when the upload-time run failed).
pub async fn reassess_ownership_claim(
    claim_id: String,
    auth: RequestAuth,
) -> AuthStackResult<OwnershipClaimListResponse> {
    let session = require_step_up_permission_for(STAFF_PERMISSION, auth).await?;
    let staff_id = session.user_id.ok_or(AuthStackError::AuthRequired)?;
    let claim = crate::store::load_ownership_claim(&claim_id)
        .await?
        .ok_or_else(|| AuthStackError::not_found("claim not found"))?;
    if claim.claimant_user_id == staff_id {
        return Err(AuthStackError::Forbidden);
    }
    if claim.status != ClaimCaseStatus::Pending {
        return Err(AuthStackError::conflict("only a pending claim can be checked"));
    }
    let (kind, nonce, ciphertext) = crate::store::ownership_claim_file(&claim.claim_id)
        .await?
        .ok_or_else(|| AuthStackError::not_found("this claim has no file"))?;
    let key = crate::store::claim_file_key().await?;
    let file = crate::store::decrypt_claim_file(&claim.claim_id, &nonce, &ciphertext, &key)?;
    if !run_ai_review(&claim, kind, &file).await? {
        return Err(AuthStackError::configuration(
            "AI review is not configured: set AI_GATEWAY_API_KEY or a per-role key",
        ));
    }
    Ok(OwnershipClaimListResponse {
        claims: crate::store::list_pending_ownership_claims().await?,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn read(tin: &str, branch: Option<&str>) -> DocumentReading {
        DocumentReading {
            is_form_2303: true,
            document_confidence: 0.95,
            tin: Some(tin.to_owned()),
            branch_code: branch.map(str::to_owned),
            ..DocumentReading::default()
        }
    }

    #[test]
    fn the_branch_comes_from_the_document_then_the_tin_then_the_profile() {
        let key = read_registration(&read("123-456-789-001", None), "00000").unwrap();
        assert_eq!((key.tin_root.as_str(), key.branch_code.as_str()), ("123456789", "00001"));
        let key = read_registration(&read("123 456 789", Some("00002")), "00000").unwrap();
        assert_eq!(key.branch_code.as_str(), "00002");
        let key = read_registration(&read("123456789", None), "00007").unwrap();
        assert_eq!(key.branch_code.as_str(), "00007");
        assert!(read_registration(&read("12345", None), "00000").is_none());
        assert!(read_registration(&DocumentReading::default(), "00000").is_none());
    }

    #[test]
    fn recommendations_and_outcomes_are_stored_in_snake_case() {
        use crate::domain::claim_assessment::{Outcome, Recommendation};
        assert_eq!(snake_case(&Recommendation::NeedsReview), "needs_review");
        assert_eq!(snake_case(&Outcome::Unchecked), "unchecked");
    }
}
