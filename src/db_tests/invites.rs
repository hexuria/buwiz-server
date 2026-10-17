//! Owner invites and firm publishers (`application::tax_profile`).

use serde_json::json;

use super::*;
use crate::application as app;
use crate::contracts::{FirmPublisherAddRequest, OwnerInviteCreateRequest, OwnerInviteCreated};

/// A firm (its manager and a plain member) holding a draft profile.
struct Firm {
    manager: User,
    member: User,
    org: String,
    profile_id: String,
}

async fn firm_with_draft() -> Firm {
    let manager = user().await;
    let org = firm(&manager, &[MANAGE]).await;
    let member = user().await;
    join(&org, &member, &[]).await;
    let (profile_id, _) = profile(&manager, Some(&org)).await;
    Firm {
        manager,
        member,
        org,
        profile_id,
    }
}

fn email_of(user: &User) -> String {
    format!("{}@test.invalid", user.user_id)
}

async fn invite(firm: &Firm, by: &User, email: &str, days: Option<u32>) -> AuthStackResult<OwnerInviteCreated> {
    app::invite_owner(
        firm.profile_id.clone(),
        OwnerInviteCreateRequest {
            email: email.to_owned(),
            days,
        },
        by.auth(),
    )
    .await
}

/// The secret from an invite link (`/invites/{id}#t={token}`).
fn token(created: &OwnerInviteCreated) -> String {
    created.link_path.split("#t=").nth(1).expect("token in link").to_owned()
}

