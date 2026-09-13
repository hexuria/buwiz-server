//! Cloud tax-profile list and exclusive-ownership actions.

#![allow(unused_imports)]
#![allow(clippy::unused_unit)]
#![allow(clippy::unit_arg)]

use crate::app::{
    ClaimTaxProfile, CreateTaxProfile, ListTaxProfiles, ReclaimTaxProfile,
    browser_load, get_current_session, list_organizations, list_tax_profiles,
};
use crate::contracts::{OrganizationListResponse, TaxProfileListResponse};
use crate::ui::classes::{
    BANNER_ERROR, BANNER_SUCCESS, BTN_PRIMARY, BTN_SECONDARY, FIELD, FIELD_GROUP, INPUT,
    ORG_KICKER, ORG_PAGE, ORG_TOOLBAR, ORG_TOOLBAR_COPY, ORG_TOOLBAR_SUB, ORG_TOOLBAR_TITLE,
    PANEL, RESULT_LINE, SECTION_LABEL,
};
use crate::ui::page_shell;
use leptos::attr::custom::custom_attribute;
use leptos::prelude::*;
use server_fn::ServerFnError;

#[component]
pub fn TaxProfilesPage() -> impl IntoView {
    page_shell(
        "Tax profiles",
        "Exclusive cloud control of a BIR registration unit. Server identity is a UUID; TIN is attested and hashed. Desktop secrets stay on the device.",
        view! { <TaxProfilesHome /> },
    )
}

