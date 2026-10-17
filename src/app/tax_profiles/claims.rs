//! Ownership claims with Form 2303: the claimant's form and the staff queue.
//!
//! Form 2303 is used here only to prove a TIN is yours; it never decides
//! which forms you file. The browser sends the file in 4 KiB parts (the
//! server runtime cannot take one large body yet).

use crate::app::{
    DecideOwnershipClaim, WithdrawOwnershipClaim, browser_load, list_my_ownership_claims,
    list_ownership_claim_queue, start_ownership_claim, upload_ownership_claim_part,
};
use crate::contracts::{OwnershipClaimListResponse, OwnershipClaimView};
use crate::ui::classes::{
    BANNER_ERROR, BANNER_OK, BTN_PRIMARY, BTN_SECONDARY, FIELD, FIELD_GROUP, INPUT, PANEL,
    RESULT_LINE, SECTION_LABEL,
};
use crate::ui::page_shell;
use leptos::prelude::*;

#[cfg(feature = "hydrate")]
use crate::domain::ownership_claim::{MAX_PARTS, PART_BYTES};

#[island]
pub fn OwnershipClaimPanel() -> impl IntoView {
    let loaded = browser_load(list_my_ownership_claims);
    let claims = RwSignal::new(Vec::<OwnershipClaimView>::new());
    let withdraw = ServerAction::<WithdrawOwnershipClaim>::new();
    let tin = RwSignal::new(String::new());
    let branch = RwSignal::new("00000".to_string());
    let progress = RwSignal::new(None::<String>);
    let message = RwSignal::new(None::<Result<String, String>>);
    let busy = RwSignal::new(false);

    let take = move |value: Option<Result<OwnershipClaimListResponse, server_fn::ServerFnError>>| {
        if let Some(Ok(list)) = value {
            claims.set(list.claims);
        }
    };
    Effect::new(move |_| take(loaded.get()));
    Effect::new(move |_| take(withdraw.value().get()));

    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let Some(file) = chosen_file() else {
            message.set(Some(Err("Choose a photo or scan of your Form 2303.".into())));
            return;
        };
        let (tin_root, branch_code) = (tin.get_untracked(), branch.get_untracked());
        busy.set(true);
        message.set(None);
        leptos::task::spawn_local(async move {
            let result = send_claim(tin_root, branch_code, file, progress).await;
            progress.set(None);
            busy.set(false);
            message.set(Some(result.map(|_| {
                "Sent. Buwiz staff will check it; the file is deleted once they decide.".to_owned()
            })));
            if let Ok(list) = list_my_ownership_claims().await {
                claims.set(list.claims);
            }
        });
    };

    view! {
        <section class=PANEL>
            <p class=SECTION_LABEL>"Prove a TIN is yours"</p>
            <p class=RESULT_LINE>
                "If someone else registered your TIN, or a firm holds a draft for you, upload your Form 2303 (Certificate of Registration). Buwiz staff check every claim. If they approve it, the profile becomes yours and every pass and invite on it ends. Form 2303 is used only for this, never to choose your forms."
            </p>
            <p class=RESULT_LINE>
                "Upload a clean scan or photo (JPEG, PNG or PDF, up to 5 MB). Files marked as made by AI are refused."
            </p>
            <form class=FIELD_GROUP on:submit=submit>
                <label class=FIELD>
                    <span>"TIN (9 digits)"</span>
                    <input class=INPUT required prop:value=move || tin.get()
                        on:input=move |ev| tin.set(event_target_value(&ev)) />
                </label>
                <label class=FIELD>
                    <span>"Branch code"</span>
                    <input class=INPUT prop:value=move || branch.get()
                        on:input=move |ev| branch.set(event_target_value(&ev)) />
                </label>
                <label class=FIELD>
                    <span>"Form 2303"</span>
                    <input class=INPUT id="form-2303-file" type="file" accept="image/jpeg,image/png,application/pdf" required />
                </label>
                <button class=BTN_PRIMARY type="submit" disabled=move || busy.get()>"Send claim"</button>
                {move || progress.get().map(|p| view! { <p class=RESULT_LINE>{p}</p> })}
                {move || message.get().map(|m| match m {
                    Ok(text) => view! { <p class=BANNER_OK>{text}</p> }.into_any(),
                    Err(text) => view! { <p class=BANNER_ERROR>{text}</p> }.into_any(),
                })}
            </form>

            <Show when=move || !claims.get().is_empty()>
                <p class=SECTION_LABEL>"Your claims"</p>
                <ul>
                    <For
                        each=move || claims.get()
                        key=|c| format!("{}{}{}", c.claim_id, c.status, c.updated_at)
                        children=move |c: OwnershipClaimView| {
                            let id = c.claim_id.clone();
                            let open = matches!(c.status.as_str(), "awaiting_file" | "pending");
                            let line = format!(
                                "TIN ····{} · {}{}",
                                c.tin_last4,
                                claim_status_label(&c.status),
                                if c.profile_name.is_empty() { String::new() } else { format!(" · {}", c.profile_name) },
                            );
                            view! {
                                <li class="mb-4 rounded-xl border border-border-subtle p-4">
                                    <p class=RESULT_LINE>{line}</p>
                                    {c.decision_note.clone().map(|note| view! { <p class=RESULT_LINE>{format!("Staff note: {note}")}</p> })}
                                    <Show when=move || open>
                                        <button class=BTN_SECONDARY type="button" on:click={
                                            let id = id.clone();
                                            move |_| { withdraw.dispatch(WithdrawOwnershipClaim { claim_id: id.clone() }); }
                                        }>"Withdraw"</button>
                                    </Show>
                                </li>
                            }
                        }
                    />
                </ul>
            </Show>
        </section>
    }
}

