//! Cloud tax-profile aggregate: exclusive ownership of a BIR registration unit.
//!
//! Vocabulary follows hexuria/buwiz-forms `docs/tax-profile/CONTEXT.md`:
//! one Taxpayer (TIN root) has one or more Registration Units (TIN root +
//! branch code). This aggregate is the **cloud control plane** for a single
//! registration unit. Desktop IMAP secrets, PIN, and TOTP are never stored.

use serde::{Deserialize, Serialize};

use crate::domain::orus::{OwnershipProof, ProofMethod};
use crate::domain::tin::RegistrationKey;

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
    Create {
        profile_id: String,
        holder: AccountHolder,
        facts: CloudTaxProfileFacts,
        actor_user_id: String,
    },
    Claim {
        claimant: AccountHolder,
        actor_user_id: String,
        proof: OwnershipProof,
    },
    Reclaim {
        actor_user_id: String,
        proof: OwnershipProof,
    },
    Transfer {
        new_holder: AccountHolder,
        actor_user_id: String,
    },
    UpdateFacts {
        facts: CloudTaxProfileFacts,
        actor_user_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaxProfileEvent {
    Created {
        profile_id: String,
        registration: RegistrationKey,
        holder: AccountHolder,
        ownership: OwnershipStatus,
        verification: VerificationStatus,
        facts: CloudTaxProfileFacts,
        actor_user_id: String,
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
    FactsUpdated {
        facts: CloudTaxProfileFacts,
        actor_user_id: String,
    },
}

impl TaxProfileEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::Created { .. } => "tax_profile.created",
            Self::Claimed { .. } => "tax_profile.claimed",
            Self::Reclaimed { .. } => "tax_profile.reclaimed",
            Self::Transferred { .. } => "tax_profile.transferred",
            Self::FactsUpdated { .. } => "tax_profile.facts_updated",
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
        }
    }
}

/// Exclusive-ownership aggregate for one registration unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaxProfile {
    pub exists: bool,
    pub profile_id: Option<String>,
    pub registration: Option<RegistrationKey>,
    pub holder: Option<AccountHolder>,
    pub ownership: Option<OwnershipStatus>,
    pub verification: VerificationStatus,
    pub facts: Option<CloudTaxProfileFacts>,
    pub verified_owner_user_id: Option<String>,
}

impl Default for TaxProfile {
    fn default() -> Self {
        Self {
            exists: false,
            profile_id: None,
            registration: None,
            holder: None,
            ownership: None,
            verification: VerificationStatus::Unverified,
            facts: None,
            verified_owner_user_id: None,
        }
    }
}

impl TaxProfile {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle(
        &self,
        registration: &RegistrationKey,
        command: TaxProfileCommand,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        match command {
            TaxProfileCommand::Create {
                profile_id,
                holder,
                facts,
                actor_user_id,
            } => self.handle_create(
                registration,
                profile_id,
                holder,
                facts,
                actor_user_id,
            ),
            TaxProfileCommand::Claim {
                claimant,
                actor_user_id,
                proof,
            } => self.handle_claim(registration, claimant, actor_user_id, proof),
            TaxProfileCommand::Reclaim {
                actor_user_id,
                proof,
            } => self.handle_reclaim(registration, actor_user_id, proof),
            TaxProfileCommand::Transfer {
                new_holder,
                actor_user_id,
            } => self.handle_transfer(new_holder, actor_user_id),
            TaxProfileCommand::UpdateFacts {
                facts,
                actor_user_id,
            } => self.handle_update_facts(facts, actor_user_id),
        }
    }