#[island]
pub fn TaxProfilesHome() -> impl IntoView {
    let profiles = browser_load(|| list_tax_profiles(None));
    let session = browser_load(get_current_session);
    let orgs = browser_load(list_organizations);
    let create = ServerAction::<CreateTaxProfile>::new();
    let claim = ServerAction::<ClaimTaxProfile>::new();
    let reclaim = ServerAction::<ReclaimTaxProfile>::new();
    let create_pending = create.pending();
    let create_value = create.value();
    let claim_value = claim.value();
    let reclaim_value = reclaim.value();
    let tin_root = RwSignal::new(String::new());
    let branch_code = RwSignal::new("00000".to_string());
    let registered_name = RwSignal::new(String::new());
    let rdo_code = RwSignal::new(String::new());
    let taxpayer_type = RwSignal::new("individual".to_string());
    let hold_org = RwSignal::new(false);
    let claim_tin = RwSignal::new(String::new());
    let claim_branch = RwSignal::new("00000".to_string());
    let selected_org = RwSignal::new(String::new());

    let refresh = move |_| {
        #[cfg(feature = "hydrate")]
        if let Some(window) = web_sys::window() {
            let _ = window.location().reload();
        }
    };
    let webmcp_create_name = custom_attribute("toolname", "create_tax_profile");
    let webmcp_create_description = custom_attribute(
        "tooldescription",
        "Create a cloud tax profile for a TIN and branch. The human must confirm. Email must already be verified.",
    );
    let webmcp_tin_param = custom_attribute("toolparamdescription", "Nine-digit TIN root");
    let webmcp_branch_param =
        custom_attribute("toolparamdescription", "Five-digit branch code, head office 00000");
    let webmcp_name_param = custom_attribute("toolparamdescription", "Registered or legal name");
    let webmcp_rdo_param = custom_attribute("toolparamdescription", "BIR RDO code");

    view! {
        <div class=ORG_PAGE>
            <header class=ORG_TOOLBAR>
                <div class=ORG_TOOLBAR_COPY>
                    <p class=ORG_KICKER>"Cloud control plane"</p>
                    <h2 class=ORG_TOOLBAR_TITLE>"Tax profiles"</h2>
                    <p class=ORG_TOOLBAR_SUB>
                        "One holder per hashed TIN identity. A company may hold a profile until the verified owner claims or reclaims it. Cloud identity is the profile UUID, not the TIN."
                    </p>
                </div>
                <button class=BTN_SECONDARY type="button" on:click=refresh>"Refresh"</button>
            </header>

            <Show when=move || create_value.get().and_then(Result::ok).is_some()>
                <p class=BANNER_SUCCESS>"Tax profile saved. Redis only wakes listeners; Postgres is the source of truth."</p>
            </Show>
            <Show when=move || create_value.get().and_then(|r| r.err()).is_some()
                || claim_value.get().and_then(|r| r.err()).is_some()
                || reclaim_value.get().and_then(|r| r.err()).is_some()>
                <p class=BANNER_ERROR>
                    {move || {
                        create_value.get().and_then(|r| r.err()).map(|e| e.to_string())
                            .or_else(|| claim_value.get().and_then(|r| r.err()).map(|e| e.to_string()))
                            .or_else(|| reclaim_value.get().and_then(|r| r.err()).map(|e| e.to_string()))
                            .unwrap_or_default()
                    }}
                </p>
            </Show>

            <section class=PANEL>
                <p class=SECTION_LABEL>"Create"</p>
                <form
                    class=FIELD_GROUP
                    on:submit=move |event| {
                        event.prevent_default();
                        let organization_id = if hold_org.get_untracked() {
                            let id = selected_org.get_untracked();
                            if id.is_empty() {
                                session
                                    .get_untracked()
                                    .and_then(Result::ok)
                                    .and_then(|s| s.tenant_id)
                            } else {
                                Some(id)
                            }
                        } else {
                            None
                        };
                        create.dispatch(CreateTaxProfile {
                            tin_root: tin_root.get_untracked(),
                            branch_code: branch_code.get_untracked(),
                            registered_name: registered_name.get_untracked(),
                            rdo_code: rdo_code.get_untracked(),
                            line_of_business: String::new(),
                            registered_address: String::new(),
                            zip_code: String::new(),
                            phone: String::new(),
                            email: session
                                .get_untracked()
                                .and_then(Result::ok)
                                .and_then(|s| s.primary_email)
                                .unwrap_or_default(),
                            taxpayer_type: taxpayer_type.get_untracked(),
                            tax_classification: None,
                            is_vat_registered: false,
                            organization_id,
                        });
                    }
                    {..webmcp_create_name}
                    {..webmcp_create_description}
                >
                    <label class=FIELD>
                        <span>"TIN (9 digits)"</span>
                        <input class=INPUT name="tin_root" {..webmcp_tin_param} prop:value=move || tin_root.get()
                            on:input=move |ev| tin_root.set(event_target_value(&ev)) />
                    </label>
                    <label class=FIELD>
                        <span>"Branch code"</span>
                        <input class=INPUT name="branch_code" {..webmcp_branch_param} prop:value=move || branch_code.get()
                            on:input=move |ev| branch_code.set(event_target_value(&ev)) />
                    </label>
                    <label class=FIELD>
                        <span>"Registered name"</span>
                        <input class=INPUT name="registered_name" {..webmcp_name_param} prop:value=move || registered_name.get()
                            on:input=move |ev| registered_name.set(event_target_value(&ev)) />
                    </label>
                    <label class=FIELD>
                        <span>"RDO code"</span>
                        <input class=INPUT name="rdo_code" {..webmcp_rdo_param} prop:value=move || rdo_code.get()
                            on:input=move |ev| rdo_code.set(event_target_value(&ev)) />
                    </label>
                    <label class=FIELD>
                        <span>"Taxpayer type"</span>
                        <select class=INPUT prop:value=move || taxpayer_type.get()
                            on:change=move |ev| taxpayer_type.set(event_target_value(&ev))>
                            <option value="individual">"Individual"</option>
                            <option value="corporation">"Corporation"</option>
                            <option value="partnership">"Partnership"</option>
                            <option value="cooperative">"Cooperative"</option>
                            <option value="estate">"Estate"</option>
                            <option value="trust">"Trust"</option>
                        </select>
                    </label>
                    <label class=FIELD>
                        <span>
                            <input type="checkbox" prop:checked=move || hold_org.get()
                                on:change=move |ev| hold_org.set(event_target_checked(&ev)) />
                            " Hold as company / org account"
                        </span>
                    </label>
                    <Show when=move || hold_org.get()>
                        <label class=FIELD>
                            <span>"Organization"</span>
                            <select class=INPUT on:change=move |ev| selected_org.set(event_target_value(&ev))>
                                <For
                                    each=move || orgs.get().and_then(Result::ok).map(|list: OrganizationListResponse| list.organizations).unwrap_or_default()
                                    key=|org| org.organization_id.clone()
                                    children=move |org| {
                                        view! { <option value=org.organization_id.clone()>{org.name}</option> }
                                    }
                                />
                            </select>
                        </label>
                    </Show>
                    <button class=BTN_PRIMARY type="submit" disabled=move || create_pending.get()>"Create tax profile"</button>
                </form>
            </section>

            <section class=PANEL>
                <p class=SECTION_LABEL>"Claim or reclaim"</p>
                <p class=RESULT_LINE>
                    "Attest the TIN to match the hashed identity. ORUS is stubbed unless ORUS_FAKE_VERIFIER is on."
                </p>
                <div class=FIELD_GROUP>
                    <label class=FIELD>
                        <span>"TIN (9 digits)"</span>
                        <input class=INPUT prop:value=move || claim_tin.get()
                            on:input=move |ev| claim_tin.set(event_target_value(&ev)) />
                    </label>
                    <label class=FIELD>
                        <span>"Branch code"</span>
                        <input class=INPUT prop:value=move || claim_branch.get()
                            on:input=move |ev| claim_branch.set(event_target_value(&ev)) />
                    </label>
                    <div class="flex flex-wrap gap-2">
                        <button class=BTN_SECONDARY type="button"
                            on:click=move |_| {
                                claim.dispatch(ClaimTaxProfile {
                                    tin_root: claim_tin.get_untracked(),
                                    branch_code: claim_branch.get_untracked(),
                                });
                            }
                        >"Claim as owner"</button>
                        <button class=BTN_SECONDARY type="button"
                            on:click=move |_| {
                                reclaim.dispatch(ReclaimTaxProfile {
                                    tin_root: claim_tin.get_untracked(),
                                    branch_code: claim_branch.get_untracked(),
                                });
                            }
                        >"Reclaim"</button>
                    </div>
                </div>
            </section>

            <section class=PANEL>
                <p class=SECTION_LABEL>"Held profiles"</p>
                <Show when=move || profiles.get().is_none()>
                    <p class=RESULT_LINE>"Loading tax profiles"</p>
                </Show>
                <Show when=move || matches!(profiles.get(), Some(Err(_)))>
                    <p class=BANNER_ERROR>{move || profiles.get().and_then(|r| r.err()).map(|e| e.to_string()).unwrap_or_default()}</p>
                </Show>
                <Show when=move || profiles.get().and_then(Result::ok).is_some_and(|list| list.profiles.is_empty())>
                    <p class=RESULT_LINE>"No tax profiles yet. Create one after verifying your email."</p>
                </Show>
                <ul>
                    <For
                        each=move || profiles.get().and_then(Result::ok).map(|list| list.profiles).unwrap_or_default()
                        key=|profile| profile.profile_id.clone()
                        children=move |profile| {
                            view! {
                                <li class="mb-4 rounded-xl border border-border-subtle p-4">
                                    <strong>{profile.display_name}</strong>
                                    <p class=RESULT_LINE>
                                        {format!(
                                            "····{} · {} · {} · {} · {}",
                                            profile.tin_last4,
                                            profile.claim_status,
                                            profile.ownership_status,
                                            profile.verification_status,
                                            profile.id
                                        )}
                                    </p>
                                    <div class="mt-2 flex flex-wrap gap-2">
                                        <span class=RESULT_LINE>"Claim/reclaim uses TIN attestation below — the UUID is the cloud key."</span>
                                    </div>
                                </li>
                            }
                        }
                    />
                </ul>
                <Show when=move || claim_value.get().and_then(Result::ok).is_some() || reclaim_value.get().and_then(Result::ok).is_some()>
                    <button class=BTN_SECONDARY type="button" on:click=refresh>"Refresh list"</button>
                </Show>
            </section>
        </div>
    }
}
