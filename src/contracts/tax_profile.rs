//! Cloud tax-profile DTOs shared by REST, server functions, and the UI.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfileView {
    pub profile_id: String,
    pub tin_root: String,
    pub branch_code: String,
    pub registered_name: String,
    pub rdo_code: String,
    pub line_of_business: String,
    pub registered_address: String,
    pub zip_code: String,
    pub phone: String,
    pub email: String,
    pub taxpayer_type: String,
    pub tax_classification: Option<String>,
    pub is_vat_registered: bool,
    pub holder_kind: String,
    pub holder_user_id: Option<String>,
    pub holder_organization_id: Option<String>,
    pub ownership_status: String,
    pub verification_status: String,
    pub verified_owner_user_id: Option<String>,
    pub revision: u64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfileListResponse {
    pub profiles: Vec<TaxProfileView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfileCreateRequest {
    pub tin_root: String,
    pub branch_code: String,
    pub registered_name: String,
    pub rdo_code: String,
    #[serde(default)]
    pub line_of_business: String,
    #[serde(default)]
    pub registered_address: String,
    #[serde(default)]
    pub zip_code: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub email: String,
    pub taxpayer_type: String,
    #[serde(default)]
    pub tax_classification: Option<String>,
    #[serde(default)]
    pub is_vat_registered: bool,
    /// When set, the profile is held by this organization (company-managed).
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfileClaimRequest {
    pub tin_root: String,
    pub branch_code: String,
    #[serde(default)]
    pub expected_revision: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfileTransferRequest {
    pub tin_root: String,
    pub branch_code: String,
    pub organization_id: String,
    #[serde(default)]
    pub expected_revision: Option<u64>,
}
