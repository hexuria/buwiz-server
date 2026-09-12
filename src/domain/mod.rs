//! Product domain aggregates (independent of wasi-auth and Leptos).
//!
//! Cloud tax-profile ownership is the first Buwiz bounded context. Identity
//! (sessions, orgs, email verification) stays in `wasi-auth`.

#![allow(unused_imports)]

pub mod oauth_client;
pub mod orus;
pub mod tax_profile;
pub mod tin;
pub mod tin_identity;

pub use orus::{
    FakeOrusVerifier, OrusError, OwnershipProof, ProofMethod, TinOwnershipVerifier,
    UnconfiguredOrusVerifier,
};
pub use tax_profile::{
    AccountHolder, ClaimStatus, CloudTaxProfileFacts, OwnershipStatus, TaxProfile,
    TaxProfileCommand, TaxProfileError, TaxProfileEvent, TaxpayerType, VerificationStatus,
};
pub use oauth_client::{
    desktop_redirect_uri_allowed, DESKTOP_CLIENT_ID, DESKTOP_CUSTOM_SCHEME_REDIRECT,
    DESKTOP_LEGACY_CUSTOM_SCHEME_REDIRECT, DESKTOP_SCOPE,
};
pub use tin::{BranchCode, RegistrationKey, TinRoot};
pub use tin_identity::{tin_last4, TinIdentityHash};
