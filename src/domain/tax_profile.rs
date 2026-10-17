//! Cloud tax-profile aggregate: exclusive ownership of a BIR registration unit.
//!
//! Vocabulary follows hexuria/buwiz-forms `docs/tax-profile/CONTEXT.md`:
//! one Taxpayer (TIN root) has one or more Registration Units (TIN root +
//! branch code). This aggregate is the **cloud control plane** for a single
//! registration unit. The server source of truth is a **UUID**. Exclusive
//! uniqueness is the hashed TIN identity, never a raw TIN primary key.
//! Desktop IMAP secrets, PIN, TOTP, and mailbox OAuth tokens are never stored.

use serde::{Deserialize, Serialize};

use crate::domain::invite::{normalize_invite_email, InviteStatus, OwnerInvite};
use crate::domain::orus::ProofMethod;
use crate::domain::pass::{
    instant_before, Pass, PassGrantee, PassPermission, PassPermissions, PassStatus,
};
use crate::domain::tin::BranchCode;
use crate::domain::tin_identity::TinIdentityHash;

fn default_branch_code() -> String {
    BranchCode::HEAD_OFFICE.to_owned()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountHolder {
    Personal { user_id: String },
    Organization { organization_id: String },
}

impl AccountHolder {
    pub fn id(&self) -> &str {
        match self {
            Self::Personal { user_id } => user_id,
            Self::Organization { organization_id } => organization_id,
        }
    }

    pub fn is_personal(&self) -> bool {
        matches!(self, Self::Personal { .. })
    }

    pub fn is_organization(&self) -> bool {
        matches!(self, Self::Organization { .. })
    }

    pub fn same_as(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OwnershipStatus {
    PersonalExclusive,
    CompanyManaged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    Unverified,
    PendingOrus,
    Verified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaxpayerType {
    Individual,
    Corporation,
    Partnership,
    Cooperative,
    Estate,
    Trust,
}

/// Desktop-facing ownership of this cloud row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimStatus {
    Owned,
    PendingClaim,
    ReadOnly,
}

impl OwnershipStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PersonalExclusive => "personal_exclusive",
            Self::CompanyManaged => "company_managed",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, TaxProfileError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "personal_exclusive" => Ok(Self::PersonalExclusive),
            "company_managed" => Ok(Self::CompanyManaged),
            _ => Err(TaxProfileError::InvalidFacts {
                reason: "ownership_status is invalid".to_owned(),
            }),
        }
    }
}

impl VerificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unverified => "unverified",
            Self::PendingOrus => "pending_orus",
            Self::Verified => "verified",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, TaxProfileError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "unverified" => Ok(Self::Unverified),
            "pending_orus" => Ok(Self::PendingOrus),
            "verified" => Ok(Self::Verified),
            _ => Err(TaxProfileError::InvalidFacts {
                reason: "verification_status is invalid".to_owned(),
            }),
        }
    }
}

impl TaxpayerType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Individual => "individual",
            Self::Corporation => "corporation",
            Self::Partnership => "partnership",
            Self::Cooperative => "cooperative",
            Self::Estate => "estate",
            Self::Trust => "trust",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, TaxProfileError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "individual" => Ok(Self::Individual),
            "corporation" => Ok(Self::Corporation),
            "partnership" => Ok(Self::Partnership),
            "cooperative" => Ok(Self::Cooperative),
            "estate" => Ok(Self::Estate),
            "trust" => Ok(Self::Trust),
            _ => Err(TaxProfileError::InvalidFacts {
                reason: "taxpayer_type is invalid".to_owned(),
            }),
        }
    }
}

impl ClaimStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owned => "owned",
            Self::PendingClaim => "pending_claim",
            Self::ReadOnly => "read_only",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, TaxProfileError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "owned" => Ok(Self::Owned),
            "pending_claim" => Ok(Self::PendingClaim),
            "read_only" => Ok(Self::ReadOnly),
            _ => Err(TaxProfileError::InvalidFacts {
                reason: "claim_status is invalid".to_owned(),
            }),
        }
    }
}

/// Cloud-syncable subset of a taxpayer profile. Never includes PIN, TOTP,
/// IMAP passwords, or mailbox OAuth tokens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudTaxProfileFacts {
    pub registered_name: String,
    pub rdo_code: String,
    pub line_of_business: String,
    pub registered_address: String,
    pub zip_code: String,
    pub phone: String,
    pub email: String,
    pub taxpayer_type: TaxpayerType,
    pub tax_classification: Option<String>,
    pub is_vat_registered: bool,
    #[serde(default)]
    pub eopt_tier: Option<String>,
    #[serde(default)]
    pub business_start_date: Option<String>,
    #[serde(default)]
    pub birth_date: Option<String>,
    #[serde(default)]
    pub atc_codes: Vec<String>,
}