fn claim_status_label(status: &str) -> &'static str {
    match status {
        "awaiting_file" => "waiting for your file",
        "pending" => "waiting for staff",
        "approved" => "approved: the profile is yours",
        "rejected" => "rejected",
        "withdrawn" => "withdrawn",
        _ => "unknown",
    }
}

/// Starts the claim (or reuses an open one for the same TIN) and sends the
/// file in parts.
#[cfg(feature = "hydrate")]
async fn send_claim(
    tin_root: String,
    branch_code: String,
    file: web_sys::File,
    progress: RwSignal<Option<String>>,
) -> Result<(), String> {
    use base64::Engine as _;
    let bytes = read_file(&file).await?;
    if bytes.is_empty() {
        return Err("The file is empty.".into());
    }
    if bytes.len() > MAX_PARTS * PART_BYTES {
        return Err("The file is larger than 5 MB.".into());
    }
    let claim = start_ownership_claim(tin_root, branch_code)
        .await
        .map_err(|e| e.to_string())?;
    let parts: Vec<&[u8]> = bytes.chunks(PART_BYTES).collect();
    let total = parts.len() as u32;
    for (index, part) in parts.iter().enumerate() {
        progress.set(Some(format!("Sending part {} of {total}…", index + 1)));
        upload_ownership_claim_part(
            claim.claim_id.clone(),
            index as u32,
            total,
            base64::engine::general_purpose::STANDARD.encode(part),
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(not(feature = "hydrate"))]
async fn send_claim(
    _tin_root: String,
    _branch_code: String,
    _file: (),
    _progress: RwSignal<Option<String>>,
) -> Result<(), String> {
    Err("uploads run in the browser".into())
}

#[cfg(feature = "hydrate")]
fn chosen_file() -> Option<web_sys::File> {
    use wasm_bindgen::JsCast;
    web_sys::window()?
        .document()?
        .get_element_by_id("form-2303-file")?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()?
        .files()?
        .get(0)
}

#[cfg(not(feature = "hydrate"))]
fn chosen_file() -> Option<()> {
    None
}

#[cfg(feature = "hydrate")]
async fn read_file(file: &web_sys::File) -> Result<Vec<u8>, String> {
    let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
        .await
        .map_err(|_| "Could not read the file.".to_owned())?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

// ── Staff ───────────────────────────────────────────────────────────────────

#[component]
pub fn AdminOwnershipClaimsPage() -> impl IntoView {
    page_shell(
        "Ownership claims",
        "Check each uploaded Form 2303 against the profile's TIN and the claimant. Files are deleted when you decide.",
        view! { <OwnershipClaimQueue /> },
    )
}

#[island]
pub fn OwnershipClaimQueue() -> impl IntoView {
    let loaded = browser_load(list_ownership_claim_queue);
    let decide = ServerAction::<DecideOwnershipClaim>::new();
    let queue = RwSignal::new(Vec::<OwnershipClaimView>::new());
    let load_error = RwSignal::new(None::<String>);
    Effect::new(move |_| match loaded.get() {
        Some(Ok(list)) => queue.set(list.claims),
        Some(Err(error)) => load_error.set(Some(error.to_string())),
        None => {}
    });
    Effect::new(move |_| {
        if let Some(Ok(list)) = decide.value().get() {
            queue.set(list.claims);
        }
    });
    let error = move || {
        decide
            .value()
            .get()
            .and_then(Result::err)
            .map(|e| e.to_string())
            .or_else(|| load_error.get())
    };

    view! {
        <section class=PANEL>
            {move || error().map(|e| view! { <p class=BANNER_ERROR>{e}</p> })}
            <Show when=move || queue.get().is_empty()>
                <p class=RESULT_LINE>"Nothing waiting."</p>
            </Show>
            <ul>
                <For
                    each=move || queue.get()
                    key=|c| format!("{}{}", c.claim_id, c.updated_at)
                    children=move |c: OwnershipClaimView| {
                        let note = RwSignal::new(String::new());
                        let id = StoredValue::new(c.claim_id.clone());
                        let file_href = format!("/api/admin/ownership-claims/{}/file", c.claim_id);
                        let facts = format!(
                            "{} · claims {} ····{} · {} {}{}{}",
                            c.claimant_email.clone().unwrap_or_default(),
                            c.profile_name,
                            c.tin_last4,
                            c.file_kind.clone().unwrap_or_default().to_uppercase(),
                            c.file_size.map(|size| format!("{} KB", size.div_ceil(1024))).unwrap_or_default(),
                            if c.has_c2pa { " · has a C2PA manifest (not AI-labelled)" } else { "" },
                            if c.ai_refusals > 0 {
                                format!(" · {} earlier upload(s) refused as AI-made", c.ai_refusals)
                            } else {
                                String::new()
                            },
                        );
                        let hash = c.file_sha256.clone().unwrap_or_default();
                        view! {
                            <li class="mb-4 rounded-xl border border-border-subtle p-4">
                                <p class=RESULT_LINE>{facts}</p>
                                <p class=RESULT_LINE>{format!("SHA-256 {hash}")}</p>
                                <a class=BTN_SECONDARY href=file_href>"Download the file"</a>
                                <label class=FIELD>
                                    <span>"Note for the claimant (needed to reject)"</span>
                                    <input class=INPUT prop:value=move || note.get()
                                        on:input=move |ev| note.set(event_target_value(&ev)) />
                                </label>
                                <div class="mt-2 flex flex-wrap gap-2">
                                    <button class=BTN_PRIMARY type="button" disabled=move || decide.pending().get()
                                        on:click=move |_| {
                                            let text = note.get_untracked();
                                            decide.dispatch(DecideOwnershipClaim {
                                                claim_id: id.get_value(),
                                                approve: true,
                                                note: (!text.trim().is_empty()).then_some(text),
                                            });
                                        }>"Approve: they own it"</button>
                                    <button class=BTN_SECONDARY type="button" disabled=move || decide.pending().get()
                                        on:click=move |_| {
                                            decide.dispatch(DecideOwnershipClaim {
                                                claim_id: id.get_value(),
                                                approve: false,
                                                note: Some(note.get_untracked()),
                                            });
                                        }>"Reject"</button>
                                </div>
                            </li>
                        }
                    }
                />
            </ul>
        </section>
    }
}
