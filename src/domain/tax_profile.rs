//! Cloud tax-profile aggregate: exclusive ownership of a BIR registration unit.
//!
//! Vocabulary follows hexuria/buwiz-forms `docs/tax-profile/CONTEXT.md`:
//! one Taxpayer (TIN root) has one or more Registration Units (TIN root +
//! branch code). This aggregate is the **cloud control plane** for a single
//! registration unit. The server source of truth is a **UUID**. Exclusive
//! uniqueness is the hashed TIN identity, never a raw TIN primary key.
//! Desktop IMAP secrets, PIN, TOTP, and mailbox OAuth tokens are never stored.

use serde::{Deserialize, Serialize};

use crate::domain::orus::ProofMethod;
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
        occurred_at: String,
    },
    ArchiveTaxProfile {
        actor_user_id: String,
        occurred_at: String,
    },
    RestoreTaxProfile {
        actor_user_id: String,
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
    Transfer {
        new_holder: AccountHolder,
        actor_user_id: String,
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
}

impl TaxProfileEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::TaxProfileRegistered { .. } => "tax_profile.registered",
            Self::Claimed { .. } => "tax_profile.claimed",
            Self::Reclaimed { .. } => "tax_profile.reclaimed",
            Self::Transferred { .. } => "tax_profile.transferred",
            Self::TaxProfileIdentityUpdated { .. } => "tax_profile.identity_updated",
            Self::TaxProfileArchived { .. } => "tax_profile.archived",
            Self::TaxProfileRestored { .. } => "tax_profile.restored",
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
}

impl std::fmt::Display for TaxProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFacts { reason } => write!(f, "{reason}"),
            Self::AlreadyHeld { holder: _ } => write!(
                f,
                "This TIN and branch is already held. Add a different branch, or claim it if you are the owner."
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
        }
    }
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
            Some(AccountHolder::Organization { .. }) => {
                if self.verified_owner_user_id.as_deref() == Some(actor_user_id) {
                    ClaimStatus::ReadOnly
                } else {
                    ClaimStatus::PendingClaim
                }
            }
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
                occurred_at,
            } => self.handle_update_identity(
                display_name,
                facts,
                expected_updated_at,
                actor_user_id,
                occurred_at,
            ),
            TaxProfileCommand::ArchiveTaxProfile {
                actor_user_id,
                occurred_at,
            } => self.handle_archive(actor_user_id, occurred_at),
            TaxProfileCommand::RestoreTaxProfile {
                actor_user_id,
                occurred_at,
            } => self.handle_restore(actor_user_id, occurred_at),
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
            TaxProfileCommand::Transfer {
                new_holder,
                actor_user_id,
            } => self.handle_transfer(new_holder, actor_user_id),
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
                ..
            }
            | TaxProfileEvent::Reclaimed {
                holder,
                ownership,
                verification,
                verified_owner_user_id,
                ..
            } => {
                self.holder = Some(holder.clone());
                self.ownership = Some(*ownership);
                self.verification = *verification;
                self.verified_owner_user_id = Some(verified_owner_user_id.clone());
            }
            TaxProfileEvent::Transferred {
                holder, ownership, ..
            } => {
                self.holder = Some(holder.clone());
                self.ownership = Some(*ownership);
            }
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
        }
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
        if !is_recorded_owner && !matches!(proof_method, ProofMethod::FakeOrus | ProofMethod::Orus)
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

    fn handle_transfer(
        &self,
        new_holder: AccountHolder,
        actor_user_id: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        let Some(current) = &self.holder else {
            return Err(TaxProfileError::NotFound);
        };
        let actor_is_holder = match current {
            AccountHolder::Personal { user_id } => user_id == &actor_user_id,
            AccountHolder::Organization { .. } => true,
        };
        if !actor_is_holder {
            return Err(TaxProfileError::NotHolder);
        }
        if current.same_as(&new_holder) {
            return Err(TaxProfileError::InvalidFacts {
                reason: "new holder must be different".to_owned(),
            });
        }
        let ownership = match &new_holder {
            AccountHolder::Personal { .. } => OwnershipStatus::PersonalExclusive,
            AccountHolder::Organization { .. } => OwnershipStatus::CompanyManaged,
        };
        Ok(vec![TaxProfileEvent::Transferred {
            holder: new_holder,
            ownership,
            actor_user_id,
        }])
    }

    fn handle_update_identity(
        &self,
        display_name: Option<String>,
        facts: Option<CloudTaxProfileFacts>,
        expected_updated_at: Option<String>,
        actor_user_id: String,
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        self.require_account(&actor_user_id)?;
        if self.is_archived {
            return Err(TaxProfileError::AlreadyArchived);
        }
        if let Some(expected) = expected_updated_at {
            if self.updated_at.as_deref() != Some(expected.as_str()) {
                return Err(TaxProfileError::StaleWrite);
            }
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
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        self.require_account(&actor_user_id)?;
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
        occurred_at: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        self.require_account(&actor_user_id)?;
        if !self.is_archived {
            return Err(TaxProfileError::NotArchived);
        }
        Ok(vec![TaxProfileEvent::TaxProfileRestored {
            actor_user_id,
            occurred_at,
        }])
    }

    fn require_account(&self, actor_user_id: &str) -> Result<(), TaxProfileError> {
        match self.account_id.as_deref() {
            Some(account_id) if account_id == actor_user_id => Ok(()),
            _ => Err(TaxProfileError::NotHolder),
        }
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

    #[test]
    fn verified_owner_can_reclaim_from_company_anytime() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute(
                "tp-1",
                create("tp-1", org("firm-1"), "bookkeeper", "Client Co"),
                0,
            )
            .unwrap();
        store
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
        store
            .execute(
                "tp-1",
                TaxProfileCommand::Transfer {
                    new_holder: org("firm-2"),
                    actor_user_id: "alice".into(),
                },
                2,
            )
            .unwrap();
        let (state, _) = store
            .execute(
                "tp-1",
                TaxProfileCommand::Reclaim {
                    actor_user_id: "alice".into(),
                    identity_hash: identity(),
                    proof_method: ProofMethod::FakeOrus,
                },
                3,
            )
            .unwrap();
        assert_eq!(state.holder, Some(personal("alice")));
        assert_eq!(state.ownership, Some(OwnershipStatus::PersonalExclusive));
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
                    occurred_at: "2026-01-02T00:00:00Z".into(),
                },
                1,
            )
            .unwrap();
        assert_eq!(state.identity_hash, Some(identity()));
        assert_eq!(state.tin_last4.as_deref(), Some("6789"));
    }

    #[test]
    fn archive_and_restore_require_account_id() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute("tp-1", create("tp-1", personal("alice"), "alice", "Alice"), 0)
            .unwrap();
        let err = store
            .execute(
                "tp-1",
                TaxProfileCommand::ArchiveTaxProfile {
                    actor_user_id: "bob".into(),
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
                    occurred_at: "2026-01-04T00:00:00Z".into(),
                },
                2,
            )
            .unwrap();
        assert!(!state.is_archived);
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
}