impl CloudTaxProfileFacts {
    pub fn validate(&self) -> Result<(), TaxProfileError> {
        if self.registered_name.trim().is_empty() {
            return Err(TaxProfileError::InvalidFacts {
                reason: "registered_name is required".to_owned(),
            });
        }
        if self.rdo_code.trim().is_empty() {
            return Err(TaxProfileError::InvalidFacts {
                reason: "rdo_code is required".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaxProfileCommand {
    /// Create the cloud row. TIN exclusivity is enforced here via `tin_hash` only.
    RegisterTaxProfile {
        profile_id: String,
        account_id: String,
        holder: AccountHolder,
        facts: CloudTaxProfileFacts,
        display_name: String,
        tin_hash: TinIdentityHash,
        tin_last4: String,
        branch_code: String,
        actor_user_id: String,
        occurred_at: String,
    },
    /// Patch identity / contact / classification. Never secrets; never TIN.
    UpdateTaxProfileIdentity {
        display_name: Option<String>,
        facts: Option<CloudTaxProfileFacts>,
        expected_updated_at: Option<String>,
        actor_user_id: String,
        /// Organizations the application verified the actor belongs to: the
        /// holding organization and any with a pass on this profile.
        actor_orgs: Vec<String>,
        occurred_at: String,
    },
    ArchiveTaxProfile {
        actor_user_id: String,
        /// Organizations the application verified the actor belongs to: the
        /// holding organization and any with a pass on this profile.
        actor_orgs: Vec<String>,
        occurred_at: String,
    },
    RestoreTaxProfile {
        actor_user_id: String,
        /// Organizations the application verified the actor belongs to: the
        /// holding organization and any with a pass on this profile.
        actor_orgs: Vec<String>,
        occurred_at: String,
    },
    /// Extra (company-managed). Not a V1 listed command; kept for reclaim/claim.
    Claim {
        claimant: AccountHolder,
        actor_user_id: String,
        identity_hash: TinIdentityHash,
        proof_method: ProofMethod,
    },
    Reclaim {
        actor_user_id: String,
        identity_hash: TinIdentityHash,
        proof_method: ProofMethod,
    },
    /// The owner offers a pass; the firm accepts it.
    OfferPass {
        pass_id: String,
        grantee: PassGrantee,
        permissions: PassPermissions,
        expires_at: Option<String>,
        actor_user_id: String,
        occurred_at: String,
    },
    /// A firm asks for a pass, proving it knows the TIN; the owner approves.
    RequestPass {
        pass_id: String,
        grantee: PassGrantee,
        permissions: PassPermissions,
        identity_hash: TinIdentityHash,
        actor_user_id: String,
        /// Organizations where the application verified the actor may manage
        /// passes (`organization.update`).
        actor_managed_orgs: Vec<String>,
        occurred_at: String,
    },
    /// The firm accepts an offered pass.
    AcceptPass {
        pass_id: String,
        actor_user_id: String,
        actor_managed_orgs: Vec<String>,
        occurred_at: String,
    },
    /// The owner approves a requested pass, optionally changing it first
    /// (`None` keeps what was asked for).
    ApprovePass {
        pass_id: String,
        permissions: Option<PassPermissions>,
        expires_at: Option<String>,
        actor_user_id: String,
        occurred_at: String,
    },
    /// The owner changes what an offered or active pass allows.
    ChangePass {
        pass_id: String,
        permissions: PassPermissions,
        expires_at: Option<String>,
        actor_user_id: String,
        occurred_at: String,
    },
    /// Either side ends a pass: the owner revokes or declines; the firm
    /// declines an offer, withdraws its request, or gives the pass back.
    EndPass {
        pass_id: String,
        actor_user_id: String,
        actor_managed_orgs: Vec<String>,
        occurred_at: String,
    },
    /// The firm holding a draft invites the taxpayer to own it. A newer
    /// invite replaces any open one.
    InviteOwner {
        invite_id: String,
        email: String,
        /// SHA-256 (base64url) of the link's secret.
        token_hash: String,
        expires_at: String,
        actor_user_id: String,
        actor_managed_orgs: Vec<String>,
        occurred_at: String,
    },
    /// The firm withdraws an open invite.
    WithdrawOwnerInvite {
        invite_id: String,
        actor_user_id: String,
        actor_managed_orgs: Vec<String>,
        occurred_at: String,
    },
    /// The invited taxpayer accepts and becomes the owner; the firm keeps a
    /// Full pass without publishing (`firm_pass_id`).
    AcceptOwnerInvite {
        invite_id: String,
        token_hash: String,
        actor_user_id: String,
        /// The actor's verified email (`auth_users.normalized_email` of an
        /// active account), looked up by the application.
        actor_verified_email: String,
        firm_pass_id: String,
        occurred_at: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaxProfileEvent {
    #[serde(alias = "Created")]
    TaxProfileRegistered {
        profile_id: String,
        #[serde(default)]
        account_id: String,
        #[serde(alias = "identity_hash")]
        tin_hash: TinIdentityHash,
        tin_last4: String,
        #[serde(default = "default_branch_code")]
        branch_code: String,
        display_name: String,
        holder: AccountHolder,
        ownership: OwnershipStatus,
        verification: VerificationStatus,
        facts: CloudTaxProfileFacts,
        actor_user_id: String,
        occurred_at: String,
    },
    Claimed {
        holder: AccountHolder,
        ownership: OwnershipStatus,
        verification: VerificationStatus,
        verified_owner_user_id: String,
        proof_method: ProofMethod,
        actor_user_id: String,
    },
    Reclaimed {
        holder: AccountHolder,
        ownership: OwnershipStatus,
        verification: VerificationStatus,
        verified_owner_user_id: String,
        proof_method: ProofMethod,
        actor_user_id: String,
    },
    Transferred {
        holder: AccountHolder,
        ownership: OwnershipStatus,
        actor_user_id: String,
    },
    #[serde(alias = "MetadataUpdated")]
    TaxProfileIdentityUpdated {
        display_name: String,
        facts: CloudTaxProfileFacts,
        actor_user_id: String,
        occurred_at: String,
    },
    TaxProfileArchived {
        actor_user_id: String,
        occurred_at: String,
    },
    TaxProfileRestored {
        actor_user_id: String,
        occurred_at: String,
    },
    PassOffered {
        pass_id: String,
        grantee: PassGrantee,
        permissions: Vec<String>,
        #[serde(default)]
        expires_at: Option<String>,
        actor_user_id: String,
        occurred_at: String,
    },
    PassRequested {
        pass_id: String,
        grantee: PassGrantee,
        permissions: Vec<String>,
        actor_user_id: String,
        occurred_at: String,
    },
    PassActivated {
        pass_id: String,
        permissions: Vec<String>,
        #[serde(default)]
        expires_at: Option<String>,
        actor_user_id: String,
        occurred_at: String,
    },
    PassChanged {
        pass_id: String,
        permissions: Vec<String>,
        #[serde(default)]
        expires_at: Option<String>,
        actor_user_id: String,
        occurred_at: String,
    },
    PassEnded {
        pass_id: String,
        /// `Declined` or `Revoked`.
        status: PassStatus,
        actor_user_id: String,
        occurred_at: String,
    },
    OwnerInvited {
        invite_id: String,
        organization_id: String,
        email: String,
        token_hash: String,
        expires_at: String,
        actor_user_id: String,
        occurred_at: String,
    },
    OwnerInviteWithdrawn {
        invite_id: String,
        actor_user_id: String,
        occurred_at: String,
    },
    OwnerInviteAccepted {
        invite_id: String,
        owner_user_id: String,
        organization_id: String,
        firm_pass_id: String,
        firm_pass_permissions: Vec<String>,
        occurred_at: String,
    },
}

impl TaxProfileEvent {
    /// Whether this event changes the profile itself (identity, facts,
    /// holder, archive state). Pass events only change who may work on it,
    /// so they leave the profile's `updated_at` (the desktop's
    /// last-write-wins token) alone.
    pub fn changes_profile(&self) -> bool {
        !matches!(
            self,
            Self::PassOffered { .. }
                | Self::PassRequested { .. }
                | Self::PassActivated { .. }
                | Self::PassChanged { .. }
                | Self::PassEnded { .. }
                | Self::OwnerInvited { .. }
                | Self::OwnerInviteWithdrawn { .. }
        )
    }

    pub fn event_type(&self) -> &'static str {
        match self {
            Self::TaxProfileRegistered { .. } => "tax_profile.registered",
            Self::Claimed { .. } => "tax_profile.claimed",
            Self::Reclaimed { .. } => "tax_profile.reclaimed",
            Self::Transferred { .. } => "tax_profile.transferred",
            Self::TaxProfileIdentityUpdated { .. } => "tax_profile.identity_updated",
            Self::TaxProfileArchived { .. } => "tax_profile.archived",
            Self::TaxProfileRestored { .. } => "tax_profile.restored",
            Self::PassOffered { .. } => "tax_profile.pass_offered",
            Self::PassRequested { .. } => "tax_profile.pass_requested",
            Self::PassActivated { .. } => "tax_profile.pass_activated",
            Self::PassChanged { .. } => "tax_profile.pass_changed",
            Self::PassEnded { .. } => "tax_profile.pass_ended",
            Self::OwnerInvited { .. } => "tax_profile.owner_invited",
            Self::OwnerInviteWithdrawn { .. } => "tax_profile.owner_invite_withdrawn",
            Self::OwnerInviteAccepted { .. } => "tax_profile.owner_invite_accepted",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaxProfileError {
    InvalidFacts { reason: String },
    AlreadyHeld { holder: AccountHolder },
    NotFound,
    NotHolder,
    ClaimRequiresCompanyHold,
    ReclaimDenied,
    ProofMismatch,
    ConcurrentModification,
    StaleWrite,
    IdentityImmutable,
    AlreadyArchived,
    NotArchived,
    /// Only the owner may share, change or end passes.
    OwnerOnly,
    PassNotFound,
    /// The firm already has an open pass (requested, offered or active).
    PassAlreadyOpen,
    /// The pass is not in a state that allows this step.
    PassState { reason: String },
    /// No invite with that id and secret.
    InviteNotFound,
    /// The invite or profile is not in a state that allows this step.
    InviteState { reason: String },
}

impl std::fmt::Display for TaxProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFacts { reason } => write!(f, "{reason}"),
            Self::AlreadyHeld { holder } => write!(
                f,
                "registration unit is already held exclusively by {}",
                holder.id()
            ),
            Self::NotFound => write!(f, "tax profile not found"),
            Self::NotHolder => write!(f, "caller is not the current holder"),
            Self::ClaimRequiresCompanyHold => write!(
                f,
                "claim is only allowed while a company/managing account holds the profile"
            ),
            Self::ReclaimDenied => write!(
                f,
                "only the verified natural or legal owner can reclaim this profile"
            ),
            Self::ProofMismatch => write!(
                f,
                "ownership proof does not match this registration unit"
            ),
            Self::ConcurrentModification => {
                write!(f, "the profile changed; retry with the latest revision")
            }
            Self::StaleWrite => write!(
                f,
                "the profile was updated on another device; retry with the latest updated_at"
            ),
            Self::IdentityImmutable => write!(
                f,
                "TIN identity is immutable; never merge two different TINs into one row"
            ),
            Self::AlreadyArchived => write!(f, "tax profile is already archived"),
            Self::NotArchived => write!(f, "tax profile is not archived"),
            Self::OwnerOnly => write!(f, "only the owner can share this tax profile"),
            Self::PassNotFound => write!(f, "access pass not found"),
            Self::PassAlreadyOpen => write!(
                f,
                "this workspace already has an access pass for this profile, or one is waiting"
            ),
            Self::PassState { reason } => write!(f, "{reason}"),
            Self::InviteNotFound => write!(f, "invite not found"),
            Self::InviteState { reason } => write!(f, "{reason}"),
        }
    }
}

/// An actor's standing on one tax profile. The single source for every read
/// and write check (the application builds it, the domain re-checks it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileAccess {
    /// The personal holder: everything, including sharing.
    Owner,
    /// A member of the firm working on a draft nobody owns yet (a profile the
    /// firm created): the Full preset, so no publishing, and no sharing,
    /// since only an owner shares.
    HoldingOrg,
    /// A member of one or more organizations with a working pass.
    Pass(PassPermissions),
    None,
}

/// What an actor wants to do with a profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileAction {
    View,
    Edit,
    Archive,
    /// Offer, approve, change or end passes.
    Share,
}

impl ProfileAccess {
    /// What this standing allows, as pass permissions.
    pub fn permissions(self) -> PassPermissions {
        match self {
            Self::Owner => PassPermissions::all(),
            Self::HoldingOrg => crate::domain::pass::PassPreset::Full.permissions(),
            Self::Pass(permissions) => permissions,
            Self::None => PassPermissions::empty(),
        }
    }

    #[must_use]
    pub fn has(self, permission: PassPermission) -> bool {
        self.permissions().contains(permission)
    }

    #[must_use]
    pub fn allows(self, action: ProfileAction) -> bool {
        match action {
            ProfileAction::View => self.has(PassPermission::ProfileView),
            ProfileAction::Edit => self.has(PassPermission::ProfileEdit),
            ProfileAction::Archive => matches!(self, Self::Owner | Self::HoldingOrg),
            ProfileAction::Share => self == Self::Owner,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::HoldingOrg => "holding_org",
            Self::Pass(_) => "pass",
            Self::None => "none",
        }
    }
}

/// A stable UUID for the pass an old "move to firm" event replays into.
pub fn legacy_move_pass_id(profile_id: &str, organization_id: &str, nth: usize) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("legacy-move|{profile_id}|{organization_id}|{nth}"));
    let mut b = [0u8; 16];
    b.copy_from_slice(&digest[..16]);
    b[6] = (b[6] & 0x0f) | 0x50; // name-based (version 5 layout)
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// Exclusive-ownership aggregate for one registration unit (UUID identity).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaxProfile {
    pub exists: bool,
    pub profile_id: Option<String>,
    pub account_id: Option<String>,
    pub identity_hash: Option<TinIdentityHash>,
    pub tin_last4: Option<String>,
    pub branch_code: Option<String>,
    pub display_name: Option<String>,
    pub holder: Option<AccountHolder>,
    pub ownership: Option<OwnershipStatus>,
    pub verification: VerificationStatus,
    pub facts: Option<CloudTaxProfileFacts>,
    pub verified_owner_user_id: Option<String>,
    pub is_archived: bool,
    pub updated_at: Option<String>,
    /// Every pass ever offered or asked for, open or ended.
    pub passes: Vec<Pass>,
    /// Every owner invite the holding firm ever sent.
    pub invites: Vec<OwnerInvite>,
}

impl Default for TaxProfile {
    fn default() -> Self {
        Self {
            exists: false,
            profile_id: None,
            account_id: None,
            identity_hash: None,
            tin_last4: None,
            branch_code: None,
            display_name: None,
            holder: None,
            ownership: None,
            verification: VerificationStatus::Unverified,
            facts: None,
            verified_owner_user_id: None,
            is_archived: false,
            updated_at: None,
            passes: Vec::new(),
            invites: Vec::new(),
        }
    }
}

impl TaxProfile {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn claim_status_for(&self, actor_user_id: &str) -> ClaimStatus {
        match &self.holder {
            Some(AccountHolder::Personal { user_id }) if user_id == actor_user_id => {
                ClaimStatus::Owned
            }
            Some(AccountHolder::Organization { .. }) => ClaimStatus::PendingClaim,
            Some(AccountHolder::Personal { .. }) => ClaimStatus::ReadOnly,
            None => ClaimStatus::ReadOnly,
        }
    }

    pub fn owner_user_id(&self) -> Option<String> {
        match &self.holder {
            Some(AccountHolder::Personal { user_id }) => Some(user_id.clone()),
            Some(AccountHolder::Organization { .. }) => self.verified_owner_user_id.clone(),
            None => None,
        }
    }

    pub fn org_id(&self) -> Option<String> {
        match &self.holder {
            Some(AccountHolder::Organization { organization_id }) => {
                Some(organization_id.clone())
            }
            _ => None,
        }
    }

    pub fn handle(
        &self,
        command: TaxProfileCommand,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        match command {
            TaxProfileCommand::RegisterTaxProfile {
                profile_id,
                account_id,
                holder,
                facts,
                display_name,
                tin_hash,
                tin_last4,
                branch_code,
                actor_user_id,
                occurred_at,
            } => self.handle_register(
                profile_id,
                account_id,
                holder,
                facts,
                display_name,
                tin_hash,
                tin_last4,
                branch_code,
                actor_user_id,
                occurred_at,
            ),
            TaxProfileCommand::UpdateTaxProfileIdentity {
                display_name,
                facts,
                expected_updated_at,
                actor_user_id,
                actor_orgs,
                occurred_at,
            } => self.handle_update_identity(
                display_name,
                facts,
                expected_updated_at,
                actor_user_id,
                actor_orgs,
                occurred_at,
            ),
            TaxProfileCommand::ArchiveTaxProfile {
                actor_user_id,
                actor_orgs,
                occurred_at,
            } => self.handle_archive(actor_user_id, actor_orgs, occurred_at),
            TaxProfileCommand::RestoreTaxProfile {
                actor_user_id,
                actor_orgs,
                occurred_at,
            } => self.handle_restore(actor_user_id, actor_orgs, occurred_at),
            TaxProfileCommand::Claim {
                claimant,
                actor_user_id,
                identity_hash,
                proof_method,
            } => self.handle_claim(claimant, actor_user_id, identity_hash, proof_method),
            TaxProfileCommand::Reclaim {
                actor_user_id,
                identity_hash,
                proof_method,
            } => self.handle_reclaim(actor_user_id, identity_hash, proof_method),
            TaxProfileCommand::OfferPass {
                pass_id,
                grantee,
                permissions,
                expires_at,
                actor_user_id,
                occurred_at,
            } => self.handle_offer_pass(
                pass_id,
                grantee,
                permissions,
                expires_at,
                actor_user_id,
                occurred_at,
            ),
            TaxProfileCommand::RequestPass {
                pass_id,
                grantee,
                permissions,
                identity_hash,
                actor_user_id,
                actor_managed_orgs,
                occurred_at,
            } => self.handle_request_pass(
                pass_id,
                grantee,
                permissions,
                identity_hash,
                actor_user_id,
                actor_managed_orgs,
                occurred_at,
            ),
            TaxProfileCommand::AcceptPass {
                pass_id,
                actor_user_id,
                actor_managed_orgs,
                occurred_at,
            } => self.handle_accept_pass(pass_id, actor_user_id, actor_managed_orgs, occurred_at),
            TaxProfileCommand::ApprovePass {
                pass_id,
                permissions,
                expires_at,
                actor_user_id,
                occurred_at,
            } => self.handle_approve_pass(
                pass_id,
                permissions,
                expires_at,
                actor_user_id,
                occurred_at,
            ),
            TaxProfileCommand::ChangePass {
                pass_id,
                permissions,
                expires_at,
                actor_user_id,
                occurred_at,
            } => self.handle_change_pass(
                pass_id,
                permissions,
                expires_at,
                actor_user_id,
                occurred_at,
            ),
            TaxProfileCommand::EndPass {
                pass_id,
                actor_user_id,
                actor_managed_orgs,
                occurred_at,
            } => self.handle_end_pass(pass_id, actor_user_id, actor_managed_orgs, occurred_at),
            TaxProfileCommand::InviteOwner {
                invite_id,
                email,
                token_hash,
                expires_at,
                actor_user_id,
                actor_managed_orgs,
                occurred_at,
            } => self.handle_invite_owner(
                invite_id,
                email,
                token_hash,
                expires_at,
                actor_user_id,
                actor_managed_orgs,
                occurred_at,
            ),
            TaxProfileCommand::WithdrawOwnerInvite {
                invite_id,
                actor_user_id,
                actor_managed_orgs,
                occurred_at,
            } => self.handle_withdraw_owner_invite(
                invite_id,
                actor_user_id,
                actor_managed_orgs,
                occurred_at,
            ),
            TaxProfileCommand::AcceptOwnerInvite {
                invite_id,
                token_hash,
                actor_user_id,
                actor_verified_email,
                firm_pass_id,
                occurred_at,
            } => self.handle_accept_owner_invite(
                invite_id,
                token_hash,
                actor_user_id,
                actor_verified_email,
                firm_pass_id,
                occurred_at,
            ),
        }
    }

    pub fn apply(&mut self, event: &TaxProfileEvent) {
        match event {
            TaxProfileEvent::TaxProfileRegistered {
                profile_id,
                account_id,
                tin_hash,
                tin_last4,
                branch_code,
                display_name,
                holder,
                ownership,
                verification,
                facts,
                actor_user_id,
                occurred_at,
            } => {
                self.exists = true;
                self.profile_id = Some(profile_id.clone());
                self.identity_hash = Some(tin_hash.clone());
                self.tin_last4 = Some(tin_last4.clone());
                self.branch_code = Some(if branch_code.trim().is_empty() {
                    default_branch_code()
                } else {
                    branch_code.clone()
                });
                self.account_id = Some(if account_id.trim().is_empty() {
                    match holder {
                        AccountHolder::Personal { user_id } => user_id.clone(),
                        AccountHolder::Organization { .. } => actor_user_id.clone(),
                    }
                } else {
                    account_id.clone()
                });
                self.display_name = Some(display_name.clone());
                self.holder = Some(holder.clone());
                self.ownership = Some(*ownership);
                self.verification = *verification;
                self.facts = Some(facts.clone());
                self.is_archived = false;
                self.updated_at = Some(occurred_at.clone());
            }
            TaxProfileEvent::Claimed {
                holder,
                ownership,
                verification,
                verified_owner_user_id,
                actor_user_id,
                ..
            }
            | TaxProfileEvent::Reclaimed {
                holder,
                ownership,
                verification,
                verified_owner_user_id,
                actor_user_id,
                ..
            } => {
                // A claim or reclaim puts the proven owner in charge and ends
                // every open pass, including a firm's pass from an old move,
                // and every open owner invite.
                self.end_open_passes(actor_user_id);
                self.withdraw_open_invites(actor_user_id, None);
                self.holder = Some(holder.clone());
                self.ownership = Some(*ownership);
                self.verification = *verification;
                self.verified_owner_user_id = Some(verified_owner_user_id.clone());
            }
            TaxProfileEvent::Transferred {
                holder,
                ownership,
                actor_user_id,
            } => match (&self.holder, holder) {
                // Moving to a firm no longer exists. An old move by the owner
                // replays under "the taxpayer always wins": the owner keeps
                // the profile and the firm works under a Full pass (no
                // publishing) that the owner can change or end.
                (
                    Some(AccountHolder::Personal { .. }),
                    AccountHolder::Organization { organization_id },
                ) => self.grant_legacy_move_pass(organization_id, actor_user_id),
                _ => {
                    self.end_passes_on_new_holder(holder, actor_user_id);
                    self.holder = Some(holder.clone());
                    self.ownership = Some(*ownership);
                }
            },
            TaxProfileEvent::TaxProfileIdentityUpdated {
                display_name,
                facts,
                occurred_at,
                ..
            } => {
                self.display_name = Some(display_name.clone());
                self.facts = Some(facts.clone());
                self.updated_at = Some(occurred_at.clone());
            }
            TaxProfileEvent::TaxProfileArchived { occurred_at, .. } => {
                self.is_archived = true;
                self.updated_at = Some(occurred_at.clone());
            }
            TaxProfileEvent::TaxProfileRestored { occurred_at, .. } => {
                self.is_archived = false;
                self.updated_at = Some(occurred_at.clone());
            }
            TaxProfileEvent::PassOffered {
                pass_id,
                grantee,
                permissions,
                expires_at,
                actor_user_id,
                occurred_at,
            } => self.passes.push(Pass {
                pass_id: pass_id.clone(),
                grantee: grantee.clone(),
                permissions: PassPermissions::from_stored(permissions),
                status: PassStatus::Offered,
                expires_at: expires_at.clone(),
                created_by: actor_user_id.clone(),
                created_at: occurred_at.clone(),
                updated_at: occurred_at.clone(),
                ended_by: None,
            }),
            TaxProfileEvent::PassRequested {
                pass_id,
                grantee,
                permissions,
                actor_user_id,
                occurred_at,
            } => self.passes.push(Pass {
                pass_id: pass_id.clone(),
                grantee: grantee.clone(),
                permissions: PassPermissions::from_stored(permissions),
                status: PassStatus::Requested,
                expires_at: None,
                created_by: actor_user_id.clone(),
                created_at: occurred_at.clone(),
                updated_at: occurred_at.clone(),
                ended_by: None,
            }),
            TaxProfileEvent::PassActivated {
                pass_id,
                permissions,
                expires_at,
                occurred_at,
                ..
            }
            | TaxProfileEvent::PassChanged {
                pass_id,
                permissions,
                expires_at,
                occurred_at,
                ..
            } => {
                let activate = matches!(event, TaxProfileEvent::PassActivated { .. });
                if let Some(pass) = self.pass_mut(pass_id) {
                    pass.permissions = PassPermissions::from_stored(permissions);
                    pass.expires_at = expires_at.clone();
                    pass.updated_at = occurred_at.clone();
                    if activate {
                        pass.status = PassStatus::Active;
                    }
                }
            }
            TaxProfileEvent::PassEnded {
                pass_id,
                status,
                actor_user_id,
                occurred_at,
            } => {
                if let Some(pass) = self.pass_mut(pass_id) {
                    pass.status = *status;
                    pass.ended_by = Some(actor_user_id.clone());
                    pass.updated_at = occurred_at.clone();
                }
            }
            TaxProfileEvent::OwnerInvited {
                invite_id,
                organization_id,
                email,
                token_hash,
                expires_at,
                actor_user_id,
                occurred_at,
            } => self.invites.push(OwnerInvite {
                invite_id: invite_id.clone(),
                organization_id: organization_id.clone(),
                email: email.clone(),
                token_hash: token_hash.clone(),
                status: InviteStatus::Open,
                expires_at: expires_at.clone(),
                created_by: actor_user_id.clone(),
                created_at: occurred_at.clone(),
                updated_at: occurred_at.clone(),
                decided_by: None,
            }),
            TaxProfileEvent::OwnerInviteWithdrawn {
                invite_id,
                actor_user_id,
                occurred_at,
            } => {
                if let Some(invite) = self.invite_mut(invite_id) {
                    invite.status = InviteStatus::Withdrawn;
                    invite.decided_by = Some(actor_user_id.clone());
                    invite.updated_at = occurred_at.clone();
                }
            }
            TaxProfileEvent::OwnerInviteAccepted {
                invite_id,
                owner_user_id,
                organization_id,
                firm_pass_id,
                firm_pass_permissions,
                occurred_at,
            } => {
                if let Some(invite) = self.invite_mut(invite_id) {
                    invite.status = InviteStatus::Accepted;
                    invite.decided_by = Some(owner_user_id.clone());
                    invite.updated_at = occurred_at.clone();
                }
                self.withdraw_open_invites(owner_user_id, Some(occurred_at));
                self.end_open_passes(owner_user_id);
                self.holder = Some(AccountHolder::Personal {
                    user_id: owner_user_id.clone(),
                });
                self.ownership = Some(OwnershipStatus::PersonalExclusive);
                self.updated_at = Some(occurred_at.clone());
                // The firm that prepared the draft keeps working, under a
                // pass the new owner can change or end.
                self.passes.push(Pass {
                    pass_id: firm_pass_id.clone(),
                    grantee: PassGrantee::Organization {
                        organization_id: organization_id.clone(),
                    },
                    permissions: PassPermissions::from_stored(firm_pass_permissions),
                    status: PassStatus::Active,
                    expires_at: None,
                    created_by: owner_user_id.clone(),
                    created_at: occurred_at.clone(),
                    updated_at: occurred_at.clone(),
                    ended_by: None,
                });
            }
        }
    }

    fn invite_mut(&mut self, invite_id: &str) -> Option<&mut OwnerInvite> {
        self.invites
            .iter_mut()
            .find(|invite| invite.invite_id == invite_id)
    }

    pub fn invite(&self, invite_id: &str) -> Option<&OwnerInvite> {
        self.invites.iter().find(|invite| invite.invite_id == invite_id)
    }

    /// Withdraws every open invite (the profile got an owner). `at` is the
    /// time when the event carries one.
    fn withdraw_open_invites(&mut self, actor_user_id: &str, at: Option<&String>) {
        for invite in self
            .invites
            .iter_mut()
            .filter(|invite| invite.status == InviteStatus::Open)
        {
            invite.status = InviteStatus::Withdrawn;
            invite.decided_by = Some(actor_user_id.to_owned());
            if let Some(at) = at {
                invite.updated_at = at.clone();
            }
        }
    }

    fn pass_mut(&mut self, pass_id: &str) -> Option<&mut Pass> {
        self.passes.iter_mut().find(|pass| pass.pass_id == pass_id)
    }

    pub fn pass(&self, pass_id: &str) -> Option<&Pass> {
        self.passes.iter().find(|pass| pass.pass_id == pass_id)
    }

    /// Passes were given by the previous holder; a new holder starts clean.
    fn end_passes_on_new_holder(&mut self, new_holder: &AccountHolder, actor_user_id: &str) {
        if self.holder.as_ref() == Some(new_holder) {
            return;
        }
        for pass in self.passes.iter_mut().filter(|pass| pass.status.is_open()) {
            pass.status = if pass.status.is_pending() {
                PassStatus::Declined
            } else {
                PassStatus::Revoked
            };
            pass.ended_by = Some(actor_user_id.to_owned());
        }
    }

    /// Ends every open pass (a claim or reclaim starts clean).
    fn end_open_passes(&mut self, actor_user_id: &str) {
        for pass in self.passes.iter_mut().filter(|pass| pass.status.is_open()) {
            pass.status = if pass.status.is_pending() {
                PassStatus::Declined
            } else {
                PassStatus::Revoked
            };
            pass.ended_by = Some(actor_user_id.to_owned());
        }
    }

    /// The pass an old "move to firm" turns into on replay. Its id is
    /// derived from the profile, the firm and how many such moves came
    /// before, so every replay (and the stored pass row) agrees on it.
    fn grant_legacy_move_pass(&mut self, organization_id: &str, actor_user_id: &str) {
        let grantee = PassGrantee::Organization {
            organization_id: organization_id.to_owned(),
        };
        // One open pass per firm: an older one for the same firm ends.
        for pass in self
            .passes
            .iter_mut()
            .filter(|pass| pass.status.is_open() && pass.grantee == grantee)
        {
            pass.status = PassStatus::Revoked;
            pass.ended_by = Some(actor_user_id.to_owned());
        }
        let nth = self.passes.len();
        // Old move events carry no time; use the profile's last change.
        let at = self
            .updated_at
            .clone()
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_owned());
        self.passes.push(Pass {
            pass_id: legacy_move_pass_id(
                self.profile_id.as_deref().unwrap_or_default(),
                organization_id,
                nth,
            ),
            grantee,
            permissions: crate::domain::pass::PassPreset::Full.permissions(),
            status: PassStatus::Active,
            expires_at: None,
            created_by: actor_user_id.to_owned(),
            created_at: at.clone(),
            updated_at: at,
            ended_by: None,
        });
    }

    fn handle_register(
        &self,
        profile_id: String,
        account_id: String,
        holder: AccountHolder,
        facts: CloudTaxProfileFacts,
        display_name: String,
        tin_hash: TinIdentityHash,
        tin_last4: String,
        branch_code: String,
        actor_user_id: String,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        facts.validate()?;
        if self.exists {
            return Err(TaxProfileError::AlreadyHeld {
                holder: self.holder.clone().expect("held profile has a holder"),
            });
        }
        if profile_id.trim().is_empty() {
            return Err(TaxProfileError::InvalidFacts {
                reason: "profile id is required".to_owned(),
            });
        }
        if account_id.trim().is_empty() {
            return Err(TaxProfileError::InvalidFacts {
                reason: "account_id is required".to_owned(),
            });
        }
        if tin_last4.len() != 4 || !tin_last4.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(TaxProfileError::InvalidFacts {
                reason: "tin_last4 is invalid".to_owned(),
            });
        }
        let branch_code = BranchCode::parse(&branch_code).map_err(|reason| {
            TaxProfileError::InvalidFacts { reason }
        })?;
        let display_name = if display_name.trim().is_empty() {
            facts.registered_name.clone()
        } else {
            display_name.trim().to_owned()
        };
        let ownership = match &holder {
            AccountHolder::Personal { .. } => OwnershipStatus::PersonalExclusive,
            AccountHolder::Organization { .. } => OwnershipStatus::CompanyManaged,
        };
        Ok(vec![TaxProfileEvent::TaxProfileRegistered {
            profile_id,
            account_id,
            tin_hash,
            tin_last4,
            branch_code: branch_code.as_str().to_owned(),
            display_name,
            holder,
            ownership,
            verification: VerificationStatus::Unverified,
            facts,
            actor_user_id,
            occurred_at,
        }])
    }

