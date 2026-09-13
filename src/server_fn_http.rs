//! Non-streaming `/api/ui` server-function dispatch.
//!
//! `leptos-wasi` `Handler::handle_with_context` converts responses to a WASI
//! stream. On current wasip3/wit-bindgen that traps (`waitable cannot be used
//! synchronously`) and the browser sees HTTP 500. Collect the body and return
//! a single-frame response instead.

use bytes::Bytes;
use http::header::CONTENT_TYPE;
use http_body_util::BodyExt;
use leptos::prelude::{Owner, provide_context};
use leptos_router::components::provide_server_redirect;
use leptos_router::location::RequestUrl;
use leptos_wasi::__private::ServerWithBody;
use leptos_wasi::response::ResponseOptions;
use leptos_wasi::utils::redirect;
use std::sync::{Arc, Mutex};
use server_fn::ServerFn;
use wasi_auth::context::VerifiedRequestContext;
use wasip3::http::types::{ErrorCode, Response};

use crate::app::{
    AcceptOrganizationInvitation, AssignWorkspaceMemberRole, ChangePassword, ClaimTaxProfile,
    CompleteEmailVerification, CompleteOauthCallback, CompletePasswordReset,
    ConfirmTotpEnrollment, CreateDashboardSecret, CreateOrganization, CreateTaxProfile,
    DeactivateWorkspace, DeleteDashboardQuery, DeleteDashboardResource, DeleteDashboardSecret,
    DeleteDashboardSource, DeleteWorkspaceRole, DevelopmentMailCaptureEnabled,
    DismissDashboardNotification, GetAccountProfile, GetAdminHealth, GetAuthCapabilities,
    GetAuthorizationCapabilities, GetCurrentSession, GetDashboardSnapshot, GetMfaStatus,
    GetPublicProfile, GetWorkspaceSettingsContext, InviteCurrentOrganizationMember,
    InviteWorkspaceMember, LatestDevelopmentMail, LeaveWorkspace, ListAccountSessions,
    ListAdminUsers, ListAuthProviders, ListCurrentOrganizationAudit,
    ListCurrentOrganizationInvitations, ListCurrentOrganizationMembers,
    ListCurrentOrganizationRoles, ListDashboardSecrets, ListOrganizations, ListPolicyVersions,
    ListSigningKeys, ListTaxProfiles, ListWorkspaceAudit, ListWorkspaceInvitations,
    ListWorkspaceMembers, ListWorkspacePermissions, ListWorkspaceRoles, LoginEmailPassword,
    LogoutCurrentSession, MigrateWorkspaceLegacyData, PatchTaxProfile, PublishPolicyVersion,
    ReclaimTaxProfile, RegisterEmailPassword, RemoveWorkspaceMember, RequireAuthenticatedRoute,
    RequireAuthorizedRoute, ResendEmailVerification, ResendWorkspaceInvitation,
    ResolveWorkspaceVaultTarget, RevealDashboardSecret, RevokeAccountSession,
    RevokeWorkspaceInvitation, RotateSigningKey, RunDashboardQuery, SaveAuthProvider,
    SaveDashboardLayout, SaveRedirectAllowlist, SeedDashboardDemos, SelectOrganization,
    StartOauthLogin, StartPasskeyLogin, StartPasskeyRegistration, StartPasswordReset,
    StartTotpEnrollment, TestDashboardHttpSource, TransferTaxProfile, TransferWorkspaceOwnership,
    UpdateAccountProfile, UpdateDashboardNote, UpdateWorkspaceName, UpsertCurrentOrganizationRole,
    UpsertDashboardQuery, UpsertDashboardResource, UpsertDashboardSource, UpsertWorkspaceRole,
    VerifyPasskeyLogin, VerifyPasskeyRegistration, VerifyRecoveryCode, VerifyTotpStepUp,
    WebmcpEnabled,
};

const MAX_BODY_BYTES: usize = 256 * 1024;

#[derive(Clone, Default)]
pub(crate) struct OutgoingUiResponse {
    inner: Arc<Mutex<OutgoingUiParts>>,
}

#[derive(Default)]
struct OutgoingUiParts {
    status: Option<http::StatusCode>,
    headers: http::HeaderMap,
}

impl OutgoingUiResponse {
    pub(crate) fn append_header(&self, key: http::HeaderName, value: http::HeaderValue) {
        if let Ok(mut parts) = self.inner.lock() {
            parts.headers.append(key, value);
        }
    }

    pub(crate) fn set_status(&self, status: http::StatusCode) {
        if let Ok(mut parts) = self.inner.lock() {
            parts.status = Some(status);
        }
    }

