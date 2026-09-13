//! TIN → branch → year tax-profile workspace.

#![allow(unused_imports)]
#![allow(clippy::unused_unit)]
#![allow(clippy::unit_arg)]

use crate::app::{
    ClaimTaxProfile, CreateTaxProfile, ReclaimTaxProfile, browser_load, clone_profile_year,
    get_current_session, get_profile_year, list_organizations, list_profile_years,
    list_tax_profiles, save_profile_year, server_error_text, set_year_forms,
};
use crate::contracts::{
    group_tax_profiles_by_tin, masked_tin_label, year_choices, BirFormSpec, BIR_FORM_CATALOG,
    OrganizationListResponse, ProfileYearView, TaxProfileListResponse,
};
use crate::ui::classes::{
    BANNER_ERROR, BANNER_SUCCESS, BTN_PRIMARY, BTN_SECONDARY, FIELD, FIELD_GROUP, FIELD_HINT,
    INLINE_FIELD, INPUT, ORG_KICKER, ORG_TOOLBAR, ORG_TOOLBAR_COPY, ORG_TOOLBAR_SUB,
    ORG_TOOLBAR_TITLE, PANEL, PROFILE_SWITCH, PROFILE_SWITCH_COPY,
    PROFILE_SWITCH_COPY_SMALL, PROFILE_SWITCH_COPY_STRONG, PROFILE_SWITCH_INPUT,
    PROFILE_SWITCH_THUMB, PROFILE_SWITCH_TRACK, RESULT_LINE, SECTION_LABEL, TAX_BRANCH_META,
    TAX_BRANCH_NAME, TAX_EMPTY, TAX_FORM_CODE, TAX_FORM_FREQ, TAX_FORM_GRID, TAX_FORM_LIST,
    TAX_FORM_TITLE, TAX_FORM_TOOLBAR, TAX_MAIN, TAX_PAGE, TAX_RAIL, TAX_RAIL_ACTIONS, TAX_TIN_GROUP,
    TAX_TIN_HEAD, TAX_TIN_SEGMENT, TAX_TIN_SEGMENTS, TAX_TABS, TAX_WORKSPACE, TAX_YEAR_BAR,
    TAX_YEAR_BAR_FIELDS,
    tax_branch_class, tax_form_row_class, tax_tab_class,
};
use crate::ui::page_shell;
use leptos::attr::custom::custom_attribute;
use leptos::prelude::*;
use leptos::task::spawn_local;
use server_fn::ServerFnError;

#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkspaceTab {
    Profile,
    Forms,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Composer {
    Closed,
    Tin,
    Branch,
}

#[component]
pub fn TaxProfilesPage() -> impl IntoView {
    page_shell(
        "Tax profiles",
        "One TIN per personal account, many branches. Each branch has its own year — that is why forms can change when you go from non-VAT to VAT.",
        view! { <TaxProfilesHome /> },
    )
}

