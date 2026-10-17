//! Tax years, drafts and filings of a tax profile (`docs/sync-api.md`).
//!
//! Every call resolves the caller's standing through the tax profile's
//! gatekeeper and checks the one permission the step needs
//! (`domain::records`). An archived profile is read-only.

use crate::contracts::{
    DraftListResponse, DraftSaveRequest, DraftView, FilingListResponse, FilingQueueRequest,
    FilingReportRequest, FilingView, TaxYearCloneRequest, TaxYearListResponse, TaxYearPutRequest,
};
use crate::domain::records::{self, FilingStatus, FilingStep, RecordError};
use crate::domain::{PassPermission, ProfileAccess};
use crate::error::{AuthStackError, AuthStackResult};

use super::{agent_require, record_access, RequestAuth};
use crate::store::AgentActor;

fn map(error: RecordError) -> AuthStackError {
    match error {
        RecordError::NotAllowed { .. } => AuthStackError::Forbidden,
        RecordError::Invalid { reason } => AuthStackError::validation(reason),
        RecordError::StaleDraft { .. } | RecordError::FilingState { .. } => {
            AuthStackError::conflict(error.to_string())
        }
    }
}

fn need(access: ProfileAccess, permission: PassPermission) -> AuthStackResult<()> {
    records::require(access, permission).map_err(map)
}

/// The standing, refusing writes on an archived profile.
async fn writable(profile_id: &str, auth: RequestAuth) -> AuthStackResult<(String, ProfileAccess)> {
    let (user_id, access, archived) = record_access(profile_id, auth).await?;
    if archived {
        return Err(AuthStackError::conflict("this profile is archived; restore it first"));
    }
    Ok((user_id, access))
}

// ── Tax years ───────────────────────────────────────────────────────────────

pub async fn list_tax_years(profile_id: String, auth: RequestAuth) -> AuthStackResult<TaxYearListResponse> {
    let (_, access, _) = record_access(&profile_id, auth).await?;
    need(access, PassPermission::YearsView)?;
    Ok(TaxYearListResponse {
        years: crate::store::list_tax_years(&profile_id).await?,
    })
}