    fn take(&self) -> OutgoingUiParts {
        self.inner
            .lock()
            .map(|mut parts| std::mem::take(&mut *parts))
            .unwrap_or_default()
    }
}

type IncomingRequest = http::Request<wasip3::http_compat::IncomingRequestBody>;

type ReqBody<T> = <<T as ServerFn>::Server as ServerWithBody<
    <T as ServerFn>::Error,
    <T as ServerFn>::InputStreamError,
    <T as ServerFn>::OutputStreamError,
>>::ReqBody;

type ResBody<T> = <<T as ServerFn>::Server as ServerWithBody<
    <T as ServerFn>::Error,
    <T as ServerFn>::InputStreamError,
    <T as ServerFn>::OutputStreamError,
>>::ResBody;

pub(crate) async fn serve(req: IncomingRequest) -> Result<Response, ErrorCode> {
    let path = req.uri().path().to_string();
    tracing::debug!(path, "dispatching /api/ui server function");
    let (mut parts, body) = match collect_incoming(req).await {
        Ok(collected) => collected,
        Err(status) => {
            return server_fn_error_response(status, "failed to read request body");
        }
    };

    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    wasi_auth::http::strip_untrusted_auth_metadata(&mut parts.headers);
    if let Err(error) = crate::application::validate_browser_origin(&parts.headers).await {
        return server_fn_error_response(
            error.http_status(),
            "Request origin rejected. Open the app with the same host as AUTH_PUBLIC_BASE_URL (localhost and 127.0.0.1 are interchangeable on loopback).",
        );
    }

    let auth_req = http::Request::from_parts(parts.clone(), body.clone());
    let request_context = match crate::application::trusted_context_from_request(&auth_req).await {
        Ok(context) => context,
        Err(error) => {
            return server_fn_error_response(error.http_status(), "Request rejected.");
        }
    };
    let request_context = if request_context.is_some() {
        request_context
    } else {
        match crate::application::authenticate_ingress(&auth_req).await {
            Ok(context) => context,
            Err(
                crate::error::AuthStackError::AuthRequired
                | crate::error::AuthStackError::InvalidCredentials
                | crate::error::AuthStackError::InvalidToken
                | crate::error::AuthStackError::SessionExpired,
            ) => None,
            Err(error) => {
                return server_fn_error_response(error.http_status(), "Request rejected.");
            }
        }
    };

    let req = http::Request::from_parts(parts, body);
    if let Some(response) = dispatch(&path, req, request_context).await? {
        return Ok(response);
    }

    server_fn_error_response(http::StatusCode::NOT_FOUND, "unknown server function")
}

async fn dispatch(
    path: &str,
    req: http::Request<Bytes>,
    request_context: Option<VerifiedRequestContext>,
) -> Result<Option<Response>, ErrorCode> {
    macro_rules! try_fn {
        ($($ty:ty),+ $(,)?) => {{
            $(
                if path == <$ty as ServerFn>::PATH {
                    return Ok(Some(run_typed::<$ty>(req, request_context).await?));
                }
            )+
            Ok(None)
        }};
    }

    try_fn!(
        ListAuthProviders,
        GetAuthCapabilities,
        RegisterEmailPassword,
        LoginEmailPassword,
        CompleteEmailVerification,
        ResendEmailVerification,
        StartPasswordReset,
        CompletePasswordReset,
        GetCurrentSession,
        GetAccountProfile,
        UpdateAccountProfile,
        GetPublicProfile,
        GetDashboardSnapshot,
        SaveDashboardLayout,
        DismissDashboardNotification,
        UpdateDashboardNote,
        UpsertDashboardSource,
        DeleteDashboardSource,
        CreateDashboardSecret,
        DeleteDashboardSecret,
        RevealDashboardSecret,
        ListDashboardSecrets,
        ResolveWorkspaceVaultTarget,
        SeedDashboardDemos,
        MigrateWorkspaceLegacyData,
        TestDashboardHttpSource,
        UpsertDashboardResource,
        UpsertDashboardQuery,
        DeleteDashboardResource,
        DeleteDashboardQuery,
        RunDashboardQuery,
        DevelopmentMailCaptureEnabled,
        LatestDevelopmentMail,
        RequireAuthenticatedRoute,
        RequireAuthorizedRoute,
        StartPasskeyRegistration,
        VerifyPasskeyRegistration,
        StartPasskeyLogin,
        VerifyPasskeyLogin,
        StartOauthLogin,
        CompleteOauthCallback,
        LogoutCurrentSession,
        SaveAuthProvider,
        SaveRedirectAllowlist,
        ListSigningKeys,
        RotateSigningKey,
        GetAuthorizationCapabilities,
        ChangePassword,
        ListTaxProfiles,
        CreateTaxProfile,
        PatchTaxProfile,
        ClaimTaxProfile,
        ReclaimTaxProfile,
        TransferTaxProfile,
        WebmcpEnabled,
        ListAccountSessions,
        RevokeAccountSession,
        GetMfaStatus,
        StartTotpEnrollment,
        ConfirmTotpEnrollment,
        VerifyTotpStepUp,
        VerifyRecoveryCode,
        ListOrganizations,
        CreateOrganization,
        SelectOrganization,
        ListCurrentOrganizationMembers,
        ListCurrentOrganizationInvitations,
        InviteCurrentOrganizationMember,
        AcceptOrganizationInvitation,
        ListCurrentOrganizationRoles,
        UpsertCurrentOrganizationRole,
        ListCurrentOrganizationAudit,
        GetWorkspaceSettingsContext,
        ListWorkspaceMembers,
        ListWorkspaceInvitations,
        ListWorkspaceRoles,
        ListWorkspaceAudit,
        UpdateWorkspaceName,
        AssignWorkspaceMemberRole,
        RemoveWorkspaceMember,
        InviteWorkspaceMember,
        RevokeWorkspaceInvitation,
        ResendWorkspaceInvitation,
        UpsertWorkspaceRole,
        DeleteWorkspaceRole,
        TransferWorkspaceOwnership,
        LeaveWorkspace,
        DeactivateWorkspace,
        ListWorkspacePermissions,
        ListAdminUsers,
        GetAdminHealth,
        ListPolicyVersions,
        PublishPolicyVersion,
    )
}

