//! Tax years, drafts and filings (`application::records`, `store::records`).

use serde_json::json;

use super::*;
use crate::application as app;
use crate::contracts::{
    DraftSaveRequest, FilingQueueRequest, FilingReportRequest, TaxYearCloneRequest,
    TaxYearPutRequest, YearForm,
};

async fn profile(owner: &User) -> String {
    super::profile(owner, None).await.0
}

#[tokio::test(flavor = "current_thread")]
async fn owner_round_trips_a_year() {
    if !database() {
        return;
    }
    let owner = user().await;
    let id = profile(&owner).await;
    let years = app::put_tax_year(
        id.clone(),
        2026,
        TaxYearPutRequest {
            overrides: Default::default(),
            forms: Some(vec![YearForm {
                form_code: "1701q".to_owned(),
                frequency: "quarterly".to_owned(),
                active: true,
            }]),
        },
        owner.auth(),
    )
    .await
    .expect("put year");
    assert_eq!(years.years.len(), 1);
    assert_eq!(years.years[0].forms[0].form_code, "1701Q");
}

fn form(code: &str) -> YearForm {
    YearForm {
        form_code: code.to_owned(),
        frequency: "quarterly".to_owned(),
        active: true,
    }
}

fn year_with(forms: Vec<YearForm>) -> TaxYearPutRequest {
    TaxYearPutRequest {
        overrides: Default::default(),
        forms: Some(forms),
    }
}

