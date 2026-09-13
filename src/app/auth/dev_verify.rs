//! Development verification actions: captured link, copy, skip.

use crate::app::helpers::{
    first_http_url_in_text, query_value_from_url, redirect_browser, selected_action_error,
};
use crate::app::{
    LatestDevelopmentMail, SkipDevelopmentEmailVerification, browser_load,
    development_mail_capture_enabled, get_auth_capabilities, latest_development_mail,
    skip_development_email_verification,
};
use crate::contracts::AuthCapabilities;
use crate::dev_auth::{
    development_verification_controls_visible, same_origin_action_path,
};
use crate::ui::classes::{
    AUTH_INLINE_ERROR, AUTH_SECONDARY, BANNER_SUCCESS, BTN_AUTH_SUBMIT, BUTTON_ROW,
};
use leptos::prelude::*;
#[cfg(feature = "hydrate")]
use leptos::task::spawn_local;

#[cfg(feature = "hydrate")]
use crate::app::copy_text;

#[component]
pub fn DevelopmentVerificationActions(#[prop(into)] email: Signal<String>) -> impl IntoView {
    let capabilities = browser_load(get_auth_capabilities);
    let capture_enabled = browser_load(development_mail_capture_enabled);
    let capture_action = ServerAction::<LatestDevelopmentMail>::new();
    let capture_pending = capture_action.pending();
    let capture_value = capture_action.value();
    let skip_action = ServerAction::<SkipDevelopmentEmailVerification>::new();
    let skip_pending = skip_action.pending();
    let skip_value = skip_action.value();
    let (copied, set_copied) = signal(false);
    let (copy_error, set_copy_error) = signal(None::<String>);

    Effect::new(move |_| {
        if let Some(Ok(response)) = skip_value.get() {
            redirect_browser(&response.redirect_url);
        }
    });
    Effect::new(move |_| {
        if let Some(Ok(message)) = capture_value.get() {
            if let Some(action_url) = message
                .action_url
                .as_deref()
                .and_then(same_origin_action_path)
                .or_else(|| message.action_url.clone())
                .filter(|url| !url.is_empty())
            {
                redirect_browser(&action_url);
            } else if let Some(action_url) = first_http_url_in_text(&message.body_text)
                .and_then(|url| same_origin_action_path(&url).or(Some(url)))
            {
                redirect_browser(&action_url);
            }
        }
    });

    let show_capture = move || {
        matches!(capture_enabled.get(), Some(Ok(true)))
            || capabilities.get().is_some_and(|result| {
                result.is_ok_and(|caps| {
                    development_verification_controls_visible(
                        caps.mail_capture,
                        false,
                        caps.development_tools,
                        caps.mail_capture,
                    )
                })
            })
    };

    view! {
        <Show when=show_capture>
            <div class=BANNER_SUCCESS data-testid="dev-verify-panel">
                <p>
                    <strong>"Email captured on this host."</strong>
                    " Nothing was sent to a real inbox. Use the actions below to verify in this browser."
                </p>
                <div class=BUTTON_ROW>
                    <button
                        type="button"
                        class=BTN_AUTH_SUBMIT
                        data-testid="dev-verify-now"
                        disabled=move || capture_pending.get() || skip_pending.get()
                        on:click=move |_| {
                            set_copied.set(false);
                            set_copy_error.set(None);
                            capture_action.dispatch(LatestDevelopmentMail {
                                recipient: email.get_untracked(),
                                message_kind: "email-verification".to_owned(),
                            });
                        }
                    >
                        {move || if capture_pending.get() { "Opening captured link…" } else { "Verify now" }}
                    </button>
                    <button
                        type="button"
                        class=AUTH_SECONDARY
                        data-testid="dev-copy-link"
                        disabled=move || capture_pending.get() || skip_pending.get()
                        on:click=move |_| {
                            set_copy_error.set(None);
                            #[cfg(feature = "hydrate")]
                            {
                                let email = email.get_untracked();
                                spawn_local(async move {
                                    match latest_development_mail(
                                        email,
                                        "email-verification".to_owned(),
                                    )
                                    .await
                                    {
                                        Ok(message) => {
                                            let path = message
                                                .action_url
                                                .as_deref()
                                                .and_then(same_origin_action_path)
                                                .or(message.action_url)
                                                .unwrap_or_default();
                                            if path.is_empty() {
                                                set_copy_error.set(Some(
                                                    "No captured link is available yet.".to_owned(),
                                                ));
                                                return;
                                            }
                                            let href = web_sys::window()
                                                .and_then(|window| window.location().origin().ok())
                                                .map(|origin| format!("{origin}{path}"))
                                                .unwrap_or(path);
                                            match copy_text(href).await {
                                                Ok(_) => set_copied.set(true),
                                                Err(_) => set_copy_error.set(Some(
                                                    "Could not copy the link.".to_owned(),
                                                )),
                                            }
                                        }
                                        Err(error) => {
                                            set_copy_error.set(Some(error.to_string()));
                                        }
                                    }
                                });
                            }
                        }
                    >
                        {move || if copied.get() { "Copied" } else { "Copy link" }}
                    </button>
                    <button
                        type="button"
                        class=AUTH_SECONDARY
                        data-testid="dev-skip-verify"
                        disabled=move || capture_pending.get() || skip_pending.get()
                        on:click=move |_| {
                            skip_action.dispatch(SkipDevelopmentEmailVerification {
                                email: email.get_untracked(),
                                redirect_url: Some("/dashboard".to_owned()),
                            });
                        }
                    >
                        {move || if skip_pending.get() { "Skipping…" } else { "Skip verification (dev)" }}
                    </button>
                </div>
                <Show when=move || selected_action_error(capture_value.get()).is_some()>
                    <p class=AUTH_INLINE_ERROR>
                        {move || selected_action_error(capture_value.get()).unwrap_or_default()}
                    </p>
                </Show>
                <Show when=move || selected_action_error(skip_value.get()).is_some()>
                    <p class=AUTH_INLINE_ERROR>
                        {move || selected_action_error(skip_value.get()).unwrap_or_default()}
                    </p>
                </Show>
                <Show when=move || copy_error.get().is_some()>
                    <p class=AUTH_INLINE_ERROR>{move || copy_error.get().unwrap_or_default()}</p>
                </Show>
            </div>
        </Show>
        <Show when=move || {
            !show_capture()
                && capabilities.get().is_some_and(|result| {
                    result.is_ok_and(|caps: AuthCapabilities| !caps.mail_capture)
                })
        }>
            <p class=BANNER_SUCCESS>
                "Check your inbox for the one-time verification link. "
                "Resend links must use a reachable AUTH_PUBLIC_BASE_URL, not localhost from a remote client."
            </p>
        </Show>
    }
}

#[component]
pub fn DevelopmentPendingEmailField(email: RwSignal<String>) -> impl IntoView {
    view! {
        <label class="grid gap-2 text-[13px] font-medium text-primary">
            <span>"Email used to register"</span>
            <input
                class="min-h-11 rounded-[10px] border border-border-strong bg-surface px-3 text-sm"
                type="email"
                autocomplete="email"
                data-testid="dev-pending-email"
                prop:value=move || email.get()
                on:input=move |event| email.set(event_target_value(&event))
            />
        </label>
    }
}

pub fn pending_email_from_browser() -> String {
    query_value_from_url("email").unwrap_or_default()
}