#[island]
pub fn TaxProfilesHome() -> impl IntoView {
    let session = browser_load(get_current_session);
    let orgs = browser_load(list_organizations);
    let profiles = RwSignal::new(None::<Result<TaxProfileListResponse, ServerFnError>>);
    let year_state = RwSignal::new(None::<Result<ProfileYearView, ServerFnError>>);
    let year_history = RwSignal::new(Vec::<i16>::new());
    let selected_id = RwSignal::new(String::new());
    let selected_year = RwSignal::new(default_tax_year());
    let tab = RwSignal::new(WorkspaceTab::Profile);
    let composer = RwSignal::new(Composer::Closed);
    let rail_search = RwSignal::new(String::new());
    let form_search = RwSignal::new(String::new());
    let notice = RwSignal::new(None::<Result<String, String>>);
    let busy = RwSignal::new(false);
    let applied_year_key = RwSignal::new(String::new());

    let tin_root = RwSignal::new(String::new());
    let branch_code = RwSignal::new("00000".to_string());
    let registered_name = RwSignal::new(String::new());
    let rdo_code = RwSignal::new(String::new());
    let line_of_business = RwSignal::new(String::new());
    let registered_address = RwSignal::new(String::new());
    let zip_code = RwSignal::new(String::new());
    let phone = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let taxpayer_type = RwSignal::new("individual".to_string());
    let business_start_date = RwSignal::new(String::new());
    let is_vat = RwSignal::new(false);
    let hold_org = RwSignal::new(false);
    let selected_org = RwSignal::new(String::new());
    let selected_forms = RwSignal::new(Vec::<String>::new());
    let claim_tin = RwSignal::new(String::new());
    let claim_branch = RwSignal::new("00000".to_string());

    let create = ServerAction::<CreateTaxProfile>::new();
    let claim = ServerAction::<ClaimTaxProfile>::new();
    let reclaim = ServerAction::<ReclaimTaxProfile>::new();
    let create_pending = create.pending();
    let create_value = create.value();
    let claim_value = claim.value();
    let reclaim_value = reclaim.value();

    let reload_profiles = move || {
        spawn_local(async move {
            profiles.set(Some(list_tax_profiles(None).await));
        });
    };

    Effect::new(move |_| {
        reload_profiles();
    });

    Effect::new(move |_| {
        if let Some(Ok(profile)) = create_value.get() {
            notice.set(Some(Ok("Branch saved. TIN stays hashed; this year is ready for forms.".into())));
            selected_id.set(profile.profile_id.clone());
            composer.set(Composer::Closed);
            reload_profiles();
        } else if let Some(Err(error)) = create_value.get() {
            notice.set(Some(Err(server_error_text(error))));
            // Commit can succeed and the HTTP worker still trap (Redis wake).
            // Reload so a landed branch appears even when the client body is empty.
            reload_profiles();
        }
    });
    Effect::new(move |_| {
        if claim_value.get().and_then(Result::ok).is_some()
            || reclaim_value.get().and_then(Result::ok).is_some()
        {
            notice.set(Some(Ok("Ownership updated.".into())));
            reload_profiles();
        } else if let Some(error) = claim_value
            .get()
            .and_then(|result| result.err())
            .or_else(|| reclaim_value.get().and_then(|result| result.err()))
        {
            notice.set(Some(Err(error.to_string())));
        }
    });

    Effect::new(move |_| {
        let id = selected_id.get();
        if id.is_empty() {
            year_state.set(None);
            year_history.set(Vec::new());
            applied_year_key.set(String::new());
            return;
        }
        let year = selected_year.get();
        applied_year_key.set(String::new());
        spawn_local(async move {
            let loaded = get_profile_year(id.clone(), year).await;
            if selected_id.get_untracked() != id || selected_year.get_untracked() != year {
                return;
            }
            year_state.set(Some(loaded));
            if let Ok(list) = list_profile_years(id.clone()).await {
                if selected_id.get_untracked() == id {
                    year_history.set(list.years);
                }
            }
        });
    });

    Effect::new(move |_| {
        let Some(Ok(list)) = profiles.get() else {
            return;
        };
        if list.profiles.is_empty() {
            composer.set(Composer::Tin);
            return;
        }
        if selected_id.get_untracked().is_empty() {
            if let Some(first) = list.profiles.first() {
                selected_id.set(first.profile_id.clone());
            }
        }
    });

    Effect::new(move |_| {
        let Some(Ok(year)) = year_state.get() else {
            return;
        };
        let key = format!("{}:{}", year.tax_profile_id, year.tax_year);
        if applied_year_key.get_untracked() == key {
            return;
        }
        applied_year_key.set(key);
        registered_name.set(year.registered_name.clone().unwrap_or_default());
        rdo_code.set(year.rdo_code.clone().unwrap_or_default());
        line_of_business.set(year.line_of_business.clone().unwrap_or_default());
        registered_address.set(year.registered_address.clone().unwrap_or_default());
        zip_code.set(year.zip_code.clone().unwrap_or_default());
        phone.set(year.phone.clone().unwrap_or_default());
        email.set(year.email.clone().unwrap_or_default());
        is_vat.set(year.is_vat_registered.unwrap_or(false));
        selected_forms.set(
            year.forms
                .iter()
                .filter(|form| form.active)
                .map(|form| form.form_code.clone())
                .collect(),
        );
        if let Some(Ok(list)) = profiles.get() {
            if let Some(profile) = list
                .profiles
                .iter()
                .find(|profile| profile.profile_id == year.tax_profile_id)
            {
                taxpayer_type.set(profile.taxpayer_type.clone());
                business_start_date.set(String::new());
                if registered_name.get_untracked().is_empty() {
                    registered_name.set(profile.full_name.clone());
                }
            }
        }
    });

    view! {
        <div class=TAX_PAGE data-testid="tax-profile-workspace">
            <header class=ORG_TOOLBAR>
                <div class=ORG_TOOLBAR_COPY>
                    <p class=ORG_KICKER>"Registration units"</p>
                    <h2 class=ORG_TOOLBAR_TITLE>"Tax profiles"</h2>
                    <p class=ORG_TOOLBAR_SUB>
                        "TIN never changes. Each branch is its own yearly profile, so 00000 and 00001 can file different forms. A personal account holds one TIN; a firm workspace can hold many."
                    </p>
                </div>
            </header>

            <Show when=move || notice.get().as_ref().is_some_and(Result::is_ok)>
                <p class=BANNER_SUCCESS data-testid="tax-profile-notice">
                    {move || notice.get().and_then(Result::ok).unwrap_or_default()}
                </p>
            </Show>
            <Show when=move || notice.get().as_ref().is_some_and(Result::is_err)>
                <p class=BANNER_ERROR data-testid="tax-profile-error">
                    {move || notice.get().and_then(Result::err).unwrap_or_default()}
                </p>
            </Show>
            <Show when=move || matches!(profiles.get(), Some(Err(_)))>
                <p class=BANNER_ERROR>
                    {move || profiles.get().and_then(|result| result.err()).map(|error| error.to_string()).unwrap_or_default()}
                </p>
            </Show>
            <Show when=move || matches!(year_state.get(), Some(Err(_)))>
                <p class=BANNER_ERROR>
                    {move || year_state.get().and_then(|result| result.err()).map(|error| error.to_string()).unwrap_or_default()}
                </p>
            </Show>

            <div class=TAX_WORKSPACE>
                <aside class=TAX_RAIL aria-label="TIN and branches" data-testid="tax-profile-tin-rail">
                    <label class=FIELD>
                        <span class=SECTION_LABEL>"Search TIN or branch"</span>
                        <input
                            class=INPUT
                            type="search"
                            placeholder="last4 or 00001"
                            prop:value=move || rail_search.get()
                            on:input=move |ev| rail_search.set(event_target_value(&ev))
                            data-testid="tax-profile-rail-search"
                        />
                    </label>
                    {move || {
                        let list = profiles.get().and_then(Result::ok);
                        let groups = list
                            .as_ref()
                            .map(|response| group_tax_profiles_by_tin(response.profiles.clone()))
                            .unwrap_or_default();
                        let personal_locked = groups.iter().any(|group| group.holder_kind == "personal");
                        let has_orgs = orgs
                            .get()
                            .and_then(Result::ok)
                            .is_some_and(|list: OrganizationListResponse| !list.organizations.is_empty());
                        let can_add_tin = !personal_locked || has_orgs;
                        view! {
                            <div class=TAX_RAIL_ACTIONS>
                                <button
                                    class=BTN_SECONDARY
                                    type="button"
                                    disabled=!can_add_tin
                                    data-testid="tax-profile-add-tin"
                                    on:click=move |_| {
                                        hold_org.set(personal_locked && has_orgs);
                                        tin_root.set(String::new());
                                        branch_code.set("00000".into());
                                        registered_name.set(String::new());
                                        rdo_code.set(String::new());
                                        composer.set(Composer::Tin);
                                    }
                                >"Add TIN"</button>
                                <button
                                    class=BTN_SECONDARY
                                    type="button"
                                    disabled=groups.is_empty()
                                    data-testid="tax-profile-add-branch"
                                    on:click=move |_| {
                                        hold_org.set(false);
                                        tin_root.set(String::new());
                                        branch_code.set(String::new());
                                        registered_name.set(String::new());
                                        rdo_code.set(String::new());
                                        composer.set(Composer::Branch);
                                    }
                                >"Add branch"</button>
                            </div>
                            <p class=RESULT_LINE>
                                {if personal_locked {
                                    "This account holds one personal TIN. Add branches under it, or another TIN on a firm workspace."
                                } else {
                                    "A firm can hold many TINs. Each TIN works like a personal profile: branches, then a year."
                                }}
                            </p>
                        }
                    }}
                    {move || rail_groups(
                        profiles.get(),
                        rail_search.get(),
                        selected_id.get(),
                        selected_id,
                    )}
                </aside>

                <section class=TAX_MAIN>
                    <Show when=move || composer.get() != Composer::Closed>
                        <form
                            class=FIELD_GROUP
                            data-testid="tax-profile-composer"
                            on:submit=move |event| {
                                event.prevent_default();
                                let organization_id = if hold_org.get_untracked() {
                                    let id = selected_org.get_untracked();
                                    if id.is_empty() {
                                        session
                                            .get_untracked()
                                            .and_then(Result::ok)
                                            .and_then(|current| current.tenant_id)
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
                                    line_of_business: line_of_business.get_untracked(),
                                    registered_address: registered_address.get_untracked(),
                                    zip_code: zip_code.get_untracked(),
                                    phone: phone.get_untracked(),
                                    email: email.get_untracked().if_empty_then(|| {
                                        session
                                            .get_untracked()
                                            .and_then(Result::ok)
                                            .and_then(|current| current.primary_email)
                                            .unwrap_or_default()
                                    }),
                                    taxpayer_type: taxpayer_type.get_untracked(),
                                    tax_classification: None,
                                    is_vat_registered: false,
                                    business_start_date: optional_text(business_start_date.get_untracked()),
                                    organization_id,
                                });
                            }
                            {..custom_attribute("toolname", "create_tax_profile")}
                            {..custom_attribute(
                                "tooldescription",
                                "Create a cloud tax profile for a TIN and branch. The human must confirm. Email must already be verified.",
                            )}
                        >
                            <p class=SECTION_LABEL>
                                {move || if composer.get() == Composer::Branch { "Add a branch" } else { "Add a TIN" }}
                            </p>
                            <p class=RESULT_LINE>
                                {move || if composer.get() == Composer::Branch {
                                    "Attest the same 9-digit TIN. The new 5-digit branch is a different yearly profile."
                                } else {
                                    "TIN is attested once and never stored as the key. After this, only last4 and branch are shown."
                                }}
                            </p>
                            <div class=TAX_FORM_GRID>
                                <label class=FIELD>
                                    <span>"TIN (9 digits)"</span>
                                    <input class=INPUT name="tin_root" {..custom_attribute("toolparamdescription", "Nine-digit TIN root")}
                                        prop:value=move || tin_root.get()
                                        on:input=move |ev| tin_root.set(event_target_value(&ev)) />
                                </label>
                                <label class=FIELD>
                                    <span>"Branch code"</span>
                                    <input class=INPUT name="branch_code" {..custom_attribute("toolparamdescription", "Five-digit branch code, head office 00000")}
                                        prop:value=move || branch_code.get()
                                        on:input=move |ev| branch_code.set(event_target_value(&ev)) />
                                </label>
                                <label class=FIELD>
                                    <span>"Registered name"</span>
                                    <input class=INPUT name="registered_name" {..custom_attribute("toolparamdescription", "Registered or legal name")}
                                        prop:value=move || registered_name.get()
                                        on:input=move |ev| registered_name.set(event_target_value(&ev)) />
                                </label>
                                <label class=FIELD>
                                    <span>"RDO code"</span>
                                    <input class=INPUT name="rdo_code" {..custom_attribute("toolparamdescription", "BIR RDO code")}
                                        prop:value=move || rdo_code.get()
                                        on:input=move |ev| rdo_code.set(event_target_value(&ev)) />
                                </label>
                            </div>
                            <label class=INLINE_FIELD>
                                <input
                                    type="checkbox"
                                    prop:checked=move || hold_org.get()
                                    on:change=move |ev| hold_org.set(event_target_checked(&ev))
                                    data-testid="tax-profile-hold-org"
                                />
                                <span>"Hold on a firm workspace"</span>
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
                            <div class="flex flex-wrap gap-2">
                                <button class=BTN_PRIMARY type="submit" disabled=move || create_pending.get()>
                                    {move || if composer.get() == Composer::Branch { "Create branch" } else { "Create TIN" }}
                                </button>
                                <button class=BTN_SECONDARY type="button" on:click=move |_| composer.set(Composer::Closed)>"Cancel"</button>
                            </div>
                        </form>
                    </Show>

                    <Show when=move || composer.get() == Composer::Closed && selected_id.get().is_empty()>
                        <div class=TAX_EMPTY>
                            <p class=SECTION_LABEL>"No tax profile yet"</p>
                            <p class=RESULT_LINE>"Create a TIN to start a yearly profile. Personal accounts keep one TIN and add branches under it."</p>
                        </div>
                    </Show>

                    <Show when=move || composer.get() == Composer::Closed && !selected_id.get().is_empty()>
                        <div class="grid min-w-0 gap-5">
                            <div class=TAX_YEAR_BAR>
                                <div class=TAX_YEAR_BAR_FIELDS>
                                    <label class=FIELD>
                                        <span>"Year"</span>
                                        <select
                                            class=INPUT
                                            data-testid="tax-profile-year"
                                            prop:value=move || selected_year.get().to_string()
                                            on:change=move |ev| {
                                                if let Ok(year) = event_target_value(&ev).parse::<i16>() {
                                                    selected_year.set(year);
                                                }
                                            }
                                        >
                                            <For
                                                each=move || year_choices(&year_history.get(), default_tax_year())
                                                key=|year| *year
                                                children=move |year| {
                                                    view! { <option value=year.to_string()>{year.to_string()}</option> }
                                                }
                                            />
                                        </select>
                                    </label>
                                    <button
                                        class=BTN_SECONDARY
                                        type="button"
                                        data-testid="tax-profile-clone-year"
                                        disabled=move || busy.get()
                                        on:click=move |_| {
                                            let profile_id = selected_id.get_untracked();
                                            let to_year = selected_year.get_untracked();
                                            let from_year = to_year - 1;
                                            busy.set(true);
                                            spawn_local(async move {
                                                match clone_profile_year(profile_id, from_year, to_year).await {
                                                    Ok(_) => {
                                                        notice.set(Some(Ok(format!("Cloned {from_year} into {to_year}."))));
                                                        applied_year_key.set(String::new());
                                                        year_state.set(Some(get_profile_year(selected_id.get_untracked(), to_year).await));
                                                    }
                                                    Err(error) => notice.set(Some(Err(error.to_string()))),
                                                }
                                                busy.set(false);
                                            });
                                        }
                                    >"Clone from previous year"</button>
                                </div>
                                <button
                                    class=BTN_PRIMARY
                                    type="button"
                                    data-testid="tax-profile-save"
                                    disabled=move || busy.get()
                                    on:click=move |_| save_selected(
                                        selected_id,
                                        selected_year,
                                        registered_name,
                                        rdo_code,
                                        line_of_business,
                                        registered_address,
                                        zip_code,
                                        phone,
                                        email,
                                        taxpayer_type,
                                        business_start_date,
                                        is_vat,
                                        selected_forms,
                                        profiles,
                                        notice,
                                        busy,
                                        applied_year_key,
                                        year_state,
                                    )
                                >"Save year"</button>
                            </div>

                            {move || selected_profile_banner(profiles.get(), selected_id.get())}

                            <div class=TAX_TABS role="tablist">
                                <button
                                    class=move || tax_tab_class(tab.get() == WorkspaceTab::Profile)
                                    type="button"
                                    role="tab"
                                    data-testid="tax-profile-tab-profile"
                                    on:click=move |_| tab.set(WorkspaceTab::Profile)
                                >"Tax profile"</button>
                                <button
                                    class=move || tax_tab_class(tab.get() == WorkspaceTab::Forms)
                                    type="button"
                                    role="tab"
                                    data-testid="tax-profile-tab-forms"
                                    on:click=move |_| tab.set(WorkspaceTab::Forms)
                                >"Forms"</button>
                            </div>

                            <Show when=move || tab.get() == WorkspaceTab::Profile>
                                <div class=TAX_FORM_GRID data-testid="tax-profile-editor">
                                    <label class=FIELD>
                                        <span>"Registered name"</span>
                                        <input class=INPUT prop:value=move || registered_name.get()
                                            on:input=move |ev| registered_name.set(event_target_value(&ev)) />
                                        <small class=FIELD_HINT>"Name can change. TIN cannot."</small>
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
                                        <span>"RDO code"</span>
                                        <input class=INPUT prop:value=move || rdo_code.get()
                                            on:input=move |ev| rdo_code.set(event_target_value(&ev)) />
                                    </label>
                                    <label class=FIELD>
                                        <span>"Line of business"</span>
                                        <input class=INPUT prop:value=move || line_of_business.get()
                                            on:input=move |ev| line_of_business.set(event_target_value(&ev)) />
                                    </label>
                                    <label class=FIELD>
                                        <span>"Registered address"</span>
                                        <input class=INPUT prop:value=move || registered_address.get()
                                            on:input=move |ev| registered_address.set(event_target_value(&ev)) />
                                    </label>
                                    <label class=FIELD>
                                        <span>"ZIP code"</span>
                                        <input class=INPUT prop:value=move || zip_code.get()
                                            on:input=move |ev| zip_code.set(event_target_value(&ev)) />
                                    </label>
                                    <label class=FIELD>
                                        <span>"Phone"</span>
                                        <input class=INPUT prop:value=move || phone.get()
                                            on:input=move |ev| phone.set(event_target_value(&ev)) />
                                    </label>
                                    <label class=FIELD>
                                        <span>"Email"</span>
                                        <input class=INPUT type="email" prop:value=move || email.get()
                                            on:input=move |ev| email.set(event_target_value(&ev)) />
                                    </label>
                                    <label class=FIELD>
                                        <span>"Business start date"</span>
                                        <input class=INPUT placeholder="YYYY-MM-DD" prop:value=move || business_start_date.get()
                                            on:input=move |ev| business_start_date.set(event_target_value(&ev)) />
                                    </label>
                                </div>
                                <label class=PROFILE_SWITCH>
                                    <input
                                        class=PROFILE_SWITCH_INPUT
                                        type="checkbox"
                                        prop:checked=move || is_vat.get()
                                        on:change=move |ev| is_vat.set(event_target_checked(&ev))
                                    />
                                    <span class=PROFILE_SWITCH_TRACK aria-hidden="true">
                                        <span class=PROFILE_SWITCH_THUMB></span>
                                    </span>
                                    <span class=PROFILE_SWITCH_COPY>
                                        <strong class=PROFILE_SWITCH_COPY_STRONG>"VAT registered this year"</strong>
                                        <small class=PROFILE_SWITCH_COPY_SMALL>
                                            "Going from non-VAT to VAT is why each year has its own form set."
                                        </small>
                                    </span>
                                </label>
                            </Show>

                            <Show when=move || tab.get() == WorkspaceTab::Forms>
                                <div class="grid min-w-0 gap-4" data-testid="tax-profile-forms">
                                    <div class=TAX_FORM_TOOLBAR>
                                        <p class=SECTION_LABEL>
                                            {move || format!("{} selected", selected_forms.get().len())}
                                        </p>
                                        <div class="flex flex-wrap gap-2">
                                            <button class=BTN_SECONDARY type="button"
                                                on:click=move |_| selected_forms.set(Vec::new())
                                            >"Deselect all"</button>
                                        </div>
                                    </div>
                                    <label class=FIELD>
                                        <span class=SECTION_LABEL>"Search forms"</span>
                                        <input
                                            class=INPUT
                                            type="search"
                                            placeholder="0605, 2550Q, 1701…"
                                            prop:value=move || form_search.get()
                                            on:input=move |ev| form_search.set(event_target_value(&ev))
                                            data-testid="tax-profile-forms-search"
                                        />
                                    </label>
                                    <ul class=TAX_FORM_LIST>
                                        <For
                                            each=move || visible_forms(form_search.get())
                                            key=|form| form.code
                                            children=move |form| {
                                                let code = form.code;
                                                view! {
                                                    <li>
                                                        <label
                                                            class=move || tax_form_row_class(selected_forms.get().iter().any(|item| item == code))
                                                            data-testid=format!("tax-profile-form-{code}")
                                                        >
                                                            <input
                                                                type="checkbox"
                                                                prop:checked=move || selected_forms.get().iter().any(|item| item == code)
                                                                on:change=move |_| toggle_form(selected_forms, code)
                                                            />
                                                            <span>
                                                                <span class=TAX_FORM_CODE>{code}</span>
                                                                <span class=TAX_FORM_TITLE>{form.title}</span>
                                                            </span>
                                                            <span class=TAX_FORM_FREQ>{form.frequency}</span>
                                                        </label>
                                                    </li>
                                                }
                                            }
                                        />
                                    </ul>
                                </div>
                            </Show>
                        </div>
                    </Show>
                </section>
            </div>

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
        </div>
    }
}

