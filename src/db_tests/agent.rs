//! Which profiles a user may connect an AI agent to (`store::agent`).

use super::*;
use crate::application as app;
use crate::contracts::{PassChoice, PassOfferRequest};
use crate::domain::PassPermissions;

/// Owner offers `firm` a Safe pass on `profile_id`; the firm's manager accepts.
async fn working_pass(owner: &User, manager: &User, firm: &str, profile_id: &str) -> String {
    let offered = app::offer_pass(
        profile_id.to_owned(),
        PassOfferRequest {
            organization_slug: None,
            organization_id: Some(firm.to_owned()),
            choice: PassChoice {
                preset: Some("safe".to_owned()),
                ..PassChoice::default()
            },
        },
        owner.auth(),
    )
    .await
    .expect("offer");
    let pass_id = offered
        .passes
        .iter()
        .find(|p| p.profile_id == profile_id && p.grantee_id == firm)
        .expect("offered pass")
        .pass_id
        .clone();
    app::accept_pass(profile_id.to_owned(), pass_id.clone(), manager.auth())
        .await
        .expect("accept");
    pass_id
}

async fn connectable(user: &User) -> Vec<(String, Option<String>)> {
    crate::store::list_connectable_profiles(&user.user_id)
        .await
        .expect("connectable")
        .into_iter()
        .map(|p| (p.profile_id, p.via))
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn owners_reach_their_own_and_members_reach_through_a_working_pass() {
    if !database() {
        return;
    }
    let owner = user().await;
    let (mine, _) = profile(&owner, None).await;
    let (other, _) = profile(&owner, None).await;
    let manager = user().await;
    let org = firm(&manager, &[MANAGE]).await;
    let member = user().await;
    join(&org, &member, &[]).await;
    working_pass(&owner, &manager, &org, &mine).await;

    let owned = connectable(&owner).await;
    assert_eq!(owned.len(), 2);
    assert!(owned.iter().all(|(_, via)| via.is_none()), "own profiles come with no firm");
    assert_eq!(connectable(&member).await, vec![(mine.clone(), Some("Test firm".to_owned()))]);
    assert!(!connectable(&member).await.iter().any(|(id, _)| id == &other));

    let outsider = user().await;
    assert!(connectable(&outsider).await.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn an_ended_pass_or_an_archived_profile_reaches_nothing() {
    if !database() {
        return;
    }
    let owner = user().await;
    let (id, _) = profile(&owner, None).await;
    let manager = user().await;
    let org = firm(&manager, &[MANAGE]).await;
    let pass_id = working_pass(&owner, &manager, &org, &id).await;
    assert_eq!(connectable(&manager).await.len(), 1);
    app::end_pass(id.clone(), pass_id, owner.auth()).await.expect("end");
    assert!(connectable(&manager).await.is_empty());

    app::archive_tax_profile(id.clone(), owner.auth()).await.expect("archive");
    assert!(connectable(&owner).await.is_empty(), "archived profiles are never connectable");
}

#[tokio::test(flavor = "current_thread")]
async fn a_connection_refuses_a_profile_the_user_cannot_hand_on() {
    if !database() {
        return;
    }
    let owner = user().await;
    let (mine, _) = profile(&owner, None).await;
    let stranger = user().await;
    let (theirs, _) = profile(&stranger, None).await;
    let client_id = format!("test-client-{}", crate::store::new_uuid_v4().unwrap());
    crate::store::insert_agent_client(&client_id, "Test agent", &["http://127.0.0.1:9/cb".to_owned()])
        .await
        .expect("client");
    let connect = |profiles: Vec<String>| {
        let client_id = client_id.clone();
        let owner = owner.clone();
        async move {
            crate::store::create_agent_connection(
                &crate::store::new_uuid_v4().unwrap(),
                &owner.user_id,
                &client_id,
                PassPermissions::empty(),
                &profiles,
                &crate::store::new_uuid_v4().unwrap(),
                "http://127.0.0.1:9/cb",
                "challenge",
                60,
            )
            .await
        }
    };
    assert!(connect(vec![theirs]).await.is_err(), "someone else's profile");
    connect(vec![mine]).await.expect("own profile");
}
