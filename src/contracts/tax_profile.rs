//! Cloud tax-profile DTOs shared by REST, server functions, and the UI.
//!
//! `id` is the server source of truth. Clients must not treat TIN as a local
//! identity key. The API never returns raw TIN, PIN hashes, TOTP secrets,
//! IMAP passwords, or mailbox OAuth tokens.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfileView {
    pub id: String,
    pub profile_id: String,
    pub owner_user_id: Option<String>,
    pub org_id: Option<String>,
    pub tin_last4: String,
    pub display_name: String,
    pub claim_status: String,
    pub full_name: String,
    pub rdo_code: String,
    pub address: String,
    pub line_of_business: String,
    pub zip_code: String,
    pub phone: String,
    pub email: String,
    pub taxpayer_type: String,
    pub tax_classification: Option<String>,
    pub is_vat_registered: bool,
    pub holder_kind: String,
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
    /// Attested at create time only. Never stored as a primary key.
    pub tin_root: String,
    pub branch_code: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default, alias = "full_name")]
    pub registered_name: String,
    pub rdo_code: String,
    #[serde(default)]
    pub line_of_business: String,
    #[serde(default, alias = "address")]
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
    #[serde(default, alias = "org_id")]
    pub org_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfilePatchRequest {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default, alias = "full_name")]
    pub registered_name: Option<String>,
    #[serde(default)]
    pub rdo_code: Option<String>,
    #[serde(default)]
    pub line_of_business: Option<String>,
    #[serde(default, alias = "address")]
    pub registered_address: Option<String>,
    #[serde(default)]
    pub zip_code: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub taxpayer_type: Option<String>,
    #[serde(default)]
    pub tax_classification: Option<String>,
    #[serde(default)]
    pub is_vat_registered: Option<bool>,
    /// Last `updated_at` the client observed. Required for multi-device LWW.
    #[serde(default)]
    pub updated_at: Option<String>,
    /// If present, must hash to this row's identity. Never used to rewrite TIN.
    #[serde(default)]
    pub tin_root: Option<String>,
    #[serde(default)]
    pub branch_code: Option<String>,
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
    pub id: String,
    pub organization_id: String,
    #[serde(default)]
    pub expected_revision: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeOrg {
    pub org_id: String,
    pub name: String,
    #[serde(default)]
    pub slug: String,
    pub role: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeResponse {
    pub user_id: String,
    pub email: Option<String>,
    pub orgs: Vec<MeOrg>,
}
