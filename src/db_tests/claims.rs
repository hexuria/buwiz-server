//! Ownership claims with Form 2303 (`application::ownership_claim`).

use base64::Engine as _;
use serde_json::json;

use super::*;
use crate::application as app;
use crate::contracts::{
    OwnershipClaimDecisionRequest, OwnershipClaimPartRequest, OwnershipClaimStartRequest,
    OwnershipClaimView,
};

const PDF: &[u8] = b"%PDF-1.4\n% Form 2303 test fixture, not a real certificate\n%%EOF\n";

/// A firm's draft profile and a taxpayer claiming it.
struct Case {
    firm_member: User,
    claimant: User,
    profile_id: String,
    claim: OwnershipClaimView,
}

async fn case() -> Case {
    let firm_member = user().await;
    let org = firm(&firm_member, &[MANAGE]).await;
    let (profile_id, tin_root) = profile(&firm_member, Some(&org)).await;
    let claimant = user().await;
    let claim = app::start_ownership_claim(
        OwnershipClaimStartRequest {
            tin_root,
            branch_code: "00000".to_owned(),
        },
        claimant.auth(),
    )
    .await
    .expect("start claim");
    assert_eq!(claim.status, "awaiting_file");
    assert_eq!(claim.profile_id, profile_id);
    Case {
        firm_member,
        claimant,
        profile_id,
        claim,
    }
}

async fn send(case: &Case, index: u32, total: u32, bytes: &[u8]) -> AuthStackResult<OwnershipClaimView> {
    app::upload_ownership_claim_part(
        case.claim.claim_id.clone(),
        OwnershipClaimPartRequest {
            index,
            total,
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
        },
        case.claimant.auth(),
    )
    .await
}

/// Uploads `file` in two parts and returns the claim after the last.
async fn upload(case: &Case, file: &[u8]) -> AuthStackResult<OwnershipClaimView> {
    let (head, tail) = file.split_at(file.len() / 2);
    let first = send(case, 0, 2, head).await?;
    assert_eq!(first.status, "awaiting_file", "one part of two is not the whole file");
    send(case, 1, 2, tail).await
}

fn staff_user(user: &User) -> &User {
    make_staff(user);
    user
}