async fn run_typed<T>(
    req: http::Request<Bytes>,
    request_context: Option<VerifiedRequestContext>,
) -> Result<Response, ErrorCode>
where
    T: ServerFn + 'static,
    T::Server: ServerWithBody<T::Error, T::InputStreamError, T::OutputStreamError>,
    ReqBody<T>: From<Bytes> + Send + 'static,
    ResBody<T>: http_body::Body + Send + 'static,
    <ResBody<T> as http_body::Body>::Data: Send,
    <ResBody<T> as http_body::Body>::Error: std::fmt::Display,
{
    let (parts, bytes) = req.into_parts();
    let context_parts = parts.clone();
    let server_req = http::Request::from_parts(parts, ReqBody::<T>::from(bytes));
    let res_opts = ResponseOptions::default();
    let outgoing = OutgoingUiResponse::default();
    let owner = Owner::new();
    owner.set();
    let request_url = context_parts
        .uri
        .path_and_query()
        .map_or("/", http::uri::PathAndQuery::as_str);
    provide_context(RequestUrl::new(request_url));
    provide_context(context_parts);
    provide_context(res_opts);
    provide_context(outgoing.clone());
    provide_server_redirect(redirect);
    leptos::nonce::provide_nonce();
    if let Some(context) = request_context {
        wasi_auth::leptos::provide_verified_request_context(context);
    }

    let response = T::run_on_server(server_req).await;
    let extras = outgoing.take();
    let (mut parts, body) = response.into_parts();
    let bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(error) => {
            tracing::error!(error = %error, "failed to collect /api/ui response body");
            return Err(ErrorCode::InternalError(None));
        }
    };
    if let Some(status) = extras.status {
        parts.status = status;
    }
    parts.headers.extend(extras.headers);
    once_bytes_response(parts.status, parts.headers, bytes)
}

const REQUEST_BODY_HEADER: &str = "x-buwiz-request-body";
const REQUEST_BODY_ENC_HEADER: &str = "x-buwiz-request-body-enc";

async fn collect_incoming(
    req: IncomingRequest,
) -> Result<(http::request::Parts, Bytes), http::StatusCode> {
    let (parts, incoming) = req.into_parts();
    if let Some(body) = payload_from_headers(&parts.headers)? {
        // The unused incoming stream stays in the handle waitable set unless
        // we consume and drop it. Later Postgres waits then trap.
        discard_incoming_body(incoming);
        return Ok((parts, body));
    }
    let body = collect_wasi_request_body(incoming, MAX_BODY_BYTES).await?;
    Ok((parts, body))
}

pub(crate) fn discard_incoming_body(mut incoming: wasip3::http_compat::IncomingRequestBody) {
    let Some(wasi_req) = incoming.take_unstarted() else {
        return;
    };
    let (result_writer, result_reader) =
        wasip3::wit_future::new(|| Ok::<(), wasip3::http::types::ErrorCode>(()));
    let (stream, trailers) =
        wasip3::http::types::Request::consume_body(wasi_req, result_reader);
    drop(stream);
    drop(trailers);
    drop(result_writer);
}

