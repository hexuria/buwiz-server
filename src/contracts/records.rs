//! Tax years, drafts and filings of one tax profile (`docs/sync-api.md`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Per-year values that differ from the profile's (null inherits).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxYearOverrides {
    #[serde(default)]
    pub registered_name: Option<String>,
    #[serde(default)]
    pub rdo_code: Option<String>,
    #[serde(default)]
    pub line_of_business: Option<String>,
    #[serde(default)]
    pub registered_address: Option<String>,
    #[serde(default)]
    pub zip_code: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub tax_classification: Option<String>,
    #[serde(default)]
    pub is_vat_registered: Option<bool>,
    #[serde(default)]
    pub eopt_tier: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct YearForm {
    pub form_code: String,
    /// `monthly`, `quarterly`, `annual` or `one_time`.
    pub frequency: String,
    #[serde(default = "yes")]
    pub active: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxYearView {
    pub year_id: String,
    pub tax_year: i32,
    pub overrides: TaxYearOverrides,
    pub forms: Vec<YearForm>,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxYearListResponse {
    pub years: Vec<TaxYearView>,
}

/// Creates or replaces a tax year. `forms: null` keeps the current set.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxYearPutRequest {
    #[serde(default)]
    pub overrides: TaxYearOverrides,
    #[serde(default)]
    pub forms: Option<Vec<YearForm>>,
}

/// Starts a year from another: its forms are copied only when the new year
/// has none yet.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxYearCloneRequest {
    pub from_year: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DraftSummary {
    pub draft_id: String,
    pub form_code: String,
    pub tax_year: i32,
    pub period_key: String,
    pub rev: u64,
    pub saved_once: bool,
    #[serde(default)]
    pub reviewed_at: Option<String>,
    #[serde(default)]
    pub reviewed_by: Option<String>,
    #[serde(default)]
    pub updated_by: Option<String>,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DraftView {
    #[serde(flatten)]
    pub summary: DraftSummary,
    /// The desktop app's form data; the server never reads it.
    pub payload: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DraftListResponse {
    pub drafts: Vec<DraftSummary>,
}

/// Saves a draft, keyed by form, year and period. `base_rev` is the
/// revision the change started from (omit or 0 for a new draft); a stale
/// one is refused with 409 so no device overwrites another's work.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DraftSaveRequest {
    pub form_code: String,
    pub tax_year: i32,
    pub period_key: String,
    pub payload: Value,
    #[serde(default)]
    pub base_rev: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FilingView {
    pub filing_id: String,
    #[serde(default)]
    pub draft_id: Option<String>,
    pub form_code: String,
    pub tax_year: i32,
    pub period_key: String,
    /// `queued`, `submitted`, `confirmed`, `failed` or `paid`.
    pub status: String,
    #[serde(default)]
    pub xml_sha256: Option<String>,
    #[serde(default)]
    pub submitted_at: Option<String>,
    #[serde(default)]
    pub receipt_ref: Option<String>,
    #[serde(default)]
    pub error_summary: Option<String>,
    #[serde(default)]
    pub paid_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FilingListResponse {
    pub filings: Vec<FilingView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FilingQueueRequest {
    pub draft_id: String,
}

/// What the app reports after a step: `submitted` needs `xml_sha256`,
/// `confirmed` needs `receipt_ref`, `failed` needs `error_summary`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FilingReportRequest {
    #[serde(default)]
    pub xml_sha256: Option<String>,
    #[serde(default)]
    pub receipt_ref: Option<String>,
    #[serde(default)]
    pub error_summary: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_form_is_active_unless_it_says_otherwise() {
        let form: YearForm =
            serde_json::from_value(serde_json::json!({"form_code": "1701Q", "frequency": "quarterly"})).unwrap();
        assert!(form.active);
        let off: YearForm = serde_json::from_value(
            serde_json::json!({"form_code": "1701Q", "frequency": "quarterly", "active": false}),
        )
        .unwrap();
        assert!(!off.active);
    }
}