pub async fn put_tax_year(
    profile_id: String,
    tax_year: i32,
    mut request: TaxYearPutRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxYearListResponse> {
    let (_, access) = writable(&profile_id, auth).await?;
    need(access, PassPermission::YearsManage)?;
    let year = records::tax_year(tax_year).map_err(map)?;
    if let Some(forms) = request.forms.as_mut() {
        if forms.len() > 100 {
            return Err(AuthStackError::validation("a year has at most 100 forms"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for form in forms.iter_mut() {
            form.form_code = records::form_code(&form.form_code).map_err(map)?;
            form.frequency = records::frequency(&form.frequency).map_err(map)?;
            if !seen.insert(form.form_code.clone()) {
                return Err(AuthStackError::validation(format!(
                    "{} is listed twice",
                    form.form_code
                )));
            }
        }
    }
    crate::store::put_tax_year(
        &crate::store::new_uuid_v4()?,
        &profile_id,
        year,
        &request.overrides,
        request.forms.as_deref(),
    )
    .await?;
    Ok(TaxYearListResponse {
        years: crate::store::list_tax_years(&profile_id).await?,
    })
}

pub async fn clone_tax_year(
    profile_id: String,
    tax_year: i32,
    request: TaxYearCloneRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxYearListResponse> {
    let (_, access) = writable(&profile_id, auth).await?;
    need(access, PassPermission::YearsManage)?;
    let to_year = records::tax_year(tax_year).map_err(map)?;
    let from_year = records::tax_year(request.from_year).map_err(map)?;
    if from_year == to_year {
        return Err(AuthStackError::validation("pick a different year to copy from"));
    }
    crate::store::clone_tax_year(&crate::store::new_uuid_v4()?, &profile_id, from_year, to_year).await?;
    Ok(TaxYearListResponse {
        years: crate::store::list_tax_years(&profile_id).await?,
    })
}

// ── Drafts ──────────────────────────────────────────────────────────────────

pub async fn list_drafts(
    profile_id: String,
    tax_year: Option<i32>,
    auth: RequestAuth,
) -> AuthStackResult<DraftListResponse> {
    let (_, access, _) = record_access(&profile_id, auth).await?;
    need(access, PassPermission::DraftsView)?;
    let year = tax_year.map(records::tax_year).transpose().map_err(map)?;
    Ok(DraftListResponse {
        drafts: crate::store::list_drafts(&profile_id, year).await?,
    })
}

pub async fn get_draft(profile_id: String, draft_id: String, auth: RequestAuth) -> AuthStackResult<DraftView> {
    let (_, access, _) = record_access(&profile_id, auth).await?;
    need(access, PassPermission::DraftsView)?;
    crate::store::get_draft(&profile_id, &draft_id)
        .await?
        .ok_or_else(|| AuthStackError::not_found("draft not found"))
}

/// Saves a draft: `drafts.save` for a new one, `drafts.update` to change
/// one, at the revision the caller started from.
pub async fn save_draft(
    profile_id: String,
    request: DraftSaveRequest,
    auth: RequestAuth,
) -> AuthStackResult<DraftView> {
    let (user_id, access) = writable(&profile_id, auth).await?;
    let form_code = records::form_code(&request.form_code).map_err(map)?;
    let year = records::tax_year(request.tax_year).map_err(map)?;
    let period_key = records::period_key(&request.period_key, year).map_err(map)?;
    let existing = crate::store::draft_rev(&profile_id, &form_code, year, &period_key).await?;
    need(access, records::save_permission(existing.is_some()))?;
    records::draft_payload(&request.payload).map_err(map)?;
    let new_rev = records::next_draft_rev(existing, request.base_rev).map_err(map)?;
    crate::store::save_draft(
        &crate::store::new_uuid_v4()?,
        &profile_id,
        &form_code,
        year,
        &period_key,
        &request.payload,
        existing,
        new_rev,
        &user_id,
    )
    .await?;
    let drafts = crate::store::list_drafts(&profile_id, Some(year)).await?;
    let saved = drafts
        .into_iter()
        .find(|draft| draft.form_code == form_code && draft.period_key == period_key)
        .ok_or_else(|| AuthStackError::store("draft missing after save"))?;
    crate::store::get_draft(&profile_id, &saved.draft_id)
        .await?
        .ok_or_else(|| AuthStackError::store("draft missing after save"))
}

pub async fn review_draft(profile_id: String, draft_id: String, auth: RequestAuth) -> AuthStackResult<DraftView> {
    let (user_id, access) = writable(&profile_id, auth).await?;
    need(access, PassPermission::DraftsReview)?;
    if !crate::store::mark_draft_reviewed(&profile_id, &draft_id, &user_id).await? {
        return Err(AuthStackError::not_found("draft not found"));
    }
    crate::store::get_draft(&profile_id, &draft_id)
        .await?
        .ok_or_else(|| AuthStackError::not_found("draft not found"))
}

// ── Filings ─────────────────────────────────────────────────────────────────

pub async fn list_filings(profile_id: String, auth: RequestAuth) -> AuthStackResult<FilingListResponse> {
    let (_, access, _) = record_access(&profile_id, auth).await?;
    need(access, PassPermission::FilingsView)?;
    Ok(FilingListResponse {
        filings: crate::store::list_filings(&profile_id).await?,
    })
}

/// Queues a draft for the caller's installed app to send. Needs
/// `filings.publish` (for a firm member, the owner's switch and the firm's
/// pick) and the form active in that year.
pub async fn queue_filing(
    profile_id: String,
    request: FilingQueueRequest,
    auth: RequestAuth,
) -> AuthStackResult<FilingView> {
    let (user_id, access) = writable(&profile_id, auth).await?;
    need(access, PassPermission::FilingsPublish)?;
    let draft = crate::store::get_draft(&profile_id, &request.draft_id)
        .await?
        .ok_or_else(|| AuthStackError::not_found("draft not found"))?;
    let year = records::tax_year(draft.summary.tax_year).map_err(map)?;
    if !crate::store::year_form_active(&profile_id, year, &draft.summary.form_code).await? {
        return Err(AuthStackError::validation(format!(
            "{} is not an active form for {}; add it to the year first",
            draft.summary.form_code, draft.summary.tax_year
        )));
    }
    let filing_id = crate::store::new_uuid_v4()?;
    crate::store::queue_filing(&filing_id, &profile_id, &request.draft_id, &user_id).await?;
    crate::store::get_filing(&profile_id, &filing_id)
        .await?
        .ok_or_else(|| AuthStackError::store("filing missing after queue"))
}

/// The app reports a step: `submitted`, `confirmed`, `failed` or `paid`.
pub async fn report_filing(
    profile_id: String,
    filing_id: String,
    step: String,
    request: FilingReportRequest,
    auth: RequestAuth,
) -> AuthStackResult<FilingView> {
    let (user_id, access) = writable(&profile_id, auth).await?;
    need(access, PassPermission::FilingsPublish)?;
    let step = FilingStep::parse(&step).ok_or_else(|| AuthStackError::not_found("unknown filing step"))?;
    let filing = crate::store::get_filing(&profile_id, &filing_id)
        .await?
        .ok_or_else(|| AuthStackError::not_found("filing not found"))?;
    let current = FilingStatus::parse(&filing.status)
        .ok_or_else(|| AuthStackError::store("unknown filing status"))?;
    let next = records::next_filing_status(current, step).map_err(map)?;
    let (mut xml, mut receipt, mut error) = (None, None, None);
    match step {
        FilingStep::Submitted => {
            xml = Some(records::sha256_hex(request.xml_sha256.as_deref().unwrap_or_default()).map_err(map)?);
        }
        FilingStep::Confirmed => {
            receipt = Some(
                records::short_text(request.receipt_ref.as_deref().unwrap_or_default(), "receipt_ref", 200)
                    .map_err(map)?,
            );
        }
        FilingStep::Failed => {
            error = Some(
                records::short_text(request.error_summary.as_deref().unwrap_or_default(), "error_summary", 1000)
                    .map_err(map)?,
            );
        }
        FilingStep::Paid => {}
    }
    crate::store::advance_filing(
        &profile_id,
        &filing_id,
        current,
        next,
        xml.as_deref(),
        receipt.as_deref(),
        error.as_deref(),
        &user_id,
    )
    .await?;
    crate::store::get_filing(&profile_id, &filing_id)
        .await?
        .ok_or_else(|| AuthStackError::store("filing missing after update"))
}

// ── AI agents (read-only, through MCP) ──────────────────────────────────────
//
// An agent reaches only profiles its taxpayer owns and connected, with the
// connection's permissions narrowed by the taxpayer's live access
// (`agent_require`). It never gets `filings.publish`, and these are its only
// record calls: nothing here writes.

pub(crate) async fn agent_list_tax_years(
    agent: &AgentActor,
    profile_id: &str,
) -> AuthStackResult<TaxYearListResponse> {
    agent_require(agent, profile_id, PassPermission::YearsView).await?;
    Ok(TaxYearListResponse {
        years: crate::store::list_tax_years(profile_id).await?,
    })
}

pub(crate) async fn agent_list_drafts(
    agent: &AgentActor,
    profile_id: &str,
    tax_year: Option<i32>,
) -> AuthStackResult<DraftListResponse> {
    let year = tax_year.map(records::tax_year).transpose().map_err(map)?;
    agent_require(agent, profile_id, PassPermission::DraftsView).await?;
    Ok(DraftListResponse {
        drafts: crate::store::list_drafts(profile_id, year).await?,
    })
}

pub(crate) async fn agent_get_draft(
    agent: &AgentActor,
    profile_id: &str,
    draft_id: &str,
) -> AuthStackResult<DraftView> {
    agent_require(agent, profile_id, PassPermission::DraftsView).await?;
    crate::store::get_draft(profile_id, draft_id)
        .await?
        .ok_or_else(|| AuthStackError::not_found("draft not found"))
}

pub(crate) async fn agent_list_filings(
    agent: &AgentActor,
    profile_id: &str,
) -> AuthStackResult<FilingListResponse> {
    agent_require(agent, profile_id, PassPermission::FilingsView).await?;
    Ok(FilingListResponse {
        filings: crate::store::list_filings(profile_id).await?,
    })
}
