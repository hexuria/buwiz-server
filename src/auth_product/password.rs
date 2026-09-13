//! Thin Spin runtime adapter for product workflows owned by `wasi-auth`.
#![allow(unused_imports)]
#![allow(dead_code)]

use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use sha2::{Digest, Sha256};
#[cfg(feature = "mail-capture")]
use wasi_auth::mail::{EmailKind, Recipient};
#[cfg(feature = "mail-capture")]
use wasi_auth::postgres::outbox::{MailOutboxWorker, PublicBaseUrl};
use wasi_auth::{
    authentication::jwt::JwksDocument,
    authentication::mfa::TotpConfig,
    authentication::passkeys::Attachment as PasskeyAttachment,
    authentication::{Clock, RandomSource},
    context::{AuthenticationAssurance, RequestId, SessionId, UserId},
    http::{AuthenticatedSession, TrustedContextCodec},
    postgres::workflows::{
        Argon2Policy, EmailVerificationError, EmailVerificationRequest,
        EmailVerificationResendRequest as ProductEmailVerificationResendRequest,
        EmailVerificationService, OutboxSealingKey,
        PasswordChangeRequest as ProductPasswordChangeRequest, PasswordLoginError,
        PasswordLoginRequest, PasswordLoginService, PasswordRegistrationError,
        PasswordRegistrationRequest, PasswordRegistrationService,
        PasswordResetCompleteRequest as ProductPasswordResetCompleteRequest, PasswordResetError,
        PasswordResetService, PasswordResetStartRequest as ProductPasswordResetStartRequest,
    },
    postgres::{
        PostgresAuthStore, PostgresStoreError,
        flows::FlowSealingKey,
        management::{
            AdminUserRecord, AuditEventRecord, InvitationRecord, InvitationService,
            ManagementError, MembershipRecord, ORGANIZATION_PERMISSION_CATALOG,
            OrganizationManagementService, RoleRecord,
            UpsertRoleRequest as ProductUpsertRoleRequest,
        },
        mfa::{MfaKeyMaterial, MfaService, MfaServiceError},
        oauth::{
            OAuthFlowService, OAuthProviderService, OAuthProviderServiceError, OAuthServiceConfig,
            OAuthServiceError, PendingOAuthFlow, VerifiedOAuthIdentity,
        },
        organizations::{
            CreateOrganizationRequest, OrganizationError, OrganizationRecord, OrganizationService,
        },
        passkeys::{
            PasskeyConfigurationError, PasskeyService, PasskeyServiceConfig, PasskeyServiceError,
        },
        policy::{
            ActivePolicyBundle, PolicyBundleLoadError, PolicyBundleRecord, PolicyBundleService,
            PolicyBundleServiceError,
        },
        rate_limits::{RateLimitError, RateLimitService},
        sessions::{SessionService, SessionServiceError},
        signing::{SigningKeyRecord, SigningKeyService, SigningKeyServiceError},
        spin::{SpinPostgresError, SpinPostgresTransport},
        tokens::{
            AccessTokenVerifier, JwtKeyRing, RefreshSealingKey, TokenService, TokenServiceConfig,
            TokenServiceError, VerifiedAccessToken,
        },
    },
};

use crate::{
    contracts::{
        AccountSessionListResponse, AccountSessionSummary, AdminUserListResponse, AdminUserSummary,
        AuditEventListResponse, AuditEventSummary, AuthProviderSummary, CapturedMailResponse,
        EmailPasswordLoginRequest, EmailPasswordRegisterRequest, EmailVerificationCompleteRequest,
        InvitationListResponse, InvitationSummary, LoginCompletionResponse, LogoutResponse,
        MembershipListResponse, MembershipSummary, MfaEnrollConfirmResponse,
        MfaEnrollStartResponse, MfaStatusResponse, OrganizationListResponse, OrganizationSummary,
        PasskeyStartResponse, PasswordResetCompleteRequest, PasswordResetStartRequest,
        PasswordResetStartResponse, PolicyVersionListResponse, PolicyVersionSummary,
        RoleListResponse, RoleSummary, SessionView, SigningKeyListResponse,
        SigningKeyRotateResponse, SigningKeySummary, TokenRefreshResponse, TokenVerifyResponse,
    },
    error::{AuthStackError, AuthStackResult},
};