    pub fn apply(&mut self, event: &TaxProfileEvent) {
        match event {
            TaxProfileEvent::Created {
                profile_id,
                registration,
                holder,
                ownership,
                verification,
                facts,
                ..
            } => {
                self.exists = true;
                self.profile_id = Some(profile_id.clone());
                self.registration = Some(registration.clone());
                self.holder = Some(holder.clone());
                self.ownership = Some(*ownership);
                self.verification = *verification;
                self.facts = Some(facts.clone());
                if matches!(holder, AccountHolder::Personal { .. }) {
                    // Personal creator controls filing; ORUS verification is independent.
                }
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
            TaxProfileEvent::FactsUpdated { facts, .. } => {
                self.facts = Some(facts.clone());
            }
        }
    }

    fn handle_create(
        &self,
        registration: &RegistrationKey,
        profile_id: String,
        holder: AccountHolder,
        facts: CloudTaxProfileFacts,
        actor_user_id: String,
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
        let ownership = match &holder {
            AccountHolder::Personal { .. } => OwnershipStatus::PersonalExclusive,
            AccountHolder::Organization { .. } => OwnershipStatus::CompanyManaged,
        };
        Ok(vec![TaxProfileEvent::Created {
            profile_id,
            registration: registration.clone(),
            holder,
            ownership,
            verification: VerificationStatus::Unverified,
            facts,
            actor_user_id,
        }])
    }

    fn handle_claim(
        &self,
        registration: &RegistrationKey,
        claimant: AccountHolder,
        actor_user_id: String,
        proof: OwnershipProof,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        self.require_proof(registration, &proof)?;
        if !matches!(self.ownership, Some(OwnershipStatus::CompanyManaged)) {
            return Err(TaxProfileError::ClaimRequiresCompanyHold);
        }
        if !claimant.is_personal() || claimant.id() != actor_user_id {
            return Err(TaxProfileError::InvalidFacts {
                reason: "only a personal account can claim as the real owner".to_owned(),
            });
        }
        if proof.claimant_user_id != actor_user_id {
            return Err(TaxProfileError::ProofMismatch);
        }
        Ok(vec![TaxProfileEvent::Claimed {
            holder: claimant,
            ownership: OwnershipStatus::PersonalExclusive,
            verification: VerificationStatus::Verified,
            verified_owner_user_id: actor_user_id.clone(),
            proof_method: proof.method,
            actor_user_id,
        }])
    }

    fn handle_reclaim(
        &self,
        registration: &RegistrationKey,
        actor_user_id: String,
        proof: OwnershipProof,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        self.require_proof(registration, &proof)?;
        if proof.claimant_user_id != actor_user_id {
            return Err(TaxProfileError::ProofMismatch);
        }
        let is_recorded_owner = self
            .verified_owner_user_id
            .as_deref()
            .is_some_and(|id| id == actor_user_id);
        let personal_holder_is_actor = matches!(
            &self.holder,
            Some(AccountHolder::Personal { user_id }) if user_id == &actor_user_id
        );
        // Verified owner can take back from a company at any time. A personal
        // holder who already has exclusive control does not need to reclaim.
        if personal_holder_is_actor {
            return Err(TaxProfileError::InvalidFacts {
                reason: "caller already holds exclusive control".to_owned(),
            });
        }
        if !is_recorded_owner && !matches!(proof.method, ProofMethod::FakeOrus | ProofMethod::Orus)
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
            proof_method: proof.method,
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
            AccountHolder::Organization { .. } => {
                // Org membership is authorized in the application layer; the
                // aggregate only forbids transferring *away* from a personal
                // exclusive owner who is not the actor.
                true
            }
        };
        if !actor_is_holder {
            return Err(TaxProfileError::NotHolder);
        }
        if let AccountHolder::Personal { user_id } = current {
            if user_id != &actor_user_id {
                return Err(TaxProfileError::NotHolder);
            }
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

    fn handle_update_facts(
        &self,
        facts: CloudTaxProfileFacts,
        actor_user_id: String,
    ) -> Result<Vec<TaxProfileEvent>, TaxProfileError> {
        if !self.exists {
            return Err(TaxProfileError::NotFound);
        }
        facts.validate()?;
        let Some(holder) = &self.holder else {
            return Err(TaxProfileError::NotFound);
        };
        let allowed = match holder {
            AccountHolder::Personal { user_id } => user_id == &actor_user_id,
            AccountHolder::Organization { .. } => true,
        };
        if !allowed {
            return Err(TaxProfileError::NotHolder);
        }
        Ok(vec![TaxProfileEvent::FactsUpdated {
            facts,
            actor_user_id,
        }])
    }

    fn require_proof(
        &self,
        registration: &RegistrationKey,
        proof: &OwnershipProof,
    ) -> Result<(), TaxProfileError> {
        if &proof.registration != registration {
            return Err(TaxProfileError::ProofMismatch);
        }
        if let Some(current) = &self.registration {
            if current != registration {
                return Err(TaxProfileError::ProofMismatch);
            }
        }
        Ok(())
    }
}

/// Process-local event store used by domain tests (optimistic concurrency).
#[derive(Clone, Debug, Default)]
pub struct InMemoryTaxProfileStore {
    streams: std::collections::BTreeMap<String, Vec<TaxProfileEvent>>,
}

impl InMemoryTaxProfileStore {
    pub fn load(&self, registration: &RegistrationKey) -> (TaxProfile, u64) {
        let mut state = TaxProfile::new();
        let events = self
            .streams
            .get(&registration.stream_id())
            .cloned()
            .unwrap_or_default();
        for event in &events {
            state.apply(event);
        }
        (state, events.len() as u64)
    }

    pub fn execute(
        &mut self,
        registration: &RegistrationKey,
        command: TaxProfileCommand,
        expected_revision: u64,
    ) -> Result<(TaxProfile, u64), TaxProfileError> {
        let (state, revision) = self.load(registration);
        if revision != expected_revision {
            return Err(TaxProfileError::ConcurrentModification);
        }
        let events = state.handle(registration, command)?;
        let stream = self
            .streams
            .entry(registration.stream_id())
            .or_default();
        stream.extend(events);
        Ok(self.load(registration))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::orus::FakeOrusVerifier;
    use crate::domain::TinOwnershipVerifier;

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
        }
    }

    fn key() -> RegistrationKey {
        RegistrationKey::parse("123456789", "00000").unwrap()
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

    fn proof(user: &str) -> OwnershipProof {
        FakeOrusVerifier
            .verify(&key(), user)
            .expect("fake proof")
    }

    #[test]
    fn personal_create_is_exclusive() {
        let mut store = InMemoryTaxProfileStore::default();
        let (state, rev) = store
            .execute(
                &key(),
                TaxProfileCommand::Create {
                    profile_id: "tp-1".into(),
                    holder: personal("alice"),
                    facts: facts("Alice"),
                    actor_user_id: "alice".into(),
                },
                0,
            )
            .unwrap();
        assert_eq!(rev, 1);
        assert_eq!(state.ownership, Some(OwnershipStatus::PersonalExclusive));
        assert_eq!(state.holder, Some(personal("alice")));
    }

    #[test]
    fn second_holder_cannot_create_same_registration_unit() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute(
                &key(),
                TaxProfileCommand::Create {
                    profile_id: "tp-1".into(),
                    holder: personal("alice"),
                    facts: facts("Alice"),
                    actor_user_id: "alice".into(),
                },
                0,
            )
            .unwrap();
        let err = store
            .execute(
                &key(),
                TaxProfileCommand::Create {
                    profile_id: "tp-2".into(),
                    holder: personal("bob"),
                    facts: facts("Bob"),
                    actor_user_id: "bob".into(),
                },
                1,
            )
            .unwrap_err();
        assert!(matches!(err, TaxProfileError::AlreadyHeld { .. }));
    }

    #[test]
    fn company_may_hold_until_owner_claims() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute(
                &key(),
                TaxProfileCommand::Create {
                    profile_id: "tp-1".into(),
                    holder: org("firm-1"),
                    facts: facts("Client Co"),
                    actor_user_id: "bookkeeper".into(),
                },
                0,
            )
            .unwrap();
        let (state, _) = store
            .execute(
                &key(),
                TaxProfileCommand::Claim {
                    claimant: personal("alice"),
                    actor_user_id: "alice".into(),
                    proof: proof("alice"),
                },
                1,
            )
            .unwrap();
        assert_eq!(state.ownership, Some(OwnershipStatus::PersonalExclusive));
        assert_eq!(state.verified_owner_user_id.as_deref(), Some("alice"));
        assert_eq!(state.verification, VerificationStatus::Verified);
    }