fn draft(form_code: &str, period_key: &str, base_rev: Option<u64>, payload: serde_json::Value) -> DraftSaveRequest {
    DraftSaveRequest {
        form_code: form_code.to_owned(),
        tax_year: 2026,
        period_key: period_key.to_owned(),
        payload,
        base_rev,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_year_takes_one_hundred_forms_and_no_more() {
    if !database() {
        return;
    }
    let owner = user().await;
    let id = profile(&owner).await;
    let forms = |count: usize| (0..count).map(|n| form(&format!("F{n:03}"))).collect::<Vec<_>>();
    let years = app::put_tax_year(id.clone(), 2026, year_with(forms(100)), owner.auth())
        .await
        .expect("100 forms fit");
    assert_eq!(years.years[0].forms.len(), 100);
    let refused = app::put_tax_year(id.clone(), 2026, year_with(forms(101)), owner.auth()).await;
    assert!(matches!(refused, Err(AuthStackError::Validation { .. })), "{refused:?}");
    let years = app::list_tax_years(id, owner.auth()).await.expect("list");
    assert_eq!(years.years[0].forms.len(), 100, "a refused put changes nothing");
}

#[tokio::test(flavor = "current_thread")]
async fn a_form_listed_twice_is_refused() {
    if !database() {
        return;
    }
    let owner = user().await;
    let id = profile(&owner).await;
    // The same code in another case is still the same form.
    let refused = app::put_tax_year(id.clone(), 2026, year_with(vec![form("1701Q"), form("1701q")]), owner.auth()).await;
    assert!(matches!(refused, Err(AuthStackError::Validation { .. })), "{refused:?}");
    let years = app::put_tax_year(id, 2026, year_with(vec![form("1701Q"), form("2551Q")]), owner.auth())
        .await
        .expect("distinct forms");
    let codes: Vec<_> = years.years[0].forms.iter().map(|f| f.form_code.as_str()).collect();
    assert_eq!(codes.len(), 2);
    assert!(codes.contains(&"1701Q") && codes.contains(&"2551Q"));
}

#[tokio::test(flavor = "current_thread")]
async fn cloning_copies_forms_once_and_never_from_the_same_year() {
    if !database() {
        return;
    }
    let owner = user().await;
    let id = profile(&owner).await;
    app::put_tax_year(id.clone(), 2025, year_with(vec![form("1701Q")]), owner.auth())
        .await
        .expect("2025");
    let same = app::clone_tax_year(id.clone(), 2025, TaxYearCloneRequest { from_year: 2025 }, owner.auth()).await;
    assert!(matches!(same, Err(AuthStackError::Validation { .. })), "{same:?}");

    let years = app::clone_tax_year(id.clone(), 2026, TaxYearCloneRequest { from_year: 2025 }, owner.auth())
        .await
        .expect("clone");
    let y2026 = years.years.iter().find(|y| y.tax_year == 2026).expect("2026 created");
    assert_eq!(y2026.forms.len(), 1);
    assert_eq!(y2026.forms[0].form_code, "1701Q");

    // The store reports whether it copied: not again once 2026 has forms.
    let new_id = crate::store::new_uuid_v4().unwrap();
    assert!(!crate::store::clone_tax_year(&new_id, &id, 2025, 2026).await.unwrap());
    let fresh = crate::store::new_uuid_v4().unwrap();
    assert!(crate::store::clone_tax_year(&fresh, &id, 2025, 2027).await.unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn saving_returns_the_draft_it_saved_with_its_payload_and_revision() {
    if !database() {
        return;
    }
    let owner = user().await;
    let id = profile(&owner).await;
    let first = app::save_draft(id.clone(), draft("1701Q", "2026-Q1", None, json!({"line": 1})), owner.auth())
        .await
        .expect("q1");
    // A second draft of the same year, other form and period: the save must
    // still answer with the one it saved, not the first one listed.
    let other = app::save_draft(id.clone(), draft("2551Q", "2026-Q2", None, json!({"line": 2})), owner.auth())
        .await
        .expect("q2");
    assert_eq!(other.summary.form_code, "2551Q");
    assert_eq!(other.summary.period_key, "2026-Q2");
    assert_eq!(other.payload, json!({"line": 2}));
    let same_form = app::save_draft(id.clone(), draft("1701Q", "2026-Q2", None, json!({"line": 3})), owner.auth())
        .await
        .expect("1701Q q2");
    assert_eq!((same_form.summary.form_code.as_str(), same_form.summary.period_key.as_str()), ("1701Q", "2026-Q2"));

    assert_eq!(first.summary.rev, 1);
    assert_eq!(first.payload, json!({"line": 1}));
    let second = app::save_draft(id.clone(), draft("1701Q", "2026-Q1", Some(1), json!({"line": 9})), owner.auth())
        .await
        .expect("update");
    assert_eq!(second.summary.draft_id, first.summary.draft_id);
    assert_eq!(second.summary.rev, 2);
    assert_eq!(second.payload, json!({"line": 9}));
    let stale = app::save_draft(id.clone(), draft("1701Q", "2026-Q1", Some(1), json!({})), owner.auth()).await;
    assert!(matches!(stale, Err(AuthStackError::Conflict { .. })), "{stale:?}");

    let fetched = app::get_draft(id, first.summary.draft_id, owner.auth()).await.expect("get");
    assert_eq!(fetched.summary.rev, 2);
    assert_eq!(fetched.payload, json!({"line": 9}));
}

#[tokio::test(flavor = "current_thread")]
async fn reviewing_marks_the_draft_and_an_unknown_draft_is_not_found() {
    if !database() {
        return;
    }
    let owner = user().await;
    let id = profile(&owner).await;
    let saved = app::save_draft(id.clone(), draft("1701Q", "2026-Q1", None, json!({})), owner.auth())
        .await
        .expect("save");
    assert!(saved.summary.reviewed_at.is_none());
    let reviewed = app::review_draft(id.clone(), saved.summary.draft_id.clone(), owner.auth())
        .await
        .expect("review");
    assert!(reviewed.summary.reviewed_at.is_some());
    assert_eq!(reviewed.summary.reviewed_by.as_deref(), Some(owner.user_id.as_str()));

    let missing = app::review_draft(id.clone(), crate::store::new_uuid_v4().unwrap(), owner.auth()).await;
    assert!(matches!(missing, Err(AuthStackError::NotFound { .. })), "{missing:?}");
    let other_owner = user().await;
    let other = profile(&other_owner).await;
    assert!(
        !crate::store::mark_draft_reviewed(&other, &saved.summary.draft_id, &owner.user_id)
            .await
            .unwrap(),
        "a draft id from another profile is not reviewed"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_filing_needs_its_form_active_that_year_then_walks_its_steps() {
    if !database() {
        return;
    }
    let owner = user().await;
    let id = profile(&owner).await;
    let saved = app::save_draft(id.clone(), draft("1701Q", "2026-Q1", None, json!({})), owner.auth())
        .await
        .expect("save");
    let queue = || FilingQueueRequest {
        draft_id: saved.summary.draft_id.clone(),
    };
    let inactive = app::queue_filing(id.clone(), queue(), owner.auth()).await;
    assert!(matches!(inactive, Err(AuthStackError::Validation { .. })), "{inactive:?}");
    let mut off = form("1701Q");
    off.active = false;
    app::put_tax_year(id.clone(), 2026, year_with(vec![off]), owner.auth()).await.expect("inactive form");
    let inactive = app::queue_filing(id.clone(), queue(), owner.auth()).await;
    assert!(matches!(inactive, Err(AuthStackError::Validation { .. })), "{inactive:?}");

    app::put_tax_year(id.clone(), 2026, year_with(vec![form("1701Q")]), owner.auth()).await.expect("active form");
    let filing = app::queue_filing(id.clone(), queue(), owner.auth()).await.expect("queue");
    assert_eq!(filing.status, "queued");
    assert_eq!(filing.draft_id.as_deref(), Some(saved.summary.draft_id.as_str()));
    assert_eq!((filing.form_code.as_str(), filing.tax_year, filing.period_key.as_str()), ("1701Q", 2026, "2026-Q1"));

    let sha = "a".repeat(64);
    let report = |step: &str, request: FilingReportRequest| {
        app::report_filing(id.clone(), filing.filing_id.clone(), step.to_owned(), request, owner.auth())
    };
    let submitted = report("submitted", FilingReportRequest { xml_sha256: Some(sha.clone()), ..Default::default() })
        .await
        .expect("submitted");
    assert_eq!(submitted.status, "submitted");
    assert_eq!(submitted.xml_sha256.as_deref(), Some(sha.as_str()));
    assert!(submitted.submitted_at.is_some());
    let confirmed = report("confirmed", FilingReportRequest { receipt_ref: Some("R-1".into()), ..Default::default() })
        .await
        .expect("confirmed");
    assert_eq!(confirmed.receipt_ref.as_deref(), Some("R-1"));
    let paid = report("paid", FilingReportRequest::default()).await.expect("paid");
    assert_eq!(paid.status, "paid");
    assert!(paid.paid_at.is_some());
    let again = report("paid", FilingReportRequest::default()).await;
    assert!(matches!(again, Err(AuthStackError::Conflict { .. })), "{again:?}");

    let listed = app::list_filings(id, owner.auth()).await.expect("list");
    assert_eq!(listed.filings.len(), 1);
    assert_eq!(listed.filings[0].status, "paid");
}