    fn handle_claim(
        &self,
        claimant: AccountHolder,
        actor_user_id: String,
        identity_hash: TinIdentityHash,
        proof_method: ProofMethod,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        self.require_identity(&identity_hash)?;
        if !matches!(self.ownership, Some(OwnershipStatus::CompanyManaged)) {
            return Err(TaxProfileError::ClaimRequiresCompanyHold);
        }
        if !claimant.is_personal() || claimant.id() != actor_user_id {
            return Err(TaxProfileError::InvalidFacts {
                reason: "only a personal account can claim as the real owner".to_owned(),
            });
        }
        Ok(vec![TaxProfileEvent::Claimed {
            holder: claimant,
            ownership: OwnershipStatus::PersonalExclusive,
            verification: VerificationStatus::Verified,
            verified_owner_user_id: actor_user_id.clone(),
            proof_method,
            actor_user_id,
        }])
    }

    fn handle_reclaim(
        &self,
        actor_user_id: String,
        identity_hash: TinIdentityHash,
        proof_method: ProofMethod,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        self.require_identity(&identity_hash)?;
        let is_recorded_owner = self
            .verified_owner_user_id
            .as_deref()
            .is_some_and(|id| id == actor_user_id);
        let personal_holder_is_actor = matches!(
            &self.holder,
            Some(AccountHolder::Personal { user_id }) if user_id == &actor_user_id
        );
        if personal_holder_is_actor {
            return Err(TaxProfileError::InvalidFacts {
                reason: "caller already holds exclusive control".to_owned(),
            });
        }
        if !is_recorded_owner
            && !matches!(
                proof_method,
                ProofMethod::FakeOrus | ProofMethod::Orus | ProofMethod::Form2303Review
            )
        {
            return Err(TaxProfileError::ReclaimDenied);
        }
        Ok(vec![TaxProfileEvent::Reclaimed {
            holder: AccountHolder::Personal {
                user_id: actor_user_id.clone(),
            },
            ownership: OwnershipStatus::PersonalExclusive,
            verification: VerificationStatus::Verified,
            verified_owner_user_id: actor_user_id.clone(),
            proof_method,
            actor_user_id,
        }])
    }