use super::*;

pub async fn register_email_password(
    request: &EmailPasswordRegisterRequest,
    redirect_uri: &str,
) -> AuthStackResult<LoginCompletionResponse> {
    let email_key = email_key(&request.email);
    let service = PasswordRegistrationService::new(
        store().await?,
        RuntimeClock,
        RuntimeRandom,
        argon2_policy().await?,
        outbox_key().await?,
    )
    .with_transactional_mail_config(transactional_mail_config().await?);
    service
        .register(PasswordRegistrationRequest::new(
            format!("register:{email_key}"),
            format!("anonymous:{email_key}"),
            request_id("register")?,
            request.email.clone(),
            request.password.clone(),
            redirect_uri,
        ))
        .await
        .map_err(map_registration_error)?;
    let pending = LoginCompletionResponse {
        authenticated: false,
        redirect_url: "/verify-email/pending".to_owned(),
        session_id: None,
        access_token: None,
        refresh_token: None,
        expires_in_seconds: 0,
    };
    maybe_auto_verify_after_register(request, pending, redirect_uri).await
}

pub async fn enforce_account_rate_limit(
    scope: &str,
    subject: &str,
    maximum_attempts: u64,
    window_seconds: u64,
) -> AuthStackResult<()> {
    let decision = RateLimitService::new(store().await?, RuntimeClock)
        .check(scope, subject, maximum_attempts, window_seconds)
        .await
        .map_err(map_rate_limit_error)?;
    if decision.allowed {
        Ok(())
    } else {
        Err(AuthStackError::RateLimited {
            retry_after_seconds: decision.retry_after_seconds,
        })
    }
}

pub async fn login_email_password(
    request: &EmailPasswordLoginRequest,
    redirect_uri: &str,
) -> AuthStackResult<LoginCompletionResponse> {
    login_email_password_with_dev_pending(request, redirect_uri).await
}

pub async fn complete_email_verification(
    request: &EmailVerificationCompleteRequest,
    redirect_uri: &str,
) -> AuthStackResult<LoginCompletionResponse> {
    let service = EmailVerificationService::new(store().await?, RuntimeClock, RuntimeRandom)
        .with_session_ttl_seconds(session_ttl_seconds().await?)
        .map_err(map_verification_error)?
        .with_bootstrap_system_administrator_emails(bootstrap_system_administrator_emails().await)
        .map_err(map_verification_error)?;
    let receipt = service
        .verify(EmailVerificationRequest::new(
            request.token.clone(),
            request_id("verify-email")?,
            redirect_uri,
        ))
        .await
        .map_err(map_verification_error)?;
    let session_id = receipt.session_id;
    let (access_token, refresh_token, expires_in_seconds) =
        finalize_new_session(&session_id).await?;
    Ok(LoginCompletionResponse {
        authenticated: true,
        redirect_url: receipt.redirect_uri,
        session_id: Some(session_id.into_string()),
        access_token: Some(access_token),
        refresh_token: Some(refresh_token),
        expires_in_seconds,
    })
}

pub async fn resend_email_verification(email: &str, redirect_uri: &str) -> AuthStackResult<()> {
    PasswordRegistrationService::new(
        store().await?,
        RuntimeClock,
        RuntimeRandom,
        argon2_policy().await?,
        outbox_key().await?,
    )
    .with_transactional_mail_config(transactional_mail_config().await?)
    .resend_verification(ProductEmailVerificationResendRequest::new(
        email,
        request_id("verification-resend")?,
        redirect_uri,
    ))
    .await
    .map_err(map_registration_error)?;
    Ok(())
}

