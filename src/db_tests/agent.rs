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
                "http://127.0.0.1:9/mcp",
            )
            .await
        }
    };
    assert!(connect(vec![theirs]).await.is_err(), "someone else's profile");
    connect(vec![mine]).await.expect("own profile");
}

fn validation_message(error: crate::error::AuthStackError) -> String {
    assert!(matches!(error, crate::error::AuthStackError::Validation { .. }), "{error:?}");
    error.to_string()
}

#[tokio::test(flavor = "current_thread")]
async fn an_agent_queues_a_filing_only_when_allowed_and_for_the_profiles_tin() {
    if !database() {
        return;
    }
    let owner = user().await;
    let (profile_id, tin_root) = profile(&owner, None).await;
    let agent = connected_agent(&owner, &profile_id).await;
    let fields = serde_json::json!({"line_1": "100.00"});
    let queue = |tin: String| {
        let agent = agent.clone();
        let profile_id = profile_id.clone();
        let fields = fields.clone();
        async move { app::agent_queue_on_device(&agent, &profile_id, &tin, "1601C", 2026, 10, &fields).await }
    };

    let refused = validation_message(queue(tin_root.clone()).await.unwrap_err());
    assert!(refused.contains("has not allowed this agent"), "{refused}");

    let listed = app::set_agent_may_queue(agent.connection_id.clone(), true, owner.auth())
        .await
        .expect("allow");
    assert!(listed.connections.iter().any(|c| c.connection_id == agent.connection_id && c.may_queue));

    let other = crate::store::AgentActor { profile_ids: vec!["someone-else".to_owned()], ..agent.clone() };
    assert!(matches!(
        app::agent_queue_on_device(&other, &profile_id, &tin_root, "1601C", 2026, 10, &fields).await,
        Err(crate::error::AuthStackError::Forbidden)
    ));

    let last4 = &tin_root[tin_root.len() - 4..];
    let mismatched = format!("{}{}", &tin_root[..5], if last4 == "0000" { "1111" } else { "0000" });
    for tin in [
        mismatched,
        format!("1234{last4}"),           // 8 digits
        format!("12345678901{last4}"),    // 15 digits
    ] {
        let message = validation_message(queue(tin.clone()).await.unwrap_err());
        assert!(message.contains("tin does not match"), "{tin}: {message}");
    }
    queue(format!("12345{last4}")).await.expect("9 digits");
    let fourteen = format!("1234-5678-90{last4}");
    let queued = queue(fourteen).await.expect("14 digits with dashes");
    let filing_id = queued["id"].as_str().expect("id").to_owned();
    assert_eq!(queued["status"], "queued");

    let waiting = app::list_device_filings(owner.auth()).await.expect("list");
    let filing = waiting["filings"]
        .as_array()
        .expect("filings")
        .iter()
        .find(|f| f["id"] == filing_id.as_str())
        .expect("queued filing")
        .clone();
    assert_eq!(filing["tin"], format!("1234567890{last4}"));
    assert_eq!(filing["form_code"], "1601C");
    assert_eq!(filing["tax_year"], 2026);
    assert_eq!(filing["period"], 10);
    assert_eq!(filing["fields"], fields);

    let report = |status: &str| crate::contracts::DeviceFilingReportRequest {
        status: status.to_owned(),
        error_summary: "Line 1 is required".to_owned(),
        field_errors: vec![serde_json::json!({"field": "line_1"})],
    };
    let bad = app::report_device_filing(filing_id.clone(), report("sent"), owner.auth()).await;
    assert!(validation_message(bad.unwrap_err()).contains("picked, invalid, or failed"));
    let done = app::report_device_filing(filing_id.clone(), report(" invalid "), owner.auth())
        .await
        .expect("report");
    assert_eq!(done["status"], "invalid");

    let seen = app::agent_device_filing_status(&agent, &filing_id).await.expect("status");
    assert_eq!(seen["status"], "invalid");
    assert_eq!(seen["message"], "Line 1 is required");
    assert_eq!(seen["tax_year"], 2026);
    assert_eq!(seen["period"], 10);
    assert_eq!(seen["field_errors"], serde_json::json!([{"field": "line_1"}]));
}

/// The server functions behind Connected agents and the pass list answer
/// with the real lists.
#[tokio::test(flavor = "current_thread")]
async fn the_agent_and_pass_server_functions_pass_the_caller_through() {
    use crate::app::server_fns as ui;
    if !database() {
        return;
    }
    let owner = user().await;
    let (profile_id, _) = profile(&owner, None).await;
    let agent = connected_agent(&owner, &profile_id).await;
    act_as(&owner);
    let listed = ui::set_agent_may_queue(agent.connection_id.clone(), true).await.expect("allow");
    let connection = listed
        .connections
        .iter()
        .find(|c| c.connection_id == agent.connection_id)
        .expect("listed");
    assert!(connection.may_queue);

    let manager = user().await;
    let org = firm(&manager, &[MANAGE]).await;
    let pass_id = working_pass(&owner, &manager, &org, &profile_id).await;
    act_as(&owner);
    let passes = ui::pass_step(profile_id.clone(), pass_id.clone(), "end".to_owned())
        .await
        .expect("end");
    assert!(
        passes.passes.iter().all(|p| p.pass_id != pass_id || p.status == "revoked"),
        "{passes:?}"
    );
    assert!(connectable(&manager).await.is_empty(), "an ended pass reaches nothing");
}

#[tokio::test(flavor = "current_thread")]
async fn each_report_status_is_accepted() {
    if !database() {
        return;
    }
    let owner = user().await;
    let (profile_id, tin_root) = profile(&owner, None).await;
    let agent = connected_agent(&owner, &profile_id).await;
    app::set_agent_may_queue(agent.connection_id.clone(), true, owner.auth()).await.expect("allow");
    for (period, status) in [(1, "picked"), (2, "invalid"), (3, "failed")] {
        let queued = app::agent_queue_on_device(&agent, &profile_id, &tin_root, "2551Q", 2026, period, &serde_json::json!({}))
            .await
            .expect("queue");
        let id = queued["id"].as_str().expect("id").to_owned();
        let report = crate::contracts::DeviceFilingReportRequest {
            status: status.to_owned(),
            ..Default::default()
        };
        let done = app::report_device_filing(id, report, owner.auth()).await.expect(status);
        assert_eq!(done["status"], status);
    }
}