fn default_tax_year() -> i16 {
    #[cfg(feature = "hydrate")]
    {
        js_sys::Date::new_0().get_full_year() as i16
    }
    #[cfg(not(feature = "hydrate"))]
    {
        2026
    }
}

fn optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

trait IfEmptyThen {
    fn if_empty_then(self, fallback: impl FnOnce() -> String) -> String;
}

impl IfEmptyThen for String {
    fn if_empty_then(self, fallback: impl FnOnce() -> String) -> String {
        if self.trim().is_empty() {
            fallback()
        } else {
            self
        }
    }
}

fn visible_forms(query: String) -> Vec<BirFormSpec> {
    let query = query.trim().to_ascii_lowercase();
    BIR_FORM_CATALOG
        .iter()
        .copied()
        .filter(|form| {
            query.is_empty()
                || form.code.to_ascii_lowercase().contains(&query)
                || form.title.to_ascii_lowercase().contains(&query)
        })
        .collect()
}

fn toggle_form(selected: RwSignal<Vec<String>>, code: &str) {
    let mut next = selected.get_untracked();
    if next.iter().any(|item| item == code) {
        next.retain(|item| item != code);
    } else {
        next.push(code.to_owned());
    }
    selected.set(next);
}

fn rail_groups(
    loaded: Option<Result<TaxProfileListResponse, ServerFnError>>,
    query: String,
    selected_id: String,
    selected: RwSignal<String>,
) -> impl IntoView {
    match loaded {
        None => view! { <p class=RESULT_LINE>"Loading tax profiles"</p> }.into_any(),
        Some(Err(_)) => view! { <></> }.into_any(),
        Some(Ok(list)) if list.profiles.is_empty() => view! {
            <div class=TAX_EMPTY>
                <p class=RESULT_LINE>"No TIN yet. Add the 9-digit root first, then branches."</p>
            </div>
        }
        .into_any(),
        Some(Ok(list)) => {
            let query = query.trim().to_ascii_lowercase();
            let groups = group_tax_profiles_by_tin(list.profiles)
                .into_iter()
                .filter(|group| {
                    query.is_empty()
                        || group.tin_last4.to_ascii_lowercase().contains(&query)
                        || group.display_name.to_ascii_lowercase().contains(&query)
                        || group.branches.iter().any(|branch| {
                            branch.branch_code.contains(&query)
                                || branch.display_name.to_ascii_lowercase().contains(&query)
                        })
                })
                .collect::<Vec<_>>();
            view! {
                <div class="grid gap-3">
                    <For
                        each=move || groups.clone()
                        key=|group| format!("{}:{}", group.holder_kind, group.tin_last4)
                        children=move |group| {
                            let selected_id = selected_id.clone();
                            view! {
                                <div class=TAX_TIN_GROUP>
                                    <div class=TAX_TIN_HEAD>
                                        <span>{format!("TIN ····{}", group.tin_last4)}</span>
                                        <span>{if group.holder_kind == "organization" { "Firm" } else { "Personal" }}</span>
                                    </div>
                                    <For
                                        each=move || group.branches.clone()
                                        key=|branch| branch.profile_id.clone()
                                        children=move |branch| {
                                            let id = branch.profile_id.clone();
                                            let select_id = id.clone();
                                            let active = selected_id == id;
                                            view! {
                                                <button
                                                    class=tax_branch_class(active)
                                                    type="button"
                                                    data-testid=format!("tax-profile-branch-{}", branch.branch_code)
                                                    on:click=move |_| selected.set(select_id.clone())
                                                >
                                                    <span class=TAX_BRANCH_NAME>{branch.display_name.clone()}</span>
                                                    <span class=TAX_BRANCH_META>
                                                        {masked_tin_label(&branch.tin_last4, &branch.branch_code)}
                                                    </span>
                                                </button>
                                            }
                                        }
                                    />
                                </div>
                            }
                        }
                    />
                </div>
            }
            .into_any()
        }
    }
}

