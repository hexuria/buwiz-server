//! Development authentication: captured outbox + env-gated verify bypass.

#[cfg(feature = "mail-capture")]
use wasi_auth::mail::{EmailKind, Recipient};
#[cfg(feature = "mail-capture")]
use wasi_auth::postgres::outbox::{MailOutboxWorker, PublicBaseUrl};
use wasi_auth::postgres::workflows::{
    PasswordLoginError, PasswordLoginRequest, PasswordLoginService,
};

use crate::contracts::{
    CapturedMailResponse, EmailPasswordLoginRequest, EmailPasswordRegisterRequest,
    EmailVerificationCompleteRequest, LoginCompletionResponse,
};
use crate::dev_auth::{
    development_verification_controls_visible, same_origin_action_path, token_from_action_url,
};
use crate::error::{AuthStackError, AuthStackResult};

use super::*;

pub async fn development_auth_tools_enabled() -> bool {
    !config_bool("AUTH_PRODUCTION_MODE", false).await
        && config_bool("AUTH_DEV_TOOLS", false).await
        && crate::application::loopback_public_base_url().await
}

pub async fn development_mail_capture_enabled() -> bool {
    development_verification_controls_visible(
        cfg!(feature = "mail-capture"),
        config_bool("AUTH_PRODUCTION_MODE", false).await,
        config_bool("AUTH_DEV_TOOLS", false).await,
        runtime_config_value("AUTH_MAIL_TRANSPORT")
            .await
            .as_deref()
            == Some("capture"),
    ) && crate::application::loopback_public_base_url().await
}

pub async fn development_auto_verify_enabled() -> bool {
    development_mail_capture_enabled().await && config_bool("AUTH_DEV_AUTO_VERIFY", false).await
}

pub async fn require_development_mail_capture() -> AuthStackResult<()> {
    if config_bool("AUTH_PRODUCTION_MODE", false).await {
        return Err(AuthStackError::Forbidden);
    }
    if !config_bool("AUTH_DEV_TOOLS", false).await {
        return Err(AuthStackError::Forbidden);
    }
    crate::application::require_loopback_public_base_url("development auth").await?;
    if !cfg!(feature = "mail-capture") {
        return Err(AuthStackError::configuration(
            "captured mail requires the mail-capture feature",
        ));
    }
    if runtime_config_value("AUTH_MAIL_TRANSPORT")
        .await
        .as_deref()
        != Some("capture")
    {
        return Err(AuthStackError::configuration(
            "captured mail requires AUTH_MAIL_TRANSPORT=capture",
        ));
    }
    Ok(())
}

pub async fn latest_captured_mail(
    recipient: &str,
    message_kind: &str,
) -> AuthStackResult<CapturedMailResponse> {
    require_development_mail_capture().await?;
    #[cfg(feature = "mail-capture")]
    {
        let expected_kind = match message_kind {
            "email-verification" => EmailKind::Verification,
            "password-reset" => EmailKind::PasswordReset,
            "invitation" => EmailKind::Invitation,
            _ => return Err(AuthStackError::validation("message_kind is invalid")),
        };
        let recipient = Recipient::new(recipient.to_owned())
            .map_err(|_| AuthStackError::validation("recipient is invalid"))?;
        let public_base_url = runtime_config_value("AUTH_PUBLIC_BASE_URL")
            .await
            .unwrap_or_else(|| crate::application::DEFAULT_PUBLIC_BASE_URL.to_owned());
        let worker = MailOutboxWorker::new(
            store().await?,
            RuntimeClock,
            RuntimeRandom,
            outbox_key().await?,
            PublicBaseUrl::new(&public_base_url)
                .map_err(|_| AuthStackError::configuration("AUTH_PUBLIC_BASE_URL is invalid"))?,
        );
        let captured = worker
            .latest_delivered_for_development(&recipient, expected_kind)
            .await
            .map_err(|_| AuthStackError::store("captured mail is unavailable"))?
            .ok_or_else(|| AuthStackError::not_found("captured mail was not found"))?;
        let raw_action = captured.action_url().map(ToOwned::to_owned);
        let action_url = raw_action
            .as_deref()
            .and_then(same_origin_action_path)
            .or(raw_action);
        Ok(CapturedMailResponse {
            message_kind: message_kind.to_owned(),
            recipient: captured.recipient().as_str().to_owned(),
            subject: captured.subject().to_owned(),
            body_text: captured.text_body().to_owned(),
            body_html: captured.html_body().map(ToOwned::to_owned),
            action_url,
        })
    }
    #[cfg(not(feature = "mail-capture"))]
    {
        let _ = (recipient, message_kind);
        Err(AuthStackError::configuration(
            "captured mail requires the mail-capture feature",
        ))
    }
}

pub async fn skip_development_email_verification(
    email: &str,
    redirect_uri: &str,
) -> AuthStackResult<LoginCompletionResponse> {
    require_development_mail_capture().await?;
    enforce_account_rate_limit("dev-verify-skip", email, 8, 15 * 60).await?;
    let captured = latest_captured_mail(email, "email-verification").await?;
    let token = captured
        .action_url
        .as_deref()
        .and_then(token_from_action_url)
        .ok_or_else(|| AuthStackError::not_found("captured verification token was not found"))?;
    complete_email_verification(
        &EmailVerificationCompleteRequest {
            token,
            redirect_url: Some(redirect_uri.to_owned()),
        },
        redirect_uri,
    )
    .await
}

pub async fn maybe_auto_verify_after_register(
    request: &EmailPasswordRegisterRequest,
    pending: LoginCompletionResponse,
    redirect_uri: &str,
) -> AuthStackResult<LoginCompletionResponse> {
    if !development_auto_verify_enabled().await {
        return Ok(pending);
    }
    match skip_development_email_verification(&request.email, redirect_uri).await {
        Ok(verified) => Ok(verified),
        Err(_) => Ok(pending),
    }
}

pub async fn account_pending_verification(email: &str) -> AuthStackResult<bool> {
    let rows = crate::store::execute_sql(
        "SELECT status FROM auth_users WHERE normalized_email = ?1 LIMIT 1",
        vec![serde_json::json!(email.trim().to_ascii_lowercase())],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(false);
    };
    Ok(crate::store::row_string(row, "status").as_deref() == Some("pending_verification"))
}

pub async fn login_email_password_with_dev_pending(
    request: &EmailPasswordLoginRequest,
    redirect_uri: &str,
) -> AuthStackResult<LoginCompletionResponse> {
    let service = PasswordLoginService::new(
        store().await?,
        RuntimeClock,
        RuntimeRandom,
        argon2_policy().await?,
    )
    .with_session_ttl_seconds(session_ttl_seconds().await?)
    .map_err(map_login_error)?;
    match service
        .login(PasswordLoginRequest::new(
            request.email.clone(),
            request.password.clone(),
            request_id("login")?,
            redirect_uri,
        ))
        .await
    {
        Ok(receipt) => {
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
        Err(PasswordLoginError::InvalidCredentials) => {
            if development_auth_tools_enabled().await
                && account_pending_verification(&request.email).await?
            {
                return Err(AuthStackError::EmailUnverified);
            }
            Err(AuthStackError::InvalidCredentials)
        }
        Err(error) => Err(map_login_error(error)),
    }
}