pub async fn start_password_reset(
    request: &PasswordResetStartRequest,
    redirect_uri: &str,
) -> AuthStackResult<PasswordResetStartResponse> {
    let service = PasswordResetService::new(
        store().await?,
        RuntimeClock,
        RuntimeRandom,
        argon2_policy().await?,
        outbox_key().await?,
    )
    .with_transactional_mail_config(transactional_mail_config().await?)
    .with_session_ttl_seconds(session_ttl_seconds().await?)
    .map_err(map_password_reset_error)?;
    let receipt = service
        .start(ProductPasswordResetStartRequest::new(
            request.email.clone(),
            redirect_uri,
            request_id("password-reset-start")?,
        ))
        .await
        .map_err(map_password_reset_error)?;
    Ok(PasswordResetStartResponse {
        accepted: receipt.accepted,
        expires_in_seconds: receipt.expires_in_seconds,
    })
}

pub async fn complete_password_reset(
    request: &PasswordResetCompleteRequest,
    redirect_uri: &str,
) -> AuthStackResult<LoginCompletionResponse> {
    let service = PasswordResetService::new(
        store().await?,
        RuntimeClock,
        RuntimeRandom,
        argon2_policy().await?,
        outbox_key().await?,
    )
    .with_transactional_mail_config(transactional_mail_config().await?)
    .with_session_ttl_seconds(session_ttl_seconds().await?)
    .map_err(map_password_reset_error)?;
    let receipt = service
        .complete(ProductPasswordResetCompleteRequest::new(
            request.token.clone(),
            request.password.clone(),
            request_id("password-reset-complete")?,
            redirect_uri,
        ))
        .await
        .map_err(map_password_reset_error)?;
    let session_id = receipt.session_id;
    let (access_token, refresh_token, expires_in_seconds) =
        finalize_new_session(&session_id).await?;
    Ok(LoginCompletionResponse {
        authenticated: true,
        redirect_url: receipt.redirect_uri,
        session_id: Some(session_id.into_string()),
        access_token: Some(access_token),
        refresh_token: Some(refresh_token),
        expires_in_seconds,
    })
}

pub async fn refresh_tokens(
    session_id: Option<&str>,
    refresh_token: &str,
) -> AuthStackResult<TokenRefreshResponse> {
    let session_id = session_id.map(bounded_session_id).transpose()?;
    let pair = token_service()
        .await?
        .refresh(
            session_id.as_ref(),
            refresh_token,
            &request_id("refresh-token")?,
        )
        .await
        .map_err(map_token_error)?;
    let (access_token, refresh_token, expires_in_seconds) = pair.into_parts();
    Ok(TokenRefreshResponse {
        access_token: Some(access_token),
        refresh_token: Some(refresh_token),
        expires_in_seconds,
    })
}

pub async fn verify_access_token(token: &str) -> AuthStackResult<TokenVerifyResponse> {
    let cached = cached_verified_token(token);
    let verified = match token_verification_service()
        .await?
        .verify_cached(token, &request_id("verify-access-token")?, cached.as_ref())
        .await
    {
        Ok(verified) => verified,
        Err(error) => {
            remove_cached_verified_token(token);
            return Err(map_token_error(error));
        }
    };
    cache_verified_token(token, verified.clone());
    Ok(TokenVerifyResponse {
        active: true,
        subject: verified.user_id,
        tenant_id: verified.organization_id,
        session_id: Some(verified.session_id),
        expires_at: verified.expires_at_seconds,
        scopes: verified.permissions,
        role_ids: verified.role_ids,
        policy_revision: verified.policy_revision,
        assurance: verified.assurance,
        system_administrator: verified.system_administrator,
        issued_at_unix_seconds: verified.issued_at_seconds,
    })
}

pub async fn get_jwks() -> AuthStackResult<JwksDocument> {
    Ok(token_service().await?.jwks())
}