#[tokio::test(flavor = "current_thread")]
async fn an_invite_lasts_one_to_fourteen_days() {
    if !database() {
        return;
    }
    let firm = firm_with_draft().await;
    for days in [0, 15] {
        let refused = invite(&firm, &firm.manager, "taxpayer@test.invalid", Some(days)).await;
        assert!(matches!(refused, Err(AuthStackError::Validation { .. })), "{days}: {refused:?}");
    }
    for days in [1, 14] {
        let created = invite(&firm, &firm.manager, &format!("d{days}@test.invalid"), Some(days))
            .await
            .unwrap_or_else(|e| panic!("{days} days: {e:?}"));
        assert_eq!(created.invite.status, "open");
        assert_eq!(created.invite.organization_id, firm.org);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn only_a_firm_manager_invites_or_withdraws() {
    if !database() {
        return;
    }
    let firm = firm_with_draft().await;
    let refused = invite(&firm, &firm.member, "taxpayer@test.invalid", None).await;
    assert!(refused.is_err(), "a member without organization.update cannot invite");
    let created = invite(&firm, &firm.manager, "taxpayer@test.invalid", None).await.expect("invite");

    let refused = app::withdraw_owner_invite(firm.profile_id.clone(), created.invite.invite_id.clone(), firm.member.auth()).await;
    assert!(refused.is_err(), "a member without organization.update cannot withdraw");
    app::withdraw_owner_invite(firm.profile_id.clone(), created.invite.invite_id.clone(), firm.manager.auth())
        .await
        .expect("withdraw");
    let preview = app::preview_owner_invite(created.invite.invite_id.clone(), token(&created), firm.manager.auth())
        .await
        .expect("preview");
    assert_eq!(preview.status, "withdrawn");
}

#[tokio::test(flavor = "current_thread")]
async fn the_link_secret_opens_the_invite_and_nothing_else_does() {
    if !database() {
        return;
    }
    let firm = firm_with_draft().await;
    let taxpayer = user().await;
    let created = invite(&firm, &firm.manager, &email_of(&taxpayer), None).await.expect("invite");
    let id = created.invite.invite_id.clone();

    let preview = app::preview_owner_invite(id.clone(), format!("  {}  ", token(&created)), taxpayer.auth())
        .await
        .expect("preview with the secret");
    assert_eq!(preview.status, "open");
    assert!(preview.email_matches);
    assert_eq!(preview.firm_name, "Test firm");

    let other = user().await;
    let theirs = app::preview_owner_invite(id.clone(), token(&created), other.auth()).await.expect("preview");
    assert!(!theirs.email_matches, "someone else's email does not match");

    for wrong in [String::new(), "   ".to_owned(), "not-the-secret".to_owned()] {
        let refused = app::preview_owner_invite(id.clone(), wrong.clone(), taxpayer.auth()).await;
        assert!(matches!(refused, Err(AuthStackError::NotFound { .. })), "{wrong:?}: {refused:?}");
    }
    let unknown = app::preview_owner_invite(crate::store::new_uuid_v4().unwrap(), token(&created), taxpayer.auth()).await;
    assert!(matches!(unknown, Err(AuthStackError::NotFound { .. })), "{unknown:?}");

    let view = app::accept_owner_invite(id, token(&created), taxpayer.auth()).await.expect("accept");
    assert_eq!(view.owner_user_id.as_deref(), Some(taxpayer.user_id.as_str()));
}

#[tokio::test(flavor = "current_thread")]
async fn an_open_invite_past_its_date_shows_as_expired() {
    if !database() {
        return;
    }
    let firm = firm_with_draft().await;
    let taxpayer = user().await;
    let created = invite(&firm, &firm.manager, &email_of(&taxpayer), Some(1)).await.expect("invite");
    // Move the stored expiry into the past.
    crate::store::execute_sql(
        "UPDATE buwiz_server.tax_profile_events \
         SET payload = replace(payload::text, ?2, '2000-01-01T00:00:00.000Z')::jsonb \
         WHERE stream_id = ?1",
        vec![json!(firm.profile_id), json!(created.invite.expires_at)],
    )
    .await
    .expect("age the invite");
    let preview = app::preview_owner_invite(created.invite.invite_id.clone(), token(&created), taxpayer.auth())
        .await
        .expect("preview");
    assert_eq!(preview.status, "expired");
}

#[tokio::test(flavor = "current_thread")]
async fn a_firm_manager_picks_publishers_from_its_members() {
    if !database() {
        return;
    }
    let firm = firm_with_draft().await;
    let add = |by: &User, email: String| {
        app::add_firm_publisher(firm.org.clone(), FirmPublisherAddRequest { email }, by.auth())
    };
    let refused = add(&firm.member, email_of(&firm.member)).await;
    assert!(matches!(refused, Err(AuthStackError::Forbidden)), "{refused:?}");
    let outsider = user().await;
    let refused = add(&firm.manager, email_of(&outsider)).await;
    assert!(matches!(refused, Err(AuthStackError::NotFound { .. })), "{refused:?}");

    let listed = add(&firm.manager, email_of(&firm.member).to_uppercase()).await.expect("add");
    assert_eq!(listed.publishers.len(), 1);
    assert_eq!(listed.publishers[0].user_id, firm.member.user_id);
    let seen = app::list_firm_publishers(firm.org.clone(), firm.member.auth()).await.expect("members may look");
    assert_eq!(seen.publishers, listed.publishers);
    let refused = app::list_firm_publishers(firm.org.clone(), outsider.auth()).await;
    assert!(matches!(refused, Err(AuthStackError::Forbidden)), "{refused:?}");

    let after = app::remove_firm_publisher(firm.org.clone(), firm.member.user_id.clone(), firm.manager.auth())
        .await
        .expect("remove");
    assert!(after.publishers.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn records_are_not_found_for_strangers_or_unknown_profiles() {
    if !database() {
        return;
    }
    let owner = user().await;
    let (id, _) = profile(&owner, None).await;
    let stranger = user().await;
    let refused = app::list_tax_years(id.clone(), stranger.auth()).await;
    assert!(matches!(refused, Err(AuthStackError::NotFound { .. })), "{refused:?}");
    let unknown = app::list_tax_years(crate::store::new_uuid_v4().unwrap(), owner.auth()).await;
    assert!(matches!(unknown, Err(AuthStackError::NotFound { .. })), "{unknown:?}");
    app::list_tax_years(id, owner.auth()).await.expect("the owner reads");
}

/// The publisher server functions answer with the firm's real list.
#[tokio::test(flavor = "current_thread")]
async fn the_publisher_server_functions_pass_the_caller_through() {
    use crate::app::server_fns as ui;
    if !database() {
        return;
    }
    let firm = firm_with_draft().await;
    act_as(&firm.manager);
    let added = ui::change_firm_publisher(firm.org.clone(), Some(email_of(&firm.member)), None, false)
        .await
        .expect("add");
    assert_eq!(added.publishers.len(), 1);
    act_as(&firm.member);
    let listed = ui::list_firm_publishers(firm.org.clone()).await.expect("list");
    assert_eq!(listed.publishers, added.publishers);
}