    fn handle_update_identity(
        &self,
        display_name: Option<String>,
        facts: Option<CloudTaxProfileFacts>,
        expected_updated_at: Option<String>,
        actor_user_id: String,
        actor_orgs: Vec<String>,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        self.require_access(&actor_user_id, &actor_orgs, &occurred_at, ProfileAction::Edit)?;
        if self.is_archived {
            return Err(TaxProfileError::AlreadyArchived);
        }
        if let Some(expected) = expected_updated_at
            && self.updated_at.as_deref() != Some(expected.as_str())
        {
            return Err(TaxProfileError::StaleWrite);
        }
        let next_facts = match facts {
            Some(facts) => {
                facts.validate()?;
                facts
            }
            None => self.facts.clone().ok_or(TaxProfileError::NotFound)?,
        };
        let next_name = display_name
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                self.display_name
                    .clone()
                    .unwrap_or_else(|| next_facts.registered_name.clone())
            });
        Ok(vec![TaxProfileEvent::TaxProfileIdentityUpdated {
            display_name: next_name,
            facts: next_facts,
            actor_user_id,
            occurred_at,
        }])
    }

    fn handle_archive(
        &self,
        actor_user_id: String,
        actor_orgs: Vec<String>,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        self.require_access(&actor_user_id, &actor_orgs, &occurred_at, ProfileAction::Archive)?;
        if self.is_archived {
            return Err(TaxProfileError::AlreadyArchived);
        }
        Ok(vec![TaxProfileEvent::TaxProfileArchived {
            actor_user_id,
            occurred_at,
        }])
    }

    fn handle_restore(
        &self,
        actor_user_id: String,
        actor_orgs: Vec<String>,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        self.require_access(&actor_user_id, &actor_orgs, &occurred_at, ProfileAction::Archive)?;
        if !self.is_archived {
            return Err(TaxProfileError::NotArchived);
        }
        Ok(vec![TaxProfileEvent::TaxProfileRestored {
            actor_user_id,
            occurred_at,
        }])
    }

    /// What `actor_user_id` is to this profile at instant `now`.
    /// `actor_orgs` are the organizations the application verified the actor
    /// belongs to. Registering the profile grants nothing by itself: control
    /// follows the holder, so a claim or transfer moves it. A working pass
    /// adds rights for every member of the firm that holds it.
    #[must_use]
    pub fn access_for(&self, actor_user_id: &str, actor_orgs: &[String], now: &str) -> ProfileAccess {
        let member_of = |org: &str| actor_orgs.iter().any(|id| id == org);
        match &self.holder {
            Some(AccountHolder::Personal { user_id }) if user_id == actor_user_id => {
                return ProfileAccess::Owner;
            }
            Some(AccountHolder::Organization { organization_id }) if member_of(organization_id) => {
                return ProfileAccess::HoldingOrg;
            }
            _ => {}
        }
        if self.is_archived {
            // An archived profile is frozen for everyone but its holder.
            return ProfileAccess::None;
        }
        let from_passes = self
            .passes
            .iter()
            .filter(|pass| pass.is_usable(now) && member_of(pass.grantee.id()))
            .fold(PassPermissions::empty(), |set, pass| set.union(pass.permissions));
        if from_passes.is_empty() {
            ProfileAccess::None
        } else {
            ProfileAccess::Pass(from_passes)
        }
    }

    fn require_access(
        &self,
        actor_user_id: &str,
        actor_orgs: &[String],
        now: &str,
        action: ProfileAction,
    ) -> Result<(), TaxProfileError> {
        if self.access_for(actor_user_id, actor_orgs, now).allows(action) {
            Ok(())
        } else if action == ProfileAction::Share {
            Err(TaxProfileError::OwnerOnly)
        } else {
            Err(TaxProfileError::NotHolder)
        }
    }

    fn require_owner(&self, actor_user_id: &str, now: &str) -> Result<(), TaxProfileError> {
        self.require_access(actor_user_id, &[], now, ProfileAction::Share)
    }

    fn open_pass_for(&self, grantee: &PassGrantee) -> Option<&Pass> {
        self.passes
            .iter()
            .find(|pass| pass.status.is_open() && &pass.grantee == grantee)
    }

    fn check_new_pass(&self, pass_id: &str, grantee: &PassGrantee) -> Result<(), TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        if self.is_archived {
            return Err(TaxProfileError::AlreadyArchived);
        }
        if pass_id.trim().is_empty() || self.pass(pass_id).is_some() {
            return Err(TaxProfileError::InvalidFacts {
                reason: "pass id is missing or already used".to_owned(),
            });
        }
        if grantee.id().trim().is_empty() {
            return Err(TaxProfileError::InvalidFacts {
                reason: "pick a workspace to give access to".to_owned(),
            });
        }
        if self.open_pass_for(grantee).is_some() {
            return Err(TaxProfileError::PassAlreadyOpen);
        }
        Ok(())
    }

    fn check_end_date(expires_at: Option<&str>, now: &str) -> Result<(), TaxProfileError> {
        match expires_at {
            Some(end) if !instant_before(now, end) => Err(TaxProfileError::InvalidFacts {
                reason: "the end date must be in the future".to_owned(),
            }),
            _ => Ok(()),
        }
    }

    fn check_permissions(permissions: PassPermissions) -> Result<(), TaxProfileError> {
        if permissions.is_empty() {
            return Err(TaxProfileError::InvalidFacts {
                reason: "a pass needs at least one permission".to_owned(),
            });
        }
        Ok(())
    }

    fn handle_offer_pass(
        &self,
        pass_id: String,
        grantee: PassGrantee,
        permissions: PassPermissions,
        expires_at: Option<String>,
        actor_user_id: String,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        // Ownership first, so a non-owner learns nothing about the passes.
        self.require_owner(&actor_user_id, &occurred_at)?;
        self.check_new_pass(&pass_id, &grantee)?;
        Self::check_permissions(permissions)?;
        Self::check_end_date(expires_at.as_deref(), &occurred_at)?;
        Ok(vec![TaxProfileEvent::PassOffered {
            pass_id,
            grantee,
            permissions: permissions.with_needs().ids(),
            expires_at,
            actor_user_id,
            occurred_at,
        }])
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_request_pass(
        &self,
        pass_id: String,
        grantee: PassGrantee,
        permissions: PassPermissions,
        identity_hash: TinIdentityHash,
        actor_user_id: String,
        actor_managed_orgs: Vec<String>,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        self.require_identity(&identity_hash)?;
        if !actor_managed_orgs.iter().any(|org| org == grantee.id()) {
            return Err(TaxProfileError::NotHolder);
        }
        if !matches!(self.holder, Some(AccountHolder::Personal { .. })) {
            return Err(TaxProfileError::PassState {
                reason: "this profile has no owner yet to approve a request".to_owned(),
            });
        }
        self.check_new_pass(&pass_id, &grantee)?;
        Self::check_permissions(permissions)?;
        if permissions.contains(PassPermission::FilingsPublish) {
            return Err(TaxProfileError::InvalidFacts {
                reason: "only the owner can switch on publishing to BIR".to_owned(),
            });
        }
        Ok(vec![TaxProfileEvent::PassRequested {
            pass_id,
            grantee,
            permissions: permissions.with_needs().ids(),
            actor_user_id,
            occurred_at,
        }])
    }

    fn open_pass(&self, pass_id: &str) -> Result<&Pass, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        let pass = self.pass(pass_id).ok_or(TaxProfileError::PassNotFound)?;
        if !pass.status.is_open() {
            return Err(TaxProfileError::PassState {
                reason: "this access pass has already ended".to_owned(),
            });
        }
        Ok(pass)
    }

    /// [`Self::open_pass`] for steps that start or change access, which an
    /// archived profile no longer allows (ending a pass still works).
    fn open_pass_to_grant(&self, pass_id: &str) -> Result<&Pass, TaxProfileError> {
        let pass = self.open_pass(pass_id)?;
        if self.is_archived {
            return Err(TaxProfileError::PassState {
                reason: "this profile is archived; restore it first".to_owned(),
            });
        }
        Ok(pass)
    }

    fn handle_accept_pass(
        &self,
        pass_id: String,
        actor_user_id: String,
        actor_managed_orgs: Vec<String>,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        let pass = self.open_pass_to_grant(&pass_id)?;
        if !actor_managed_orgs.iter().any(|org| org == pass.grantee.id()) {
            return Err(TaxProfileError::NotHolder);
        }
        if pass.status != PassStatus::Offered {
            return Err(TaxProfileError::PassState {
                reason: "only an offered pass can be accepted".to_owned(),
            });
        }
        if pass.is_expired(&occurred_at) {
            return Err(TaxProfileError::PassState {
                reason: "this offer's end date has passed".to_owned(),
            });
        }
        Ok(vec![TaxProfileEvent::PassActivated {
            pass_id,
            permissions: pass.permissions.ids(),
            expires_at: pass.expires_at.clone(),
            actor_user_id,
            occurred_at,
        }])
    }

    fn handle_approve_pass(
        &self,
        pass_id: String,
        permissions: Option<PassPermissions>,
        expires_at: Option<String>,
        actor_user_id: String,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        self.require_owner(&actor_user_id, &occurred_at)?;
        let pass = self.open_pass_to_grant(&pass_id)?;
        if pass.status != PassStatus::Requested {
            return Err(TaxProfileError::PassState {
                reason: "only a requested pass can be approved".to_owned(),
            });
        }
        let permissions = permissions.unwrap_or(pass.permissions);
        Self::check_permissions(permissions)?;
        Self::check_end_date(expires_at.as_deref(), &occurred_at)?;
        Ok(vec![TaxProfileEvent::PassActivated {
            pass_id,
            permissions: permissions.with_needs().ids(),
            expires_at,
            actor_user_id,
            occurred_at,
        }])
    }

    fn handle_change_pass(
        &self,
        pass_id: String,
        permissions: PassPermissions,
        expires_at: Option<String>,
        actor_user_id: String,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        self.require_owner(&actor_user_id, &occurred_at)?;
        let pass = self.open_pass_to_grant(&pass_id)?;
        if pass.status == PassStatus::Requested {
            return Err(TaxProfileError::PassState {
                reason: "approve or decline the request instead".to_owned(),
            });
        }
        Self::check_permissions(permissions)?;
        Self::check_end_date(expires_at.as_deref(), &occurred_at)?;
        Ok(vec![TaxProfileEvent::PassChanged {
            pass_id,
            permissions: permissions.with_needs().ids(),
            expires_at,
            actor_user_id,
            occurred_at,
        }])
    }

    /// [`Self::access_for`], with publishing kept only where both sides
    /// said yes: the owner switched it on for a working pass, and that firm
    /// picked the actor as one of its publishers (`publisher_orgs`).
    #[must_use]
    pub fn effective_access(
        &self,
        actor_user_id: &str,
        actor_orgs: &[String],
        publisher_orgs: &[String],
        now: &str,
    ) -> ProfileAccess {
        let access = self.access_for(actor_user_id, actor_orgs, now);
        let ProfileAccess::Pass(permissions) = access else {
            return access;
        };
        let listed = |org: &str| {
            actor_orgs.iter().any(|id| id == org) && publisher_orgs.iter().any(|id| id == org)
        };
        let picked = self.passes.iter().any(|pass| {
            pass.is_usable(now)
                && pass.permissions.contains(PassPermission::FilingsPublish)
                && listed(pass.grantee.id())
        });
        if picked {
            access
        } else {
            ProfileAccess::Pass(permissions.without(PassPermission::FilingsPublish))
        }
    }

    /// The firm whose draft this is: the holding organization while no
    /// taxpayer owns the profile.
    fn draft_firm(&self) -> Option<&str> {
        match &self.holder {
            Some(AccountHolder::Organization { organization_id }) => Some(organization_id),
            _ => None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_invite_owner(
        &self,
        invite_id: String,
        email: String,
        token_hash: String,
        expires_at: String,
        actor_user_id: String,
        actor_managed_orgs: Vec<String>,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        let Some(organization_id) = self.draft_firm() else {
            return Err(TaxProfileError::InviteState {
                reason: "this profile already has an owner".to_owned(),
            });
        };
        if !actor_managed_orgs.iter().any(|org| org == organization_id) {
            return Err(TaxProfileError::NotHolder);
        }
        if self.is_archived {
            return Err(TaxProfileError::AlreadyArchived);
        }
        if invite_id.trim().is_empty() || self.invite(&invite_id).is_some() {
            return Err(TaxProfileError::InvalidFacts {
                reason: "invite id is missing or already used".to_owned(),
            });
        }
        if token_hash.trim().is_empty() {
            return Err(TaxProfileError::InvalidFacts {
                reason: "invite secret is missing".to_owned(),
            });
        }
        let email =
            normalize_invite_email(&email).map_err(|reason| TaxProfileError::InvalidFacts { reason })?;
        if !instant_before(&occurred_at, &expires_at) {
            return Err(TaxProfileError::InvalidFacts {
                reason: "the invite must end in the future".to_owned(),
            });
        }
        let mut events: Vec<TaxProfileEvent> = self
            .invites
            .iter()
            .filter(|invite| invite.status == InviteStatus::Open)
            .map(|invite| TaxProfileEvent::OwnerInviteWithdrawn {
                invite_id: invite.invite_id.clone(),
                actor_user_id: actor_user_id.clone(),
                occurred_at: occurred_at.clone(),
            })
            .collect();
        events.push(TaxProfileEvent::OwnerInvited {
            invite_id,
            organization_id: organization_id.to_owned(),
            email,
            token_hash,
            expires_at,
            actor_user_id,
            occurred_at,
        });
        Ok(events)
    }

    fn handle_withdraw_owner_invite(
        &self,
        invite_id: String,
        actor_user_id: String,
        actor_managed_orgs: Vec<String>,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        let invite = self.invite(&invite_id).ok_or(TaxProfileError::InviteNotFound)?;
        if !actor_managed_orgs
            .iter()
            .any(|org| org == &invite.organization_id)
        {
            return Err(TaxProfileError::NotHolder);
        }
        if invite.status != InviteStatus::Open {
            return Err(TaxProfileError::InviteState {
                reason: "this invite is no longer open".to_owned(),
            });
        }
        Ok(vec![TaxProfileEvent::OwnerInviteWithdrawn {
            invite_id,
            actor_user_id,
            occurred_at,
        }])
    }

    fn handle_accept_owner_invite(
        &self,
        invite_id: String,
        token_hash: String,
        actor_user_id: String,
        actor_verified_email: String,
        firm_pass_id: String,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        // The secret first: without it, the caller learns nothing.
        let invite = self
            .invite(&invite_id)
            .filter(|invite| !token_hash.is_empty() && invite.token_hash == token_hash)
            .ok_or(TaxProfileError::InviteNotFound)?;
        if invite.status != InviteStatus::Open {
            return Err(TaxProfileError::InviteState {
                reason: "this invite was already used or withdrawn".to_owned(),
            });
        }
        if !instant_before(&occurred_at, &invite.expires_at) {
            return Err(TaxProfileError::InviteState {
                reason: "this invite has expired; ask the firm for a new one".to_owned(),
            });
        }
        if normalize_invite_email(&actor_verified_email).as_deref() != Ok(invite.email.as_str()) {
            return Err(TaxProfileError::InviteState {
                reason: "this invite is for a different email address; sign in with the one the firm invited"
                    .to_owned(),
            });
        }
        if self.draft_firm() != Some(invite.organization_id.as_str()) {
            return Err(TaxProfileError::InviteState {
                reason: "this profile already has an owner".to_owned(),
            });
        }
        if self.is_archived {
            return Err(TaxProfileError::AlreadyArchived);
        }
        if firm_pass_id.trim().is_empty() || self.pass(&firm_pass_id).is_some() {
            return Err(TaxProfileError::InvalidFacts {
                reason: "pass id is missing or already used".to_owned(),
            });
        }
        Ok(vec![TaxProfileEvent::OwnerInviteAccepted {
            invite_id,
            owner_user_id: actor_user_id,
            organization_id: invite.organization_id.clone(),
            firm_pass_id,
            firm_pass_permissions: crate::domain::pass::PassPreset::Full
                .permissions()
                .with_needs()
                .ids(),
            occurred_at,
        }])
    }

    fn handle_end_pass(
        &self,
        pass_id: String,
        actor_user_id: String,
        actor_managed_orgs: Vec<String>,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        let pass = self.open_pass(&pass_id)?;
        let is_owner = self.access_for(&actor_user_id, &[], &occurred_at) == ProfileAccess::Owner;
        let is_grantee = actor_managed_orgs.iter().any(|org| org == pass.grantee.id());
        if !is_owner && !is_grantee {
            return Err(TaxProfileError::NotHolder);
        }
        let status = if pass.status.is_pending() {
            PassStatus::Declined
        } else {
            PassStatus::Revoked
        };
        Ok(vec![TaxProfileEvent::PassEnded {
            pass_id,
            status,
            actor_user_id,
            occurred_at,
        }])
    }

    fn require_identity(&self, identity_hash: &TinIdentityHash) -> Result<(), TaxProfileError> {
        match &self.identity_hash {
            Some(current) if current == identity_hash => Ok(()),
            Some(_) => Err(TaxProfileError::ProofMismatch),
            None => Err(TaxProfileError::NotFound),
        }
    }
}

/// Process-local event store used by domain tests (optimistic concurrency).
#[derive(Clone, Debug, Default)]
pub struct InMemoryTaxProfileStore {
    streams: std::collections::BTreeMap<String, Vec<TaxProfileEvent>>,
    identity_index: std::collections::BTreeMap<String, String>,
}

impl InMemoryTaxProfileStore {
    /// Appends a raw event (tests replaying history the commands no longer
    /// produce).
    #[cfg(test)]
    pub fn push_event(&mut self, profile_id: &str, event: TaxProfileEvent) {
        self.streams.entry(profile_id.to_owned()).or_default().push(event);
    }

    /// The stored events of one stream, oldest first.
    #[cfg(test)]
    pub fn events(&self, profile_id: &str) -> Vec<TaxProfileEvent> {
        self.streams.get(profile_id).cloned().unwrap_or_default()
    }

    pub fn load(&self, profile_id: &str) -> (TaxProfile, u64) {
        let mut state = TaxProfile::new();
        let events = self
            .streams
            .get(profile_id)
            .cloned()
            .unwrap_or_default();
        for event in &events {
            state.apply(event);
        }
        (state, events.len() as u64)
    }

    pub fn execute(
        &mut self,
        profile_id: &str,
        command: TaxProfileCommand,
        expected_revision: u64,
    ) -> Result<(TaxProfile, u64), TaxProfileError> {
        if let TaxProfileCommand::RegisterTaxProfile { tin_hash, .. } = &command {
            if let Some(existing_id) = self.identity_index.get(tin_hash.as_str()) {
                if existing_id != profile_id {
                    let (existing, _) = self.load(existing_id);
                    return Err(TaxProfileError::AlreadyHeld {
                        holder: existing.holder.expect("indexed profile has a holder"),
                    });
                }
            }
        }
        let (state, revision) = self.load(profile_id);
        if revision != expected_revision {
            return Err(TaxProfileError::ConcurrentModification);
        }
        let events = state.handle(command)?;
        if let Some(TaxProfileEvent::TaxProfileRegistered {
            tin_hash,
            profile_id: created_id,
            ..
        }) = events.first()
        {
            self.identity_index
                .insert(tin_hash.as_str().to_owned(), created_id.clone());
        }
        let stream = self.streams.entry(profile_id.to_owned()).or_default();
        stream.extend(events);
        Ok(self.load(profile_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tin::RegistrationKey;
    use crate::domain::tin_identity::{tin_last4, TinIdentityHash};

    const PEPPER: &[u8] = b"pepper";
    const NOW: &str = "2026-02-01T00:00:00Z";

    fn facts(name: &str) -> CloudTaxProfileFacts {
        CloudTaxProfileFacts {
            registered_name: name.to_owned(),
            rdo_code: "039".to_owned(),
            line_of_business: "Software".to_owned(),
            registered_address: "Makati".to_owned(),
            zip_code: "1200".to_owned(),
            phone: "02-1234567".to_owned(),
            email: "owner@example.test".to_owned(),
            taxpayer_type: TaxpayerType::Individual,
            tax_classification: None,
            is_vat_registered: false,
            eopt_tier: None,
            business_start_date: None,
            birth_date: None,
            atc_codes: Vec::new(),
        }
    }

    fn key() -> RegistrationKey {
        RegistrationKey::parse("123456789", "00000").unwrap()
    }

    fn identity() -> TinIdentityHash {
        let key = key();
        TinIdentityHash::compute(PEPPER, &key.tin_root, &key.branch_code)
    }

    fn last4() -> String {
        tin_last4(&key().tin_root)
    }

    fn personal(user: &str) -> AccountHolder {
        AccountHolder::Personal {
            user_id: user.to_owned(),
        }
    }

    fn org(id: &str) -> AccountHolder {
        AccountHolder::Organization {
            organization_id: id.to_owned(),
        }
    }

    fn create(id: &str, holder: AccountHolder, actor: &str, name: &str) -> TaxProfileCommand {
        TaxProfileCommand::RegisterTaxProfile {
            profile_id: id.to_owned(),
            account_id: actor.to_owned(),
            holder,
            facts: facts(name),
            display_name: name.to_owned(),
            tin_hash: identity(),
            tin_last4: last4(),
            branch_code: BranchCode::HEAD_OFFICE.to_owned(),
            actor_user_id: actor.to_owned(),
            occurred_at: "2026-01-01T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn personal_create_is_exclusive_uuid_sot() {
        let mut store = InMemoryTaxProfileStore::default();
        let (state, rev) = store
            .execute("tp-1", create("tp-1", personal("alice"), "alice", "Alice"), 0)
            .unwrap();
        assert_eq!(rev, 1);
        assert_eq!(state.profile_id.as_deref(), Some("tp-1"));
        assert_eq!(state.tin_last4.as_deref(), Some("6789"));
        assert_eq!(state.account_id.as_deref(), Some("alice"));
        assert_eq!(state.branch_code.as_deref(), Some("00000"));
        assert!(!state.is_archived);
        assert_eq!(state.ownership, Some(OwnershipStatus::PersonalExclusive));
        assert_eq!(state.holder, Some(personal("alice")));
        assert_eq!(state.claim_status_for("alice"), ClaimStatus::Owned);
        assert_eq!(state.identity_hash, Some(identity()));
    }

    #[test]
    fn hashed_identity_blocks_second_holder_on_a_different_uuid() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", personal("alice"), "alice", "Alice"), 0)
            .unwrap();
        let err = store
            .execute("tp-2", create("tp-2", personal("bob"), "bob", "Bob"), 0)
            .unwrap_err();
        assert!(matches!(err, TaxProfileError::AlreadyHeld { .. }));
    }

    #[test]
    fn company_may_hold_until_owner_claims() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute(
                "tp-1",
                create("tp-1", org("firm-1"), "bookkeeper", "Client Co"),
                0,
            )
            .unwrap();
        let (state, _) = store
            .execute(
                "tp-1",
                TaxProfileCommand::Claim {
                    claimant: personal("alice"),
                    actor_user_id: "alice".into(),
                    identity_hash: identity(),
                    proof_method: ProofMethod::FakeOrus,
                },
                1,
            )
            .unwrap();
        assert_eq!(state.ownership, Some(OwnershipStatus::PersonalExclusive));
        assert_eq!(state.verified_owner_user_id.as_deref(), Some("alice"));
        assert_eq!(state.verification, VerificationStatus::Verified);
        assert_eq!(state.claim_status_for("alice"), ClaimStatus::Owned);
    }

    fn orgs(list: &[&str]) -> Vec<String> {
        list.iter().map(|id| (*id).to_owned()).collect()
    }

    fn rename(actor: &str, actor_org: Option<&str>) -> TaxProfileCommand {
        TaxProfileCommand::UpdateTaxProfileIdentity {
            display_name: Some("Renamed".into()),
            facts: None,
            expected_updated_at: None,
            actor_user_id: actor.into(),
            actor_orgs: actor_org.map(|org| vec![org.to_owned()]).unwrap_or_default(),
            occurred_at: "2026-02-01T00:00:00Z".into(),
        }
    }

    fn claim_as(user: &str) -> TaxProfileCommand {
        TaxProfileCommand::Claim {
            claimant: personal(user),
            actor_user_id: user.into(),
            identity_hash: identity(),
            proof_method: ProofMethod::FakeOrus,
        }
    }

    fn old_move(to_org: &str, actor: &str) -> TaxProfileEvent {
        TaxProfileEvent::Transferred {
            holder: org(to_org),
            ownership: OwnershipStatus::CompanyManaged,
            actor_user_id: actor.into(),
        }
    }

    #[test]
    fn an_old_move_to_a_firm_replays_as_a_pass_the_owner_controls() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", personal("alice"), "alice", "Alice"), 0)
            .unwrap();
        store.push_event("tp-1", old_move("firm-2", "alice"));
        let (state, revision) = store.load("tp-1");
        // The taxpayer always wins: Alice is still the owner.
        assert_eq!(state.holder, Some(personal("alice")));
        assert_eq!(state.access_for("alice", &[], NOW), ProfileAccess::Owner);
        // The firm works under a Full pass, without publishing.
        let firm = state.access_for("clerk", &orgs(&["firm-2"]), NOW);
        assert_eq!(firm, ProfileAccess::Pass(crate::domain::pass::PassPreset::Full.permissions()));
        assert!(!firm.has(crate::domain::pass::PassPermission::FilingsPublish));
        // The pass id is stable across replays, so Alice can end it.
        let pass_id = legacy_move_pass_id("tp-1", "firm-2", 0);
        assert_eq!(state.passes[0].pass_id, pass_id);
        let (state, _) = store.execute("tp-1", end(&pass_id, "alice", &[]), revision).unwrap();
        assert_eq!(state.access_for("clerk", &orgs(&["firm-2"]), NOW), ProfileAccess::None);
    }

    #[test]
    fn a_firms_draft_gives_full_without_publishing_or_sharing() {
        let mut store = InMemoryTaxProfileStore::default();
        let (state, _) = store
            .execute("tp-1", create("tp-1", org("firm-1"), "bookkeeper", "Client Co"), 0)
            .unwrap();
        let access = state.access_for("clerk", &orgs(&["firm-1"]), NOW);
        assert_eq!(access, ProfileAccess::HoldingOrg);
        assert!(access.has(crate::domain::pass::PassPermission::DraftsSave));
        assert!(!access.has(crate::domain::pass::PassPermission::FilingsPublish));
        assert!(!access.allows(ProfileAction::Share));
        assert_eq!(state.access_for("mallory", &orgs(&["firm-x"]), NOW), ProfileAccess::None);
    }

    #[test]
    fn registering_grants_nothing_once_the_owner_claims() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", org("firm-1"), "bookkeeper", "Client Co"), 0)
            .unwrap();
        store.execute("tp-1", claim_as("alice"), 1).unwrap();
        // The bookkeeper registered it, but Alice holds it now.
        let err = store.execute("tp-1", rename("bookkeeper", Some("firm-1")), 2).unwrap_err();
        assert_eq!(err, TaxProfileError::NotHolder);
        let (state, _) = store.execute("tp-1", rename("alice", None), 2).unwrap();
        assert_eq!(state.display_name.as_deref(), Some("Renamed"));
    }

    #[test]
    fn the_owner_keeps_full_control_after_an_old_move() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", org("firm-1"), "bookkeeper", "Client Co"), 0)
            .unwrap();
        store.execute("tp-1", claim_as("alice"), 1).unwrap();
        store.push_event("tp-1", old_move("firm-2", "alice"));
        // Alice edits; the firm she moved it to edits through its pass; the
        // firm that registered it has nothing.
        store.execute("tp-1", rename("alice", None), 3).unwrap();
        store.execute("tp-1", rename("clerk", Some("firm-2")), 4).unwrap();
        assert_eq!(
            store.execute("tp-1", rename("bookkeeper", Some("firm-1")), 5).unwrap_err(),
            TaxProfileError::NotHolder
        );
        let (state, _) = store.load("tp-1");
        assert_eq!(state.access_for("alice", &[], NOW), ProfileAccess::Owner);
    }

    #[test]
    fn strangers_have_no_access() {
        let mut store = InMemoryTaxProfileStore::default();
        let (state, _) = store
            .execute("tp-1", create("tp-1", personal("alice"), "alice", "Alice"), 0)
            .unwrap();
        assert_eq!(state.access_for("bob", &[], NOW), ProfileAccess::None);
        assert_eq!(state.access_for("bob", &orgs(&["firm-1"]), NOW), ProfileAccess::None);
        assert!(!ProfileAccess::None.allows(ProfileAction::View));
    }

    #[test]
    fn concurrent_creates_fail_on_expected_revision() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute(
                "tp-1",
                create("tp-1", org("firm-1"), "bookkeeper", "Client Co"),
                0,
            )
            .unwrap();
        let err = store
            .execute(
                "tp-1",
                create("tp-1", org("firm-2"), "other", "Other"),
                0,
            )
            .unwrap_err();
        assert_eq!(err, TaxProfileError::ConcurrentModification);
    }

    #[test]
    fn cannot_claim_from_personal_exclusive_holder() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", personal("alice"), "alice", "Alice"), 0)
            .unwrap();
        let err = store
            .execute(
                "tp-1",
                TaxProfileCommand::Claim {
                    claimant: personal("bob"),
                    actor_user_id: "bob".into(),
                    identity_hash: identity(),
                    proof_method: ProofMethod::FakeOrus,
                },
                1,
            )
            .unwrap_err();
        assert_eq!(err, TaxProfileError::ClaimRequiresCompanyHold);
    }

    #[test]
    fn proof_must_match_hashed_identity() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute(
                "tp-1",
                create("tp-1", org("firm-1"), "bookkeeper", "Client Co"),
                0,
            )
            .unwrap();
        let other = RegistrationKey::parse("987654321", "00000").unwrap();
        let bad = TinIdentityHash::compute(PEPPER, &other.tin_root, &other.branch_code);
        let err = store
            .execute(
                "tp-1",
                TaxProfileCommand::Claim {
                    claimant: personal("alice"),
                    actor_user_id: "alice".into(),
                    identity_hash: bad,
                    proof_method: ProofMethod::FakeOrus,
                },
                1,
            )
            .unwrap_err();
        assert_eq!(err, TaxProfileError::ProofMismatch);
    }

    #[test]
    fn patch_is_last_write_wins_on_updated_at() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", personal("alice"), "alice", "Alice"), 0)
            .unwrap();
        let (state, _) = store
            .execute(
                "tp-1",
                TaxProfileCommand::UpdateTaxProfileIdentity {
                    display_name: Some("Alice Corp".into()),
                    facts: None,
                    expected_updated_at: Some("2026-01-01T00:00:00Z".into()),
                    actor_user_id: "alice".into(),
                    actor_orgs: Vec::new(),
                    occurred_at: "2026-01-02T00:00:00Z".into(),
                },
                1,
            )
            .unwrap();
        assert_eq!(state.display_name.as_deref(), Some("Alice Corp"));
        let err = store
            .execute(
                "tp-1",
                TaxProfileCommand::UpdateTaxProfileIdentity {
                    display_name: Some("stale".into()),
                    facts: None,
                    expected_updated_at: Some("2026-01-01T00:00:00Z".into()),
                    actor_user_id: "alice".into(),
                    actor_orgs: Vec::new(),
                    occurred_at: "2026-01-03T00:00:00Z".into(),
                },
                2,
            )
            .unwrap_err();
        assert_eq!(err, TaxProfileError::StaleWrite);
    }

    #[test]
    fn identity_hash_never_changes_on_metadata_patch() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", personal("alice"), "alice", "Alice"), 0)
            .unwrap();
        let (state, _) = store
            .execute(
                "tp-1",
                TaxProfileCommand::UpdateTaxProfileIdentity {
                    display_name: Some("Renamed".into()),
                    facts: Some(facts("Renamed")),
                    expected_updated_at: Some("2026-01-01T00:00:00Z".into()),
                    actor_user_id: "alice".into(),
                    actor_orgs: Vec::new(),
                    occurred_at: "2026-01-02T00:00:00Z".into(),
                },
                1,
            )
            .unwrap();
        assert_eq!(state.identity_hash, Some(identity()));
        assert_eq!(state.tin_last4.as_deref(), Some("6789"));
    }

    #[test]
    fn archive_and_restore_require_the_controller() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", personal("alice"), "alice", "Alice"), 0)
            .unwrap();
        let err = store
            .execute(
                "tp-1",
                TaxProfileCommand::ArchiveTaxProfile {
                    actor_user_id: "bob".into(),
                    actor_orgs: Vec::new(),
                    occurred_at: "2026-01-03T00:00:00Z".into(),
                },
                1,
            )
            .unwrap_err();
        assert_eq!(err, TaxProfileError::NotHolder);
        let (state, _) = store
            .execute(
                "tp-1",
                TaxProfileCommand::ArchiveTaxProfile {
                    actor_user_id: "alice".into(),
                    actor_orgs: Vec::new(),
                    occurred_at: "2026-01-03T00:00:00Z".into(),
                },
                1,
            )
            .unwrap();
        assert!(state.is_archived);
        let err = store
            .execute(
                "tp-1",
                TaxProfileCommand::UpdateTaxProfileIdentity {
                    display_name: Some("nope".into()),
                    facts: None,
                    expected_updated_at: None,
                    actor_user_id: "alice".into(),
                    actor_orgs: Vec::new(),
                    occurred_at: "2026-01-04T00:00:00Z".into(),
                },
                2,
            )
            .unwrap_err();
        assert_eq!(err, TaxProfileError::AlreadyArchived);
        let (state, _) = store
            .execute(
                "tp-1",
                TaxProfileCommand::RestoreTaxProfile {
                    actor_user_id: "alice".into(),
                    actor_orgs: Vec::new(),
                    occurred_at: "2026-01-04T00:00:00Z".into(),
                },
                2,
            )
            .unwrap();
        assert!(!state.is_archived);
    }

    use crate::domain::pass::{PassPreset, PassPermission as P};

    fn firm(id: &str) -> PassGrantee {
        PassGrantee::Organization {
            organization_id: id.to_owned(),
        }
    }

    fn offer(pass_id: &str, to: &str, actor: &str, permissions: PassPermissions) -> TaxProfileCommand {
        TaxProfileCommand::OfferPass {
            pass_id: pass_id.into(),
            grantee: firm(to),
            permissions,
            expires_at: None,
            actor_user_id: actor.into(),
            occurred_at: NOW.into(),
        }
    }

    fn accept(pass_id: &str, actor: &str, managed: &[&str]) -> TaxProfileCommand {
        TaxProfileCommand::AcceptPass {
            pass_id: pass_id.into(),
            actor_user_id: actor.into(),
            actor_managed_orgs: orgs(managed),
            occurred_at: NOW.into(),
        }
    }

    fn end(pass_id: &str, actor: &str, managed: &[&str]) -> TaxProfileCommand {
        TaxProfileCommand::EndPass {
            pass_id: pass_id.into(),
            actor_user_id: actor.into(),
            actor_managed_orgs: orgs(managed),
            occurred_at: NOW.into(),
        }
    }

    fn alice_profile() -> InMemoryTaxProfileStore {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", personal("alice"), "alice", "Alice"), 0)
            .unwrap();
        store
    }

    #[test]
    fn offered_pass_works_only_after_the_firm_accepts() {
        let mut store = alice_profile();
        let (state, _) = store
            .execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions()), 1)
            .unwrap();
        // Waiting for the firm: nothing yet.
        assert_eq!(state.access_for("clerk", &orgs(&["firm-1"]), NOW), ProfileAccess::None);
        // Someone outside the firm cannot accept for it.
        assert_eq!(
            store.execute("tp-1", accept("pass-1", "mallory", &["firm-x"]), 2).unwrap_err(),
            TaxProfileError::NotHolder
        );
        let (state, _) = store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        // Any member of the firm can use it.
        let access = state.access_for("clerk", &orgs(&["firm-1"]), NOW);
        assert_eq!(access, ProfileAccess::Pass(PassPreset::Safe.permissions()));
        assert!(access.allows(ProfileAction::View));
        assert!(access.has(P::DraftsSave));
        assert!(!access.allows(ProfileAction::Edit));
        assert!(!access.allows(ProfileAction::Archive));
        assert!(!access.allows(ProfileAction::Share));
        assert!(!access.has(P::FilingsPublish));
        // The owner keeps full control.
        assert_eq!(state.access_for("alice", &[], NOW), ProfileAccess::Owner);
    }

    #[test]
    fn a_pass_lets_the_firm_edit_only_with_profile_edit() {
        let mut store = alice_profile();
        store
            .execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions()), 1)
            .unwrap();
        store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        assert_eq!(
            store.execute("tp-1", rename("clerk", Some("firm-1")), 3).unwrap_err(),
            TaxProfileError::NotHolder
        );
        store
            .execute(
                "tp-1",
                TaxProfileCommand::ChangePass {
                    pass_id: "pass-1".into(),
                    permissions: PassPreset::Full.permissions(),
                    expires_at: None,
                    actor_user_id: "alice".into(),
                    occurred_at: NOW.into(),
                },
                3,
            )
            .unwrap();
        let (state, _) = store.execute("tp-1", rename("clerk", Some("firm-1")), 4).unwrap();
        assert_eq!(state.display_name.as_deref(), Some("Renamed"));
    }

    #[test]
    fn only_the_owner_shares() {
        let mut store = alice_profile();
        store
            .execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Full.permissions()), 1)
            .unwrap();
        store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        // Even a Full pass cannot pass access on.
        assert_eq!(
            store
                .execute("tp-1", offer("pass-2", "firm-2", "boss", PassPreset::ReadOnly.permissions()), 3)
                .unwrap_err(),
            TaxProfileError::OwnerOnly
        );
        // A firm that holds the profile before the owner claims cannot share either.
        let mut held = InMemoryTaxProfileStore::default();
        held.execute("tp-2", create("tp-2", org("firm-1"), "boss", "Client"), 0)
            .unwrap();
        assert_eq!(
            held.execute("tp-2", offer("pass-9", "firm-2", "boss", PassPreset::ReadOnly.permissions()), 1)
                .unwrap_err(),
            TaxProfileError::OwnerOnly
        );
    }

    #[test]
    fn revoking_ends_access_at_once() {
        let mut store = alice_profile();
        store
            .execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions()), 1)
            .unwrap();
        store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        // A stranger cannot end it.
        assert_eq!(
            store.execute("tp-1", end("pass-1", "mallory", &["firm-x"]), 3).unwrap_err(),
            TaxProfileError::NotHolder
        );
        let (state, _) = store.execute("tp-1", end("pass-1", "alice", &[]), 3).unwrap();
        assert_eq!(state.pass("pass-1").unwrap().status, PassStatus::Revoked);
        assert_eq!(state.access_for("clerk", &orgs(&["firm-1"]), NOW), ProfileAccess::None);
        // Ended passes stay ended.
        assert!(matches!(
            store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 4).unwrap_err(),
            TaxProfileError::PassState { .. }
        ));
        // A new offer to the same firm is fine once the old one has ended.
        store
            .execute("tp-1", offer("pass-2", "firm-1", "alice", PassPreset::ReadOnly.permissions()), 4)
            .unwrap();
    }

    #[test]
    fn one_open_pass_per_firm() {
        let mut store = alice_profile();
        store
            .execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions()), 1)
            .unwrap();
        assert_eq!(
            store
                .execute("tp-1", offer("pass-2", "firm-1", "alice", PassPreset::Full.permissions()), 2)
                .unwrap_err(),
            TaxProfileError::PassAlreadyOpen
        );
    }

    fn request(pass_id: &str, firm_id: &str, actor: &str, managed: &[&str]) -> TaxProfileCommand {
        TaxProfileCommand::RequestPass {
            pass_id: pass_id.into(),
            grantee: firm(firm_id),
            permissions: PassPreset::Safe.permissions(),
            identity_hash: identity(),
            actor_user_id: actor.into(),
            actor_managed_orgs: orgs(managed),
            occurred_at: NOW.into(),
        }
    }

    #[test]
    fn firm_request_needs_the_tin_and_the_owner_approval() {
        let mut store = alice_profile();
        // The firm must manage the workspace it asks for.
        assert_eq!(
            store.execute("tp-1", request("pass-1", "firm-1", "clerk", &[]), 1).unwrap_err(),
            TaxProfileError::NotHolder
        );
        // Wrong TIN: refused.
        let wrong = TinIdentityHash::compute(
            PEPPER,
            &RegistrationKey::parse("987654321", "00000").unwrap().tin_root,
            &RegistrationKey::parse("987654321", "00000").unwrap().branch_code,
        );
        let mut bad = request("pass-1", "firm-1", "boss", &["firm-1"]);
        if let TaxProfileCommand::RequestPass { identity_hash, .. } = &mut bad {
            *identity_hash = wrong;
        }
        assert_eq!(
            store.execute("tp-1", bad, 1).unwrap_err(),
            TaxProfileError::ProofMismatch
        );
        let (state, _) = store
            .execute("tp-1", request("pass-1", "firm-1", "boss", &["firm-1"]), 1)
            .unwrap();
        assert_eq!(state.pass("pass-1").unwrap().status, PassStatus::Requested);
        assert_eq!(state.access_for("boss", &orgs(&["firm-1"]), NOW), ProfileAccess::None);
        // The firm cannot approve its own request.
        let approve = |actor: &str| TaxProfileCommand::ApprovePass {
            pass_id: "pass-1".into(),
            permissions: Some(PassPreset::ReadOnly.permissions()),
            expires_at: None,
            actor_user_id: actor.into(),
            occurred_at: NOW.into(),
        };
        assert_eq!(
            store.execute("tp-1", approve("boss"), 2).unwrap_err(),
            TaxProfileError::OwnerOnly
        );
        // The owner approves, narrowing it to read-only.
        let (state, _) = store.execute("tp-1", approve("alice"), 2).unwrap();
        assert_eq!(
            state.access_for("clerk", &orgs(&["firm-1"]), NOW),
            ProfileAccess::Pass(PassPreset::ReadOnly.permissions())
        );
    }

    #[test]
    fn a_firm_cannot_ask_to_publish() {
        let mut store = alice_profile();
        let mut cmd = request("pass-1", "firm-1", "boss", &["firm-1"]);
        if let TaxProfileCommand::RequestPass { permissions, .. } = &mut cmd {
            *permissions = permissions.with(P::FilingsPublish);
        }
        assert!(matches!(
            store.execute("tp-1", cmd, 1).unwrap_err(),
            TaxProfileError::InvalidFacts { .. }
        ));
    }

    #[test]
    fn publish_is_only_there_when_the_owner_switches_it_on() {
        let mut store = alice_profile();
        let with_publish = PassPreset::Safe.permissions().with(P::FilingsPublish);
        store
            .execute("tp-1", offer("pass-1", "firm-1", "alice", with_publish), 1)
            .unwrap();
        let (state, _) = store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        assert!(state
            .access_for("clerk", &orgs(&["firm-1"]), NOW)
            .has(P::FilingsPublish));
    }

    #[test]
    fn a_pass_stops_at_its_end_date() {
        let mut store = alice_profile();
        let mut cmd = offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions());
        if let TaxProfileCommand::OfferPass { expires_at, .. } = &mut cmd {
            *expires_at = Some("2026-03-01T16:00:00.000Z".into());
        }
        store.execute("tp-1", cmd, 1).unwrap();
        let (state, _) = store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        let members = orgs(&["firm-1"]);
        assert!(state.access_for("clerk", &members, "2026-03-01T15:59:59Z").allows(ProfileAction::View));
        assert_eq!(state.access_for("clerk", &members, "2026-03-01T16:00:00Z"), ProfileAccess::None);
        assert_eq!(state.pass("pass-1").unwrap().status_label("2026-03-02T00:00:00Z"), "expired");
        // An end date in the past is refused.
        let mut past = offer("pass-2", "firm-2", "alice", PassPreset::Safe.permissions());
        if let TaxProfileCommand::OfferPass { expires_at, .. } = &mut past {
            *expires_at = Some("2026-01-01T16:00:00.000Z".into());
        }
        assert!(matches!(
            store.execute("tp-1", past, 3).unwrap_err(),
            TaxProfileError::InvalidFacts { .. }
        ));
    }

    #[test]
    fn the_firm_can_give_a_pass_back_or_decline_an_offer() {
        let mut store = alice_profile();
        store
            .execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions()), 1)
            .unwrap();
        let (state, _) = store.execute("tp-1", end("pass-1", "boss", &["firm-1"]), 2).unwrap();
        assert_eq!(state.pass("pass-1").unwrap().status, PassStatus::Declined);
        store
            .execute("tp-1", offer("pass-2", "firm-1", "alice", PassPreset::Safe.permissions()), 3)
            .unwrap();
        store.execute("tp-1", accept("pass-2", "boss", &["firm-1"]), 4).unwrap();
        let (state, _) = store.execute("tp-1", end("pass-2", "boss", &["firm-1"]), 5).unwrap();
        assert_eq!(state.pass("pass-2").unwrap().status, PassStatus::Revoked);
    }

    #[test]
    fn a_reclaim_ends_every_open_pass() {
        let mut store = alice_profile();
        store
            .execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions()), 1)
            .unwrap();
        store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        store
            .execute("tp-1", offer("pass-2", "firm-2", "alice", PassPreset::Safe.permissions()), 3)
            .unwrap();
        store.push_event(
            "tp-1",
            TaxProfileEvent::Reclaimed {
                holder: personal("alice"),
                ownership: OwnershipStatus::PersonalExclusive,
                verification: VerificationStatus::Verified,
                verified_owner_user_id: "alice".into(),
                proof_method: ProofMethod::FakeOrus,
                actor_user_id: "alice".into(),
            },
        );
        let (state, _) = store.load("tp-1");
        assert_eq!(state.pass("pass-1").unwrap().status, PassStatus::Revoked);
        assert_eq!(state.pass("pass-2").unwrap().status, PassStatus::Declined);
        assert_eq!(state.access_for("clerk", &orgs(&["firm-1"]), NOW), ProfileAccess::None);
    }

    fn change(pass_id: &str, actor: &str, permissions: PassPermissions) -> TaxProfileCommand {
        TaxProfileCommand::ChangePass {
            pass_id: pass_id.into(),
            permissions,
            expires_at: None,
            actor_user_id: actor.into(),
            occurred_at: NOW.into(),
        }
    }

    fn approve(pass_id: &str, actor: &str) -> TaxProfileCommand {
        TaxProfileCommand::ApprovePass {
            pass_id: pass_id.into(),
            permissions: None,
            expires_at: None,
            actor_user_id: actor.into(),
            occurred_at: NOW.into(),
        }
    }

    fn archive(actor: &str) -> TaxProfileCommand {
        TaxProfileCommand::ArchiveTaxProfile {
            actor_user_id: actor.into(),
            actor_orgs: Vec::new(),
            occurred_at: NOW.into(),
        }
    }

    fn is_pass_state(error: TaxProfileError) -> bool {
        matches!(error, TaxProfileError::PassState { .. })
    }

    #[test]
    fn a_firm_cannot_accept_its_own_request() {
        let mut store = alice_profile();
        store.execute("tp-1", request("pass-1", "firm-1", "boss", &["firm-1"]), 1).unwrap();
        assert!(is_pass_state(
            store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap_err()
        ));
    }

    #[test]
    fn each_step_only_fits_its_own_state() {
        let mut store = alice_profile();
        store.execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions()), 1).unwrap();
        // An offer is accepted, not approved.
        assert!(is_pass_state(store.execute("tp-1", approve("pass-1", "alice"), 2).unwrap_err()));
        let mut asked = alice_profile();
        asked.execute("tp-1", request("pass-2", "firm-1", "boss", &["firm-1"]), 1).unwrap();
        // A request is approved or declined, not changed.
        assert!(is_pass_state(
            asked.execute("tp-1", change("pass-2", "alice", PassPreset::Full.permissions()), 2).unwrap_err()
        ));
        // A second request from the same firm while one is open.
        assert_eq!(
            asked.execute("tp-1", request("pass-3", "firm-1", "boss", &["firm-1"]), 2).unwrap_err(),
            TaxProfileError::PassAlreadyOpen
        );
        // An unknown pass id.
        assert_eq!(
            asked.execute("tp-1", approve("nope", "alice"), 2).unwrap_err(),
            TaxProfileError::PassNotFound
        );
        // An ended pass cannot be ended again.
        store.execute("tp-1", end("pass-1", "alice", &[]), 2).unwrap();
        assert!(is_pass_state(store.execute("tp-1", end("pass-1", "alice", &[]), 3).unwrap_err()));
    }

    #[test]
    fn non_owners_learn_nothing_from_owner_steps() {
        let mut store = alice_profile();
        store.execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions()), 1).unwrap();
        // Ownership is checked before any pass state, so these say "owner
        // only", not "already open" / "not found".
        for cmd in [
            offer("pass-2", "firm-1", "mallory", PassPreset::Safe.permissions()),
            approve("pass-1", "mallory"),
            approve("nope", "mallory"),
            change("pass-1", "mallory", PassPreset::Full.permissions()),
        ] {
            assert_eq!(store.execute("tp-1", cmd, 2).unwrap_err(), TaxProfileError::OwnerOnly);
        }
    }

    #[test]
    fn an_expired_offer_cannot_be_accepted() {
        let mut store = alice_profile();
        let mut cmd = offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions());
        if let TaxProfileCommand::OfferPass { expires_at, .. } = &mut cmd {
            *expires_at = Some("2026-03-01T16:00:00.000Z".into());
        }
        store.execute("tp-1", cmd, 1).unwrap();
        let mut late = accept("pass-1", "boss", &["firm-1"]);
        if let TaxProfileCommand::AcceptPass { occurred_at, .. } = &mut late {
            *occurred_at = "2026-03-02T00:00:00Z".into();
        }
        assert!(is_pass_state(store.execute("tp-1", late, 2).unwrap_err()));
    }

    #[test]
    fn an_archived_profile_grants_nothing_and_takes_no_new_access() {
        let mut store = alice_profile();
        store.execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions()), 1).unwrap();
        store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        store.execute("tp-1", offer("pass-2", "firm-2", "alice", PassPreset::Safe.permissions()), 3).unwrap();
        let (state, _) = store.execute("tp-1", archive("alice"), 4).unwrap();
        assert_eq!(state.access_for("clerk", &orgs(&["firm-1"]), NOW), ProfileAccess::None);
        assert!(is_pass_state(store.execute("tp-1", accept("pass-2", "boss", &["firm-2"]), 5).unwrap_err()));
        assert!(is_pass_state(
            store.execute("tp-1", change("pass-1", "alice", PassPreset::Full.permissions()), 5).unwrap_err()
        ));
        assert!(matches!(
            store.execute("tp-1", offer("pass-3", "firm-3", "alice", PassPreset::Safe.permissions()), 5).unwrap_err(),
            TaxProfileError::AlreadyArchived | TaxProfileError::PassState { .. }
        ));
        // The owner can still end passes on an archived profile.
        store.execute("tp-1", end("pass-1", "alice", &[]), 5).unwrap();
    }

    #[test]
    fn only_profile_events_move_the_profile_timestamp() {
        let mut store = alice_profile();
        let (_, _) = store
            .execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions()), 1)
            .unwrap();
        let events = store.events("tp-1");
        assert!(events[0].changes_profile(), "registering changes the profile");
        assert!(!events[1].changes_profile(), "a pass offer does not");
    }

    #[test]
    fn pass_events_replay_from_json() {
        let mut store = alice_profile();
        store
            .execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Safe.permissions()), 1)
            .unwrap();
        let (state, _) = store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        let events = state.handle(end("pass-1", "alice", &[])).unwrap();
        let json = serde_json::to_value(&events[0]).unwrap();
        let back: TaxProfileEvent = serde_json::from_value(json).unwrap();
        assert_eq!(back.event_type(), "tax_profile.pass_ended");
        assert_eq!(back, events[0]);
    }

    #[test]
    fn legacy_created_event_payload_still_applies() {
        let json = serde_json::json!({
            "Created": {
                "profile_id": "tp-legacy",
                "identity_hash": identity(),
                "tin_last4": "6789",
                "display_name": "Legacy",
                "holder": { "Personal": { "user_id": "alice" } },
                "ownership": "PersonalExclusive",
                "verification": "Unverified",
                "facts": facts("Legacy"),
                "actor_user_id": "alice",
                "occurred_at": "2026-01-01T00:00:00Z"
            }
        });
        let event: TaxProfileEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.event_type(), "tax_profile.registered");
        let mut state = TaxProfile::new();
        state.apply(&event);
        assert_eq!(state.profile_id.as_deref(), Some("tp-legacy"));
        assert_eq!(state.account_id.as_deref(), Some("alice"));
        assert_eq!(state.branch_code.as_deref(), Some("00000"));
        assert_eq!(state.identity_hash, Some(identity()));
    }

    // ── Owner invites ───────────────────────────────────────────────────────

    const INVITE_ENDS: &str = "2026-02-15T00:00:00Z";

    fn firm_draft() -> InMemoryTaxProfileStore {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", org("firm-1"), "bookkeeper", "Client Co"), 0)
            .unwrap();
        store
    }

    fn invite(invite_id: &str, email: &str, managed: &[&str]) -> TaxProfileCommand {
        TaxProfileCommand::InviteOwner {
            invite_id: invite_id.into(),
            email: email.into(),
            token_hash: format!("hash-{invite_id}"),
            expires_at: INVITE_ENDS.into(),
            actor_user_id: "bookkeeper".into(),
            actor_managed_orgs: orgs(managed),
            occurred_at: NOW.into(),
        }
    }

    fn accept_invite(invite_id: &str, token_hash: &str, user: &str, email: &str, at: &str) -> TaxProfileCommand {
        TaxProfileCommand::AcceptOwnerInvite {
            invite_id: invite_id.into(),
            token_hash: token_hash.into(),
            actor_user_id: user.into(),
            actor_verified_email: email.into(),
            firm_pass_id: format!("firm-pass-{invite_id}"),
            occurred_at: at.into(),
        }
    }

    fn withdraw(invite_id: &str, managed: &[&str]) -> TaxProfileCommand {
        TaxProfileCommand::WithdrawOwnerInvite {
            invite_id: invite_id.into(),
            actor_user_id: "bookkeeper".into(),
            actor_managed_orgs: orgs(managed),
            occurred_at: NOW.into(),
        }
    }

    fn invite_state(err: TaxProfileError) -> String {
        match err {
            TaxProfileError::InviteState { reason } => reason,
            other => panic!("expected InviteState, got {other:?}"),
        }
    }

    #[test]
    fn only_the_firm_managing_a_draft_can_invite() {
        let mut store = firm_draft();
        assert_eq!(
            store.execute("tp-1", invite("i1", "ana@example.ph", &["firm-x"]), 1).unwrap_err(),
            TaxProfileError::NotHolder
        );
        assert_eq!(
            store.execute("tp-1", invite("i1", "ana@example.ph", &[]), 1).unwrap_err(),
            TaxProfileError::NotHolder
        );
        assert!(matches!(
            store.execute("tp-1", invite("i1", "not-an-email", &["firm-1"]), 1).unwrap_err(),
            TaxProfileError::InvalidFacts { .. }
        ));
        let mut past = invite("i1", "ana@example.ph", &["firm-1"]);
        if let TaxProfileCommand::InviteOwner { expires_at, .. } = &mut past {
            *expires_at = NOW.into();
        }
        assert!(matches!(
            store.execute("tp-1", past, 1).unwrap_err(),
            TaxProfileError::InvalidFacts { .. }
        ));
        let mut no_secret = invite("i1", "ana@example.ph", &["firm-1"]);
        if let TaxProfileCommand::InviteOwner { token_hash, .. } = &mut no_secret {
            *token_hash = " ".into();
        }
        assert!(matches!(
            store.execute("tp-1", no_secret, 1).unwrap_err(),
            TaxProfileError::InvalidFacts { .. }
        ));
        let (state, _) = store
            .execute("tp-1", invite("i1", " Ana@Example.PH ", &["firm-1"]), 1)
            .unwrap();
        let sent = state.invite("i1").unwrap();
        assert_eq!(sent.email, "ana@example.ph");
        assert_eq!(sent.organization_id, "firm-1");
        assert_eq!(sent.status, InviteStatus::Open);
        // An invite id can't be reused.
        assert!(matches!(
            store.execute("tp-1", invite("i1", "ana@example.ph", &["firm-1"]), 2).unwrap_err(),
            TaxProfileError::InvalidFacts { .. }
        ));
    }

    #[test]
    fn a_profile_with_an_owner_cannot_be_invited_for() {
        let mut store = alice_profile();
        let err = store
            .execute("tp-1", invite("i1", "ana@example.ph", &["firm-1"]), 1)
            .unwrap_err();
        assert!(invite_state(err).contains("already has an owner"));
    }

    #[test]
    fn accepting_makes_the_taxpayer_owner_and_keeps_the_firm_on_a_pass() {
        let mut store = firm_draft();
        store.execute("tp-1", invite("i1", "ana@example.ph", &["firm-1"]), 1).unwrap();
        // Without the secret, nothing is revealed.
        assert_eq!(
            store
                .execute("tp-1", accept_invite("i1", "wrong", "ana", "ana@example.ph", NOW), 2)
                .unwrap_err(),
            TaxProfileError::InviteNotFound
        );
        assert_eq!(
            store
                .execute("tp-1", accept_invite("i1", "", "ana", "ana@example.ph", NOW), 2)
                .unwrap_err(),
            TaxProfileError::InviteNotFound
        );
        assert_eq!(
            store
                .execute("tp-1", accept_invite("nope", "hash-i1", "ana", "ana@example.ph", NOW), 2)
                .unwrap_err(),
            TaxProfileError::InviteNotFound
        );
        // Bound to the invited email.
        let err = store
            .execute("tp-1", accept_invite("i1", "hash-i1", "eve", "eve@example.ph", NOW), 2)
            .unwrap_err();
        assert!(invite_state(err).contains("different email"));
        // Expired at the end instant, not a second later.
        let err = store
            .execute("tp-1", accept_invite("i1", "hash-i1", "ana", "ana@example.ph", INVITE_ENDS), 2)
            .unwrap_err();
        assert!(invite_state(err).contains("expired"));
        let (state, _) = store
            .execute(
                "tp-1",
                accept_invite("i1", "hash-i1", "ana", "ANA@example.ph", "2026-02-14T23:59:59Z"),
                2,
            )
            .unwrap();
        assert_eq!(state.holder, Some(personal("ana")));
        assert_eq!(state.ownership, Some(OwnershipStatus::PersonalExclusive));
        assert_eq!(state.access_for("ana", &[], NOW), ProfileAccess::Owner);
        assert_eq!(state.invite("i1").unwrap().status, InviteStatus::Accepted);
        assert_eq!(state.invite("i1").unwrap().decided_by.as_deref(), Some("ana"));
        assert_eq!(state.updated_at.as_deref(), Some("2026-02-14T23:59:59Z"));
        // The firm keeps working under a Full pass without publishing.
        let pass = state.pass("firm-pass-i1").unwrap();
        assert_eq!(pass.status, PassStatus::Active);
        assert_eq!(pass.grantee, firm("firm-1"));
        assert_eq!(pass.created_by, "ana");
        let firm_access = state.access_for("clerk", &orgs(&["firm-1"]), NOW);
        assert_eq!(
            firm_access,
            ProfileAccess::Pass(PassPreset::Full.permissions().with_needs())
        );
        assert!(!firm_access.has(P::FilingsPublish));
        assert!(!firm_access.allows(ProfileAction::Share));
        assert!(!firm_access.allows(ProfileAction::Archive));
        // Used once.
        let err = store
            .execute("tp-1", accept_invite("i1", "hash-i1", "ana", "ana@example.ph", NOW), 3)
            .unwrap_err();
        assert!(invite_state(err).contains("already used"));
        // The owner can end the firm's pass.
        let (state, _) = store.execute("tp-1", end("firm-pass-i1", "ana", &[]), 3).unwrap();
        assert_eq!(state.access_for("clerk", &orgs(&["firm-1"]), NOW), ProfileAccess::None);
    }

    #[test]
    fn a_new_invite_replaces_the_open_one() {
        let mut store = firm_draft();
        store.execute("tp-1", invite("i1", "ana@example.ph", &["firm-1"]), 1).unwrap();
        let events = store
            .load("tp-1")
            .0
            .handle(invite("i2", "ana.cruz@example.ph", &["firm-1"]))
            .unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type(), "tax_profile.owner_invite_withdrawn");
        assert_eq!(events[1].event_type(), "tax_profile.owner_invited");
        let (state, _) = store
            .execute("tp-1", invite("i2", "ana.cruz@example.ph", &["firm-1"]), 2)
            .unwrap();
        assert_eq!(state.invite("i1").unwrap().status, InviteStatus::Withdrawn);
        let err = store
            .execute("tp-1", accept_invite("i1", "hash-i1", "ana", "ana@example.ph", NOW), 4)
            .unwrap_err();
        assert!(invite_state(err).contains("withdrawn"));
    }

    #[test]
    fn only_the_inviting_firm_withdraws_and_only_once() {
        let mut store = firm_draft();
        store.execute("tp-1", invite("i1", "ana@example.ph", &["firm-1"]), 1).unwrap();
        assert_eq!(
            store.execute("tp-1", withdraw("i1", &["firm-x"]), 2).unwrap_err(),
            TaxProfileError::NotHolder
        );
        assert_eq!(
            store.execute("tp-1", withdraw("nope", &["firm-1"]), 2).unwrap_err(),
            TaxProfileError::InviteNotFound
        );
        let (state, _) = store.execute("tp-1", withdraw("i1", &["firm-1"]), 2).unwrap();
        let withdrawn = state.invite("i1").unwrap();
        assert_eq!(withdrawn.status, InviteStatus::Withdrawn);
        assert_eq!(withdrawn.decided_by.as_deref(), Some("bookkeeper"));
        assert!(invite_state(store.execute("tp-1", withdraw("i1", &["firm-1"]), 3).unwrap_err())
            .contains("no longer open"));
    }

    #[test]
    fn a_claim_withdraws_open_invites() {
        let mut store = firm_draft();
        store.execute("tp-1", invite("i1", "ana@example.ph", &["firm-1"]), 1).unwrap();
        let (state, _) = store.execute("tp-1", claim_as("alice"), 2).unwrap();
        assert_eq!(state.invite("i1").unwrap().status, InviteStatus::Withdrawn);
        let err = store
            .execute("tp-1", accept_invite("i1", "hash-i1", "ana", "ana@example.ph", NOW), 3)
            .unwrap_err();
        assert!(invite_state(err).contains("withdrawn"));
    }

    #[test]
    fn an_archived_draft_takes_no_invites() {
        let mut store = firm_draft();
        store.execute("tp-1", invite("i1", "ana@example.ph", &["firm-1"]), 1).unwrap();
        store
            .execute(
                "tp-1",
                TaxProfileCommand::ArchiveTaxProfile {
                    actor_user_id: "bookkeeper".into(),
                    actor_orgs: orgs(&["firm-1"]),
                    occurred_at: NOW.into(),
                },
                2,
            )
            .unwrap();
        assert_eq!(
            store.execute("tp-1", invite("i2", "ana@example.ph", &["firm-1"]), 3).unwrap_err(),
            TaxProfileError::AlreadyArchived
        );
        assert_eq!(
            store
                .execute("tp-1", accept_invite("i1", "hash-i1", "ana", "ana@example.ph", NOW), 3)
                .unwrap_err(),
            TaxProfileError::AlreadyArchived
        );
    }

    #[test]
    fn invite_events_leave_the_profile_timestamp_alone() {
        let mut store = firm_draft();
        let (state, _) = store
            .execute("tp-1", invite("i1", "ana@example.ph", &["firm-1"]), 1)
            .unwrap();
        assert_eq!(state.updated_at.as_deref(), Some("2026-01-01T00:00:00Z"));
        let events = state.handle(withdraw("i1", &["firm-1"])).unwrap();
        assert!(!events[0].changes_profile());
        let events = state
            .handle(accept_invite("i1", "hash-i1", "ana", "ana@example.ph", NOW))
            .unwrap();
        assert!(events[0].changes_profile());
        assert_eq!(events[0].event_type(), "tax_profile.owner_invite_accepted");
        let back: TaxProfileEvent =
            serde_json::from_value(serde_json::to_value(&events[0]).unwrap()).unwrap();
        assert_eq!(back, events[0]);
    }

    // ── Publishing needs both sides ─────────────────────────────────────────

    #[test]
    fn publishing_needs_the_owners_switch_and_the_firms_pick() {
        let mut store = alice_profile();
        let with_publish = PassPreset::Full.permissions().with(P::FilingsPublish);
        store.execute("tp-1", offer("pass-1", "firm-1", "alice", with_publish), 1).unwrap();
        let (state, _) = store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        let member = orgs(&["firm-1"]);
        // The owner switched it on, but the firm has not picked this member.
        let access = state.effective_access("clerk", &member, &[], NOW);
        assert!(!access.has(P::FilingsPublish));
        assert!(access.has(P::DraftsSave));
        // Picked by the firm: publishing works.
        assert!(state
            .effective_access("clerk", &member, &orgs(&["firm-1"]), NOW)
            .has(P::FilingsPublish));
        // Picked by some other firm, or not a member: no.
        assert!(!state
            .effective_access("clerk", &orgs(&["firm-1", "firm-2"]), &orgs(&["firm-2"]), NOW)
            .has(P::FilingsPublish));
        assert!(!state
            .effective_access("clerk", &[], &orgs(&["firm-1"]), NOW)
            .has(P::FilingsPublish));
        // The owner always can.
        assert_eq!(state.effective_access("alice", &[], &[], NOW), ProfileAccess::Owner);
        assert!(ProfileAccess::Owner.has(P::FilingsPublish));
    }

    #[test]
    fn a_firm_pick_cannot_add_publishing_the_owner_did_not_allow() {
        let mut store = alice_profile();
        store
            .execute("tp-1", offer("pass-1", "firm-1", "alice", PassPreset::Full.permissions()), 1)
            .unwrap();
        let (state, _) = store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        let access = state.effective_access("clerk", &orgs(&["firm-1"]), &orgs(&["firm-1"]), NOW);
        assert!(!access.has(P::FilingsPublish));
        assert_eq!(access, state.access_for("clerk", &orgs(&["firm-1"]), NOW));
        // An expired publishing pass doesn't count either.
        let mut store = alice_profile();
        let mut offered = offer(
            "pass-1",
            "firm-1",
            "alice",
            PassPreset::Full.permissions().with(P::FilingsPublish),
        );
        if let TaxProfileCommand::OfferPass { expires_at, .. } = &mut offered {
            *expires_at = Some("2026-03-01T00:00:00Z".into());
        }
        store.execute("tp-1", offered, 1).unwrap();
        let (state, _) = store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        let later = "2026-03-01T00:00:00Z";
        assert_eq!(
            state.effective_access("clerk", &orgs(&["firm-1"]), &orgs(&["firm-1"]), later),
            ProfileAccess::None
        );
    }

    #[test]
    fn a_publisher_pick_on_one_firm_does_not_unlock_another_firms_pass() {
        let mut store = alice_profile();
        store
            .execute(
                "tp-1",
                offer("pass-1", "firm-1", "alice", PassPreset::Full.permissions().with(P::FilingsPublish)),
                1,
            )
            .unwrap();
        store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        store
            .execute("tp-1", offer("pass-2", "firm-2", "alice", PassPreset::Safe.permissions()), 3)
            .unwrap();
        let (state, _) = store.execute("tp-1", accept("pass-2", "boss2", &["firm-2"]), 4).unwrap();
        // A member of both, picked only by firm-2 (whose pass has no publishing).
        let both = orgs(&["firm-1", "firm-2"]);
        assert!(!state.effective_access("clerk", &both, &orgs(&["firm-2"]), NOW).has(P::FilingsPublish));
        assert!(state.effective_access("clerk", &both, &orgs(&["firm-1"]), NOW).has(P::FilingsPublish));
    }

    // ── Form 2303 claims ────────────────────────────────────────────────────

    fn reviewed_reclaim(user: &str) -> TaxProfileCommand {
        TaxProfileCommand::Reclaim {
            actor_user_id: user.into(),
            identity_hash: identity(),
            proof_method: ProofMethod::Form2303Review,
        }
    }

    #[test]
    fn a_reviewed_2303_takes_the_profile_from_whoever_registered_it_first() {
        // Mallory registered Ana's TIN first and gave a firm a pass.
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", personal("mallory"), "mallory", "Squatted"), 0)
            .unwrap();
        store
            .execute("tp-1", offer("pass-1", "firm-1", "mallory", PassPreset::Safe.permissions()), 1)
            .unwrap();
        store.execute("tp-1", accept("pass-1", "boss", &["firm-1"]), 2).unwrap();
        let (state, _) = store.execute("tp-1", reviewed_reclaim("ana"), 3).unwrap();
        assert_eq!(state.holder, Some(personal("ana")));
        assert_eq!(state.verification, VerificationStatus::Verified);
        assert_eq!(state.verified_owner_user_id.as_deref(), Some("ana"));
        assert_eq!(state.access_for("ana", &[], NOW), ProfileAccess::Owner);
        // Mallory and the pass she gave are out.
        assert_eq!(state.access_for("mallory", &[], NOW), ProfileAccess::None);
        assert_eq!(state.access_for("clerk", &orgs(&["firm-1"]), NOW), ProfileAccess::None);
        assert_eq!(state.pass("pass-1").unwrap().status, PassStatus::Revoked);
    }

    #[test]
    fn a_reviewed_2303_takes_a_firm_draft_and_withdraws_its_invite() {
        let mut store = firm_draft();
        store.execute("tp-1", invite("i1", "someone@example.ph", &["firm-1"]), 1).unwrap();
        let (state, _) = store
            .execute(
                "tp-1",
                TaxProfileCommand::Claim {
                    claimant: personal("ana"),
                    actor_user_id: "ana".into(),
                    identity_hash: identity(),
                    proof_method: ProofMethod::Form2303Review,
                },
                2,
            )
            .unwrap();
        assert_eq!(state.holder, Some(personal("ana")));
        assert_eq!(state.invite("i1").unwrap().status, InviteStatus::Withdrawn);
        assert_eq!(state.access_for("clerk", &orgs(&["firm-1"]), NOW), ProfileAccess::None);
    }

    #[test]
    fn every_proof_method_including_a_reviewed_2303_allows_a_reclaim() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", personal("mallory"), "mallory", "Squatted"), 0)
            .unwrap();
        for proof in [ProofMethod::FakeOrus, ProofMethod::Orus, ProofMethod::Form2303Review] {
            let mut command = reviewed_reclaim("ana");
            if let TaxProfileCommand::Reclaim { proof_method, .. } = &mut command {
                *proof_method = proof;
            }
            assert!(store.load("tp-1").0.handle(command).is_ok(), "{proof:?}");
        }
        // The owner can't "reclaim" from themselves.
        assert!(matches!(
            store.load("tp-1").0.handle(reviewed_reclaim("mallory")).unwrap_err(),
            TaxProfileError::InvalidFacts { .. }
        ));
    }
}