/// Browser fetch cannot safely stream a WASI request body on current
/// wasip3/wasmtime (`waitable cannot be used synchronously`). The CSR shell
/// copies small `/api/` POST payloads into this header and sends an empty body.
pub(crate) fn payload_from_headers(
    headers: &http::HeaderMap,
) -> Result<Option<Bytes>, http::StatusCode> {
    let Some(value) = headers.get(REQUEST_BODY_HEADER) else {
        return Ok(None);
    };
    let raw = value.as_bytes();
    if raw.len() > MAX_BODY_BYTES {
        return Err(http::StatusCode::PAYLOAD_TOO_LARGE);
    }
    let encoded = headers
        .get(REQUEST_BODY_ENC_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if encoded.eq_ignore_ascii_case("b64") {
        use base64::Engine as _;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(raw)
            .map_err(|_| http::StatusCode::BAD_REQUEST)?;
        if decoded.len() > MAX_BODY_BYTES {
            return Err(http::StatusCode::PAYLOAD_TOO_LARGE);
        }
        return Ok(Some(Bytes::from(decoded)));
    }
    Ok(Some(Bytes::copy_from_slice(raw)))
}

/// Drain a WASI request body at the top-level `handle` future.
///
/// `http_body::Body::poll_frame` nests `stream.read()` and traps on current
/// wasip3/wasmtime. Prefer `payload_from_headers` so this path is unused for
/// browser `/api/` calls. Native clients that POST a real body still hit this
/// and may 500 until the runtime can read incoming streams.
pub(crate) async fn collect_wasi_request_body(
    mut incoming: wasip3::http_compat::IncomingRequestBody,
    max_bytes: usize,
) -> Result<Bytes, http::StatusCode> {
    let Some(wasi_req) = incoming.take_unstarted() else {
        tracing::error!("incoming request body was already started");
        return Err(http::StatusCode::INTERNAL_SERVER_ERROR);
    };
    let (result_writer, result_reader) =
        wasip3::wit_future::new(|| Ok::<(), wasip3::http::types::ErrorCode>(()));
    let (mut stream, trailers) =
        wasip3::http::types::Request::consume_body(wasi_req, result_reader);
    let mut body = Vec::new();
    loop {
        let (status, chunk) = stream.read(Vec::with_capacity(16 * 1024)).await;
        match status {
            wasip3::wit_bindgen::StreamResult::Complete(_) => {
                if body.len().saturating_add(chunk.len()) > max_bytes {
                    return Err(http::StatusCode::PAYLOAD_TOO_LARGE);
                }
                body.extend_from_slice(&chunk);
            }
            wasip3::wit_bindgen::StreamResult::Dropped => break,
            wasip3::wit_bindgen::StreamResult::Cancelled => {
                return Err(http::StatusCode::BAD_REQUEST);
            }
        }
    }
    drop(trailers);
    drop(result_writer);
    Ok(Bytes::from(body))
}

fn server_fn_error_response(
    status: http::StatusCode,
    message: &'static str,
) -> Result<Response, ErrorCode> {
    let body_text = format!("ServerError|{message}");
    once_bytes_response(
        status,
        {
            let mut headers = http::HeaderMap::new();
            headers.insert(CONTENT_TYPE, http::HeaderValue::from_static("text/plain"));
            headers.insert(
                "serverfnerror",
                http::HeaderValue::from_static("true"),
            );
            headers
        },
        Bytes::from(body_text),
    )
}

pub(crate) fn once_bytes_response(
    status: http::StatusCode,
    headers: http::HeaderMap,
    bytes: Bytes,
) -> Result<Response, ErrorCode> {
    use http_body_util::BodyExt as _;

    let stream = futures::stream::once(async move {
        Ok::<_, std::io::Error>(http_body::Frame::data(bytes))
    });
    let body = http_body_util::StreamBody::new(stream).boxed_unsync();
    let mut builder = http::Response::builder().status(status);
    if let Some(headers_mut) = builder.headers_mut() {
        *headers_mut = headers;
    }
    if builder
        .headers_ref()
        .is_some_and(|headers| !headers.contains_key(CONTENT_TYPE))
    {
        builder = builder.header(CONTENT_TYPE, "application/json");
    }
    let response = builder.body(body).map_err(|error| {
        tracing::error!(error = %error, "failed to build /api/ui response");
        ErrorCode::InternalError(None)
    })?;
    wasip3::http_compat::http_into_wasi_response(response)
}