    #[test]
    fn verified_owner_can_reclaim_from_company_anytime() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute(
                &key(),
                TaxProfileCommand::Create {
                    profile_id: "tp-1".into(),
                    holder: org("firm-1"),
                    facts: facts("Client Co"),
                    actor_user_id: "bookkeeper".into(),
                },
                0,
            )
            .unwrap();
        store
            .execute(
                &key(),
                TaxProfileCommand::Claim {
                    claimant: personal("alice"),
                    actor_user_id: "alice".into(),
                    proof: proof("alice"),
                },
                1,
            )
            .unwrap();
        store
            .execute(
                &key(),
                TaxProfileCommand::Transfer {
                    new_holder: org("firm-2"),
                    actor_user_id: "alice".into(),
                },
                2,
            )
            .unwrap();
        let (state, _) = store
            .execute(
                &key(),
                TaxProfileCommand::Reclaim {
                    actor_user_id: "alice".into(),
                    proof: proof("alice"),
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
                &key(),
                TaxProfileCommand::Create {
                    profile_id: "tp-1".into(),
                    holder: org("firm-1"),
                    facts: facts("Client Co"),
                    actor_user_id: "bookkeeper".into(),
                },
                0,
            )
            .unwrap();
        let err = store
            .execute(
                &key(),
                TaxProfileCommand::Create {
                    profile_id: "tp-2".into(),
                    holder: org("firm-2"),
                    facts: facts("Other"),
                    actor_user_id: "other".into(),
                },
                0,
            )
            .unwrap_err();
        assert_eq!(err, TaxProfileError::ConcurrentModification);
    }

    #[test]
    fn cannot_claim_from_personal_exclusive_holder() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute(
                &key(),
                TaxProfileCommand::Create {
                    profile_id: "tp-1".into(),
                    holder: personal("alice"),
                    facts: facts("Alice"),
                    actor_user_id: "alice".into(),
                },
                0,
            )
            .unwrap();
        let err = store
            .execute(
                &key(),
                TaxProfileCommand::Claim {
                    claimant: personal("bob"),
                    actor_user_id: "bob".into(),
                    proof: proof("bob"),
                },
                1,
            )
            .unwrap_err();
        assert_eq!(err, TaxProfileError::ClaimRequiresCompanyHold);
    }

    #[test]
    fn proof_must_match_registration_unit() {
        let mut store = InMemoryTaxProfileStore::default();
        store
            .execute(
                &key(),
                TaxProfileCommand::Create {
                    profile_id: "tp-1".into(),
                    holder: org("firm-1"),
                    facts: facts("Client Co"),
                    actor_user_id: "bookkeeper".into(),
                },
                0,
            )
            .unwrap();
        let other = RegistrationKey::parse("987654321", "00000").unwrap();
        let mut bad = proof("alice");
        bad.registration = other;
        let err = store
            .execute(
                &key(),
                TaxProfileCommand::Claim {
                    claimant: personal("alice"),
                    actor_user_id: "alice".into(),
                    proof: bad,
                },
                1,
            )
            .unwrap_err();
        assert_eq!(err, TaxProfileError::ProofMismatch);
    }
}