fn selected_profile_banner(
    loaded: Option<Result<TaxProfileListResponse, ServerFnError>>,
    selected_id: String,
) -> impl IntoView {
    let Some(profile) = loaded
        .and_then(Result::ok)
        .and_then(|list| {
            list.profiles
                .into_iter()
                .find(|profile| profile.profile_id == selected_id)
        })
    else {
        return view! { <></> }.into_any();
    };
    let last4 = profile.tin_last4.clone();
    let branch = profile.branch_code.clone();
    view! {
        <div class="grid gap-2">
            <p class=SECTION_LABEL>"Taxpayer identification number"</p>
            <div class=TAX_TIN_SEGMENTS aria-label="Masked TIN">
                <div class=TAX_TIN_SEGMENT>"•••"</div>
                <div class=TAX_TIN_SEGMENT>"•••"</div>
                <div class=TAX_TIN_SEGMENT>{last4}</div>
                <div class=TAX_TIN_SEGMENT>{branch}</div>
            </div>
            <p class=RESULT_LINE>
                {format!(
                    "{} · {} · {}",
                    masked_tin_label(&profile.tin_last4, &profile.branch_code),
                    profile.holder_kind,
                    profile.rdo_code
                )}
            </p>
        </div>
    }
    .into_any()
}

fn save_selected(
    selected_id: RwSignal<String>,
    selected_year: RwSignal<i16>,
    registered_name: RwSignal<String>,
    rdo_code: RwSignal<String>,
    line_of_business: RwSignal<String>,
    registered_address: RwSignal<String>,
    zip_code: RwSignal<String>,
    phone: RwSignal<String>,
    email: RwSignal<String>,
    taxpayer_type: RwSignal<String>,
    business_start_date: RwSignal<String>,
    is_vat: RwSignal<bool>,
    selected_forms: RwSignal<Vec<String>>,
    profiles: RwSignal<Option<Result<TaxProfileListResponse, ServerFnError>>>,
    notice: RwSignal<Option<Result<String, String>>>,
    busy: RwSignal<bool>,
    applied_year_key: RwSignal<String>,
    year_state: RwSignal<Option<Result<ProfileYearView, ServerFnError>>>,
) {
    let profile_id = selected_id.get_untracked();
    if profile_id.is_empty() {
        return;
    }
    let tax_year = selected_year.get_untracked();
    let name = registered_name.get_untracked();
    let rdo = rdo_code.get_untracked();
    let line = line_of_business.get_untracked();
    let address = registered_address.get_untracked();
    let zip = zip_code.get_untracked();
    let phone = phone.get_untracked();
    let email = email.get_untracked();
    let taxpayer_type = taxpayer_type.get_untracked();
    let start = business_start_date.get_untracked();
    let vat = is_vat.get_untracked();
    let forms = selected_forms.get_untracked();
    busy.set(true);
    spawn_local(async move {
        let saved = save_profile_year(
            profile_id.clone(),
            tax_year,
            Some(name.clone()),
            Some(rdo),
            Some(line),
            Some(address),
            Some(zip),
            Some(phone),
            Some(email),
            Some(taxpayer_type),
            None,
            Some(vat),
            optional_text(start),
            Some(name),
            None,
        )
        .await;
        let forms = match saved {
            Ok(_) => set_year_forms(profile_id.clone(), tax_year, forms).await,
            Err(error) => Err(error),
        };
        match forms {
            Ok(year) => {
                notice.set(Some(Ok(format!(
                    "Saved {tax_year} for this branch ({} forms).",
                    year.forms.len()
                ))));
                applied_year_key.set(String::new());
                year_state.set(Some(Ok(year)));
                profiles.set(Some(list_tax_profiles(None).await));
            }
            Err(error) => notice.set(Some(Err(error.to_string()))),
        }
        busy.set(false);
    });
}
