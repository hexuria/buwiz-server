//! Product domain aggregates (independent of wasi-auth and Leptos).
//!
//! Cloud tax-profile ownership is the first Buwiz bounded context. Identity
//! (sessions, orgs, email verification) stays in `wasi-auth`.

#![allow(unused_imports)]

pub mod agent;
pub mod desktop_session;
pub mod oauth_client;
pub mod orus;
pub mod pass;
pub mod queries;
pub mod tax_profile;
pub mod tin;
pub mod tin_identity;
pub mod v1_sync;

pub use agent::{
    agent_effective_permissions, agent_permissions_from_choice, is_agent_token, redirect_uri_matches,
    validate_registration, AgentRegistration, AGENT_ACCESS_PREFIX, AGENT_ACCESS_TTL_SECONDS,
    AGENT_CLIENT_PREFIX, AGENT_CODE_TTL_SECONDS, AGENT_REFRESH_PREFIX,
    AGENT_REFRESH_TTL_SECONDS, AGENT_SCOPE,
};
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