fn note(text: &str) -> OwnershipClaimDecisionRequest {
    OwnershipClaimDecisionRequest {
        note: Some(text.to_owned()),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn the_last_part_screens_and_stores_the_file_for_staff() {
    if !database() {
        return;
    }
    let case = case().await;
    let claim = upload(&case, PDF).await.expect("upload");
    assert_eq!(claim.status, "pending");
    assert_eq!(claim.file_size, Some(PDF.len() as u64));

    let staff = user().await;
    let staff = staff_user(&staff);
    let queue = app::list_ownership_claim_queue(staff.auth()).await.expect("queue");
    assert!(queue.claims.iter().any(|c| c.claim_id == claim.claim_id));
    let (mime, bytes) = app::ownership_claim_file_for_review(claim.claim_id.clone(), staff.auth())
        .await
        .expect("file");
    assert_eq!(mime, "application/pdf");
    assert_eq!(bytes, PDF);

    // Without the staff permission there is no queue and no file.
    let outsider = user().await;
    assert!(app::list_ownership_claim_queue(outsider.auth()).await.is_err());
    assert!(app::ownership_claim_file_for_review(claim.claim_id, outsider.auth()).await.is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn staff_never_read_or_decide_their_own_claim() {
    if !database() {
        return;
    }
    let case = case().await;
    upload(&case, PDF).await.expect("upload");
    make_staff(&case.claimant);
    let read = app::ownership_claim_file_for_review(case.claim.claim_id.clone(), case.claimant.auth()).await;
    assert!(matches!(read, Err(AuthStackError::Forbidden)), "{read:?}");
    let decide = app::decide_ownership_claim(case.claim.claim_id.clone(), true, note("mine"), case.claimant.auth()).await;
    assert!(matches!(decide, Err(AuthStackError::Validation { .. })), "{decide:?}");
}

#[tokio::test(flavor = "current_thread")]
async fn approving_a_firm_draft_makes_the_claimant_its_owner() {
    if !database() {
        return;
    }
    let case = case().await;
    upload(&case, PDF).await.expect("upload");
    let staff = user().await;
    make_staff(&staff);
    let queue = app::decide_ownership_claim(
        case.claim.claim_id.clone(),
        true,
        OwnershipClaimDecisionRequest { note: None },
        staff.auth(),
    )
    .await
    .expect("approve");
    assert!(queue.claims.iter().all(|c| c.claim_id != case.claim.claim_id), "approved leaves the queue");

    let view = app::get_tax_profile(case.profile_id.clone(), case.claimant.auth())
        .await
        .expect("the new owner sees the profile");
    assert_eq!(view.owner_user_id.as_deref(), Some(case.claimant.user_id.as_str()));
    let mine = app::list_my_ownership_claims(case.claimant.auth()).await.expect("mine");
    let claim = mine.claims.iter().find(|c| c.claim_id == case.claim.claim_id).expect("listed");
    assert_eq!(claim.status, "approved");
    let _ = &case.firm_member;
}

#[tokio::test(flavor = "current_thread")]
async fn rejecting_needs_a_note_for_the_claimant() {
    if !database() {
        return;
    }
    let case = case().await;
    upload(&case, PDF).await.expect("upload");
    let staff = user().await;
    make_staff(&staff);
    for empty in [None, Some("   ".to_owned())] {
        let refused = app::decide_ownership_claim(
            case.claim.claim_id.clone(),
            false,
            OwnershipClaimDecisionRequest { note: empty },
            staff.auth(),
        )
        .await;
        assert!(matches!(refused, Err(AuthStackError::Validation { .. })), "{refused:?}");
    }
    app::decide_ownership_claim(case.claim.claim_id.clone(), false, note("blurry scan"), staff.auth())
        .await
        .expect("reject");
    let mine = app::list_my_ownership_claims(case.claimant.auth()).await.expect("mine");
    let claim = mine.claims.iter().find(|c| c.claim_id == case.claim.claim_id).expect("listed");
    assert_eq!(claim.status, "rejected");
    // The firm still holds its draft.
    let still = app::get_tax_profile(case.profile_id.clone(), case.claimant.auth()).await;
    assert!(still.is_err(), "a rejected claimant gets nothing");
}

#[tokio::test(flavor = "current_thread")]
async fn an_ai_made_file_is_refused_and_dropped() {
    if !database() {
        return;
    }
    let case = case().await;
    let mut fake = PDF.to_vec();
    fake.extend_from_slice(b"trainedAlgorithmicMedia");
    let refused = upload(&case, &fake).await;
    assert!(matches!(refused, Err(AuthStackError::Validation { .. })), "{refused:?}");
    assert_eq!(crate::store::ownership_claim_parts_received(&case.claim.claim_id).await.unwrap(), 0);
    let mine = app::list_my_ownership_claims(case.claimant.auth()).await.expect("mine");
    assert_eq!(mine.claims[0].status, "awaiting_file");
}

#[tokio::test(flavor = "current_thread")]
async fn a_gap_in_the_stored_parts_is_refused_and_cleared() {
    if !database() {
        return;
    }
    let case = case().await;
    send(&case, 0, 3, b"%PDF-1.4\n").await.expect("part 0");
    send(&case, 1, 3, b"% middle\n").await.expect("part 1");
    // Right count, wrong positions: 0, 5, then 2.
    crate::store::execute_sql(
        "UPDATE buwiz_server.ownership_claim_parts SET part_index = 5 \
         WHERE claim_id = ?1::uuid AND part_index = 1",
        vec![json!(case.claim.claim_id)],
    )
    .await
    .expect("move part");
    let refused = send(&case, 2, 3, b"%%EOF\n").await;
    assert!(matches!(refused, Err(AuthStackError::Validation { .. })), "{refused:?}");
    assert_eq!(crate::store::ownership_claim_parts_received(&case.claim.claim_id).await.unwrap(), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn only_the_claimant_steps_their_claim() {
    if !database() {
        return;
    }
    let case = case().await;
    let stranger = user().await;
    let part = app::upload_ownership_claim_part(
        case.claim.claim_id.clone(),
        OwnershipClaimPartRequest {
            index: 0,
            total: 1,
            data: base64::engine::general_purpose::STANDARD.encode(PDF),
        },
        stranger.auth(),
    )
    .await;
    assert!(matches!(part, Err(AuthStackError::NotFound { .. })), "{part:?}");
    let withdraw = app::withdraw_ownership_claim(case.claim.claim_id.clone(), stranger.auth()).await;
    assert!(matches!(withdraw, Err(AuthStackError::NotFound { .. })), "{withdraw:?}");

    let mine = app::withdraw_ownership_claim(case.claim.claim_id.clone(), case.claimant.auth())
        .await
        .expect("withdraw");
    let claim = mine.claims.iter().find(|c| c.claim_id == case.claim.claim_id).expect("listed");
    assert_eq!(claim.status, "withdrawn");
    let again = app::withdraw_ownership_claim(case.claim.claim_id.clone(), case.claimant.auth()).await;
    assert!(matches!(again, Err(AuthStackError::Conflict { .. })), "{again:?}");
}

#[tokio::test(flavor = "current_thread")]
async fn an_owner_cannot_claim_their_own_profile() {
    if !database() {
        return;
    }
    let owner = user().await;
    let (_, tin_root) = profile(&owner, None).await;
    let refused = app::start_ownership_claim(
        OwnershipClaimStartRequest {
            tin_root,
            branch_code: "00000".to_owned(),
        },
        owner.auth(),
    )
    .await;
    assert!(matches!(refused, Err(AuthStackError::Validation { .. })), "{refused:?}");
}

/// The server functions behind the claims pages answer with the real lists.
#[tokio::test(flavor = "current_thread")]
async fn the_claim_server_functions_pass_the_caller_through() {
    use crate::app::server_fns as ui;
    if !database() {
        return;
    }
    let first = case().await;
    upload(&first, PDF).await.expect("upload");
    act_as(&first.claimant);
    let mine = ui::list_my_ownership_claims().await.expect("mine");
    assert!(mine.claims.iter().any(|c| c.claim_id == first.claim.claim_id));

    let staff = user().await;
    make_staff(&staff);
    act_as(&staff);
    let queue = ui::list_ownership_claim_queue().await.expect("queue");
    assert!(queue.claims.iter().any(|c| c.claim_id == first.claim.claim_id));
    let other = case_for_queue().await;
    let after = ui::decide_ownership_claim(first.claim.claim_id.clone(), false, Some("unreadable".into()))
        .await
        .expect("decide");
    assert!(after.claims.iter().any(|c| c.claim_id == other), "the rest of the queue comes back");
    assert!(after.claims.iter().all(|c| c.claim_id != first.claim.claim_id));

    let second = case().await;
    act_as(&second.claimant);
    let withdrawn = ui::withdraw_ownership_claim(second.claim.claim_id.clone()).await.expect("withdraw");
    let claim = withdrawn.claims.iter().find(|c| c.claim_id == second.claim.claim_id).expect("listed");
    assert_eq!(claim.status, "withdrawn");
}

/// Another pending claim, so the queue is never empty after a decision.
async fn case_for_queue() -> String {
    let case = case().await;
    upload(&case, PDF).await.expect("upload");
    case.claim.claim_id
}
