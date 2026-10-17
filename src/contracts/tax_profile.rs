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
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub branch_code: String,
    #[serde(default)]
    pub is_archived: bool,
    pub revision: u64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// The caller's standing: `owner`, `holding_org` (a firm's draft),
    /// `pass` or `none`.
    #[serde(default)]
    pub access: String,
    /// What the caller may do, as pass permission ids. The installed
    /// buwiz-forms app sends to BIR only when this includes
    /// `filings.publish`.
    #[serde(default)]
    pub permissions: Vec<String>,
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

/// One access pass, as either side sees it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PassView {
    pub pass_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub tin_last4: String,
    /// `organization` (agents come later).
    pub grantee_kind: String,
    pub grantee_id: String,
    #[serde(default)]
    pub grantee_name: Option<String>,
    #[serde(default)]
    pub grantee_slug: Option<String>,
    pub permissions: Vec<String>,
    /// `read_only`, `safe` or `full` when the permissions match one exactly.
    #[serde(default)]
    pub preset: Option<String>,
    pub can_publish: bool,
    /// `requested`, `offered`, `active` or `expired`.
    pub status: String,
    #[serde(default)]
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// `owner` when the caller owns the profile, `grantee` when the caller's
    /// workspace holds the pass.
    pub my_side: String,
    /// True when the next step is the caller's: approve a request (owner) or
    /// accept an offer (workspace).
    pub waiting_on_me: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PassPermissionInfo {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PassPresetInfo {
    pub id: String,
    pub label: String,
    pub permissions: Vec<String>,
    /// Selected when the give-access screen opens.
    pub is_default: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PassListResponse {
    /// Open passes (requested, offered, active, expired) on profiles the
    /// caller owns or held by the caller's workspaces.
    pub passes: Vec<PassView>,
    /// Every permission, in display order. `filings.publish` is last and is
    /// never part of a preset.
    pub permission_catalog: Vec<PassPermissionInfo>,
    pub presets: Vec<PassPresetInfo>,
    /// Open owner invites the caller's workspaces sent for their drafts.
    #[serde(default)]
    pub invites: Vec<OwnerInviteView>,
}

/// A firm's invite for a taxpayer to own one of its drafts.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnerInviteView {
    pub invite_id: String,
    pub profile_id: String,
    pub organization_id: String,
    pub profile_name: String,
    pub tin_last4: String,
    /// Only an account with this verified email can accept.
    pub email: String,
    /// `open` or `expired` in lists; `accepted` or `withdrawn` once decided.
    pub status: String,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnerInviteCreateRequest {
    pub email: String,
    /// How many days the link works (1 to 14; default 14).
    #[serde(default)]
    pub days: Option<u32>,
}

/// The new invite and its link. The secret is in the fragment (`#t=`), so
/// it never reaches server logs; the firm shares the link itself and it is
/// shown only this once.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnerInviteCreated {
    pub invite: OwnerInviteView,
    pub link_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnerInviteTokenRequest {
    pub token: String,
}

/// What the invited taxpayer sees before accepting.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnerInvitePreview {
    pub invite_id: String,
    pub profile_name: String,
    pub tin_last4: String,
    pub firm_name: String,
    pub email: String,
    /// `open`, `expired`, `accepted` or `withdrawn`.
    pub status: String,
    pub expires_at: String,
    /// True when the caller's verified email is the invited one.
    pub email_matches: bool,
}

/// A member a firm picked to publish to BIR. Publishing also needs the
/// owner's switch on the firm's pass.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirmPublisherView {
    pub user_id: String,
    pub email: String,
    pub added_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirmPublisherListResponse {
    pub publishers: Vec<FirmPublisherView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirmPublisherAddRequest {
    /// The member's sign-in email.
    pub email: String,
}

/// What a pass allows. Give a `preset`, or a `permissions` list, or both
/// (the list wins). Publishing to BIR is only ever `allow_publish`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PassChoice {
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub permissions: Option<Vec<String>>,
    #[serde(default)]
    pub allow_publish: bool,
    /// Last day the pass works (`YYYY-MM-DD`, Philippine time). When
    /// offering or approving, empty or missing means no end date. When
    /// changing a pass, missing keeps the current end date and `""` removes
    /// it.
    #[serde(default)]
    pub ends_on: Option<String>,
}

/// The owner offers a pass to a workspace.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PassOfferRequest {
    /// The workspace URL name, e.g. `abc-firm`. Or give `organization_id`.
    #[serde(default)]
    pub organization_slug: Option<String>,
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(flatten)]
    pub choice: PassChoice,
}

/// A workspace asks the owner for a pass, proving it knows the TIN.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PassAskRequest {
    pub organization_id: String,
    pub tin_root: String,
    pub branch_code: String,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub permissions: Option<Vec<String>>,
}

/// The owner approves a request (`choice` empty keeps what was asked for)
/// or changes an offered or active pass.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PassUpdateRequest {
    #[serde(flatten)]
    pub choice: PassChoice,
}

/// Start an ownership claim: the TIN and branch of the profile to claim.
/// The Form 2303 itself follows in parts to
/// `POST /tax-profiles/ownership-claims/{claim}/file-parts`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnershipClaimStartRequest {
    pub tin_root: String,
    pub branch_code: String,
}

/// One ownership claim. The claimant sees the profile's name only once the
/// claim is approved; staff always see it, plus the claimant's email.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnershipClaimView {
    pub claim_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub tin_last4: String,
    /// `awaiting_file`, `pending`, `approved`, `rejected` or `withdrawn`.
    pub status: String,
    #[serde(default)]
    pub file_kind: Option<String>,
    #[serde(default)]
    pub file_size: Option<u64>,
    /// SHA-256 (hex) of the uploaded file; kept after the file is deleted.
    #[serde(default)]
    pub file_sha256: Option<String>,
    /// The file carries a C2PA manifest that says nothing about AI.
    pub has_c2pa: bool,
    /// How many uploads were refused as AI-made.
    pub ai_refusals: u64,
    /// Parts of an upload in progress (see `OwnershipClaimPartRequest`).
    #[serde(default)]
    pub parts_received: u64,
    #[serde(default)]
    pub claimant_email: Option<String>,
    #[serde(default)]
    pub decision_note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub decided_at: Option<String>,
}

/// One part of a Form 2303 upload: base64 of at most 4 KiB, sent in
/// order (`index` 0 starts over). The last part (`index == total - 1`)
/// screens and stores the whole file.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnershipClaimPartRequest {
    pub index: u32,
    pub total: u32,
    pub data: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnershipClaimListResponse {
    pub claims: Vec<OwnershipClaimView>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnershipClaimDecisionRequest {
    /// Shown to the claimant. Required when rejecting.
    #[serde(default)]
    pub note: Option<String>,
}
