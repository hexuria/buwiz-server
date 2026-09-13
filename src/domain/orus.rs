//! ORUS TIN-ownership verification port.
//!
//! Production ORUS is out of scope for v1. The application wires
//! [`FakeOrusVerifier`] when `ORUS_FAKE_VERIFIER=true` (local/dev) and
//! otherwise returns [`OrusError::NotConfigured`].

use serde::{Deserialize, Serialize};

use crate::domain::tin::RegistrationKey;

/// Evidence that a principal proved control of a registration unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnershipProof {
    pub method: ProofMethod,
    pub registration: RegistrationKey,
    pub claimant_user_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofMethod {
    /// Dev/test verifier behind `ORUS_FAKE_VERIFIER`.
    FakeOrus,
    /// Reserved for the live ORUS adapter.
    Orus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrusError {
    NotConfigured,
    Rejected { reason: String },
}

impl std::fmt::Display for OrusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured => write!(
                f,
                "ORUS TIN verification is not configured (enable ORUS_FAKE_VERIFIER for local)"
            ),
            Self::Rejected { reason } => write!(f, "TIN ownership was not verified: {reason}"),
        }
    }
}

pub trait TinOwnershipVerifier {
    fn verify(
        &self,
        registration: &RegistrationKey,
        claimant_user_id: &str,
    ) -> Result<OwnershipProof, OrusError>;
}

/// Always-succeed local verifier. Never use in production.
#[derive(Clone, Debug, Default)]
pub struct FakeOrusVerifier;

impl TinOwnershipVerifier for FakeOrusVerifier {
    fn verify(
        &self,
        registration: &RegistrationKey,
        claimant_user_id: &str,
    ) -> Result<OwnershipProof, OrusError> {
        if claimant_user_id.trim().is_empty() {
            return Err(OrusError::Rejected {
                reason: "claimant is required".to_owned(),
            });
        }
        Ok(OwnershipProof {
            method: ProofMethod::FakeOrus,
            registration: registration.clone(),
            claimant_user_id: claimant_user_id.to_owned(),
        })
    }
}

/// Production placeholder — fails closed until a live adapter exists.
#[derive(Clone, Debug, Default)]
pub struct UnconfiguredOrusVerifier;

impl TinOwnershipVerifier for UnconfiguredOrusVerifier {
    fn verify(
        &self,
        _registration: &RegistrationKey,
        _claimant_user_id: &str,
    ) -> Result<OwnershipProof, OrusError> {
        Err(OrusError::NotConfigured)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tin::RegistrationKey;

    #[test]
    fn fake_verifier_issues_proof() {
        let key = RegistrationKey::parse("123456789", "00000").unwrap();
        let proof = FakeOrusVerifier
            .verify(&key, "user-1")
            .expect("fake verifier");
        assert_eq!(proof.method, ProofMethod::FakeOrus);
        assert_eq!(proof.claimant_user_id, "user-1");
    }

    #[test]
    fn unconfigured_verifier_fails_closed() {
        let key = RegistrationKey::parse("123456789", "00000").unwrap();
        assert!(matches!(
            UnconfiguredOrusVerifier.verify(&key, "user-1"),
            Err(OrusError::NotConfigured)
        ));
    }
}
