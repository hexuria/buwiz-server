//! Product domain aggregates (independent of wasi-auth and Leptos).
//!
//! Cloud tax-profile ownership is the first Buwiz bounded context. Identity
//! (sessions, orgs, email verification) stays in `wasi-auth`.

#![allow(unused_imports)]

pub mod desktop_session;
pub mod oauth_client;
pub mod orus;
pub mod pass;
pub mod queries;
pub mod tax_profile;
pub mod tin;
pub mod tin_identity;
pub mod v1_sync;

pub use desktop_session::{
    DesktopGrant, DesktopSessionCommand, DesktopSessionError, DesktopSessionEvent,
};
pub use orus::{
    FakeOrusVerifier, OrusError, OwnershipProof, ProofMethod, TinOwnershipVerifier,
    UnconfiguredOrusVerifier,
};
pub use pass::{
    instant_before, pass_end_from_date, Pass, PassGrantee, PassPermission, PassPermissions,
    PassPreset, PassStatus,
};
pub use queries::TaxProfileQuery;
pub use tax_profile::{
    AccountHolder, ClaimStatus, CloudTaxProfileFacts, OwnershipStatus, ProfileAccess,
    ProfileAction, TaxProfile, TaxProfileCommand, TaxProfileError, TaxProfileEvent, TaxpayerType, VerificationStatus,
};
pub use oauth_client::{
    desktop_redirect_uri_allowed, DESKTOP_CLIENT_ID, DESKTOP_CUSTOM_SCHEME_REDIRECT,
    DESKTOP_LEGACY_CUSTOM_SCHEME_REDIRECT, DESKTOP_SCOPE,
};
pub use tin::{BranchCode, RegistrationKey, TinRoot};
pub use tin_identity::{tin_last4, TinIdentityHash};
pub use v1_sync::{SyncCommand, SyncError, SyncEvent, YearFormEntry};
