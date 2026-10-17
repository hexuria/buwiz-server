//! Access passes: give a workspace access to a tax profile, answer offers and
//! requests, and end passes. Publishing to BIR is a separate switch that asks
//! first; the server only records it for the grantee's installed app.

use crate::app::{
    AskForPass, ChangePass, OfferPass, PassStep, browser_load, list_organizations, list_passes,
    list_tax_profiles,
};
use crate::contracts::{PassListResponse, PassView};
use crate::ui::classes::{
    BANNER_ERROR, BANNER_OK, BTN_PRIMARY, BTN_SECONDARY, BTN_SECONDARY_ACTIVE, FIELD, FIELD_GROUP,
    INPUT, PANEL, RESULT_LINE, SECTION_LABEL,
};
use leptos::prelude::*;

#[island]
pub fn PassesPanel() -> impl IntoView {
    let loaded = browser_load(list_passes);
    let profiles = browser_load(|| list_tax_profiles(None));
    let orgs = browser_load(list_organizations);
    let list = RwSignal::new(None::<PassListResponse>);
    let offer = ServerAction::<OfferPass>::new();
    let ask = ServerAction::<AskForPass>::new();
    let step = ServerAction::<PassStep>::new();
    let change = ServerAction::<ChangePass>::new();

    // Give-access form.
    let profile_id = RwSignal::new(String::new());
    let workspace = RwSignal::new(String::new());
    let chosen = RwSignal::new(Vec::<String>::new());
    let publish = RwSignal::new(false);
    let confirm_publish = RwSignal::new(false);
    let ends_on = RwSignal::new(String::new());
    // Request-access form.
    let ask_org = RwSignal::new(String::new());
    let ask_tin = RwSignal::new(String::new());
    let ask_branch = RwSignal::new("00000".to_string());
    let ask_preset = RwSignal::new(String::new());
    // Change-pass editor: the pass being changed and its draft settings.
    let editing = RwSignal::new(None::<String>);
    let edit_chosen = RwSignal::new(Vec::<String>::new());
    let edit_publish = RwSignal::new(false);
    let edit_confirm = RwSignal::new(false);
    let edit_ends = RwSignal::new(String::new());

    let take = move |value: Option<Result<PassListResponse, server_fn::ServerFnError>>| {
        if let Some(Ok(value)) = value {
            if chosen.get_untracked().is_empty()
                && let Some(preset) = value.presets.iter().find(|p| p.is_default)
            {
                chosen.set(preset.permissions.clone());
                ask_preset.set(preset.id.clone());
            }
            list.set(Some(value));
        }
    };
    Effect::new(move |_| take(loaded.get()));
    Effect::new(move |_| take(offer.value().get()));
    Effect::new(move |_| take(ask.value().get()));
    Effect::new(move |_| take(step.value().get()));
    Effect::new(move |_| {
        let value = change.value().get();
        if value.as_ref().is_some_and(Result::is_ok) {
            editing.set(None);
        }
        take(value);
    });

    let error = move || {
        [offer.value().get(), ask.value().get(), step.value().get(), change.value().get()]
            .into_iter()
            .flatten()
            .find_map(Result::err)
            .map(|e| e.to_string())
    };
    let offered = move || offer.value().get().is_some_and(|r| r.is_ok());
    let asked = move || ask.value().get().is_some_and(|r| r.is_ok());

    let catalog = move || {
        list.get()
            .map(|l| l.permission_catalog)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.id != "filings.publish")
            .collect::<Vec<_>>()
    };
    let presets = move || list.get().map(|l| l.presets).unwrap_or_default();
    let preset_of = move |set: RwSignal<Vec<String>>| {
        let mut now = set.get();
        now.sort();
        presets().into_iter().find(|preset| {
            let mut ids = preset.permissions.clone();
            ids.sort();
            ids == now
        })
    };
    let matching_preset = move || preset_of(chosen);
    let toggle_in = move |set: RwSignal<Vec<String>>, id: String| {
        let order: Vec<String> = catalog().into_iter().map(|p| p.id).collect();
        set.update(|set| {
            if let Some(at) = set.iter().position(|p| *p == id) {
                set.remove(at);
            } else {
                set.push(id);
            }
            set.sort_by_key(|p| order.iter().position(|o| o == p));
        });
    };
    let toggle = move |id: String| toggle_in(chosen, id);
    let start_change = move |pass: &PassView| {
        edit_chosen.set(
            pass.permissions
                .iter()
                .filter(|p| *p != "filings.publish")
                .cloned()
                .collect(),
        );
        edit_publish.set(pass.can_publish);
        edit_confirm.set(false);
        edit_ends.set(last_day(pass).unwrap_or_default());
        editing.set(Some(pass.pass_id.clone()));
    };
    let owned = move || {
        profiles
            .get()
            .and_then(Result::ok)
            .map(|l| l.profiles)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.access == "owner")
            .collect::<Vec<_>>()
    };
    let waiting = move || {
        list.get()
            .map(|l| l.passes)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.waiting_on_me)
            .collect::<Vec<_>>()
    };
    let open = move || list.get().map(|l| l.passes).unwrap_or_default();
    let summary = move || {
        let n = chosen.get().len();
        let total = catalog().len();
        let name = matching_preset().map(|p| p.label).unwrap_or_else(|| "Custom".into());
        format!(
            "{name} · {n} of {total} permissions · {}",
            if publish.get() { "can publish to BIR" } else { "cannot publish to BIR" }
        )
    };

    view! {
        <section class=PANEL id="access-passes">
            <p class=SECTION_LABEL>"Access passes"</p>
            <p class=RESULT_LINE>
                "Let a firm's workspace work on your tax profile, and end it any time. Any member of that workspace can use the pass. Sharing, ending passes, archiving and deleting stay with you."
            </p>
            {move || error().map(|e| view! { <p class=BANNER_ERROR>{e}</p> })}

            <Show when=move || !waiting().is_empty()>
                <p class=SECTION_LABEL>"Waiting for you"</p>
                <ul>
                    <For
                        each=waiting
                        key=|p| format!("{}{}{}", p.pass_id, p.status, p.updated_at)
                        children=move |p: PassView| {
                            let (pid, gid) = (p.profile_id.clone(), p.pass_id.clone());
                            let (pid2, gid2) = (pid.clone(), gid.clone());
                            let owner_side = p.my_side == "owner";
                            let line = if owner_side {
                                format!("{} asks for {} access to {} ····{}", who(&p), what(&p), p.profile_name, p.tin_last4)
                            } else {
                                format!("You are offered {} access to {} ····{} for {}", what(&p), p.profile_name, p.tin_last4, who(&p))
                            };
                            view! {
                                <li class="mb-4 rounded-xl border border-border-subtle p-4">
                                    <p class=RESULT_LINE>{line}</p>
                                    <div class="mt-2 flex flex-wrap gap-2">
                                        <button class=BTN_PRIMARY type="button" on:click=move |_| {
                                            step.dispatch(PassStep {
                                                profile_id: pid.clone(),
                                                pass_id: gid.clone(),
                                                step: if owner_side { "approve".into() } else { "accept".into() },
                                            });
                                        }>{if owner_side { "Approve" } else { "Accept" }}</button>
                                        <button class=BTN_SECONDARY type="button" on:click=move |_| {
                                            step.dispatch(PassStep { profile_id: pid2.clone(), pass_id: gid2.clone(), step: "end".into() });
                                        }>"Decline"</button>
                                    </div>
                                </li>
                            }
                        }
                    />
                </ul>
            </Show>

            <Show when=move || !owned().is_empty()>
                <p class=SECTION_LABEL>"Give access"</p>
                <form class=FIELD_GROUP on:submit=move |event| {
                    event.prevent_default();
                    let profile = profile_id.get_untracked();
                    let profile = if profile.is_empty() {
                        owned().first().map(|p| p.profile_id.clone()).unwrap_or_default()
                    } else {
                        profile
                    };
                    let end = ends_on.get_untracked();
                    offer.dispatch(OfferPass {
                        profile_id: profile,
                        organization_slug: workspace.get_untracked(),
                        permissions: chosen.get_untracked(),
                        allow_publish: publish.get_untracked(),
                        ends_on: (!end.trim().is_empty()).then_some(end),
                    });
                }>
                    <label class=FIELD>
                        <span>"Tax profile"</span>
                        <select class=INPUT on:change=move |ev| profile_id.set(event_target_value(&ev))>
                            <For
                                each=owned
                                key=|p| p.profile_id.clone()
                                children=move |p| view! {
                                    <option value=p.profile_id.clone()>{format!("{} ····{}", p.display_name, p.tin_last4)}</option>
                                }
                            />
                        </select>
                    </label>
                    <label class=FIELD>
                        <span>"Workspace URL name"</span>
                        <input class=INPUT name="workspace" placeholder="abc-firm" prop:value=move || workspace.get()
                            on:input=move |ev| workspace.set(event_target_value(&ev)) />
                    </label>
                    <div class=FIELD>
                        <span>"Start from"</span>
                        <div class="flex flex-wrap gap-2" role="radiogroup" aria-label="Preset">
                            <For
                                each=presets
                                key=|p| p.id.clone()
                                children=move |p| {
                                    let id = StoredValue::new(p.id.clone());
                                    let perms = p.permissions.clone();
                                    let selected = move || matching_preset().is_some_and(|m| m.id == id.get_value());
                                    view! {
                                        <button type="button" role="radio"
                                            class=move || if selected() { BTN_SECONDARY_ACTIVE } else { BTN_SECONDARY }
                                            aria-checked=move || selected().to_string()
                                            on:click=move |_| chosen.set(perms.clone())>
                                            {p.label.clone()}
                                        </button>
                                    }
                                }
                            />
                            <Show when=move || matching_preset().is_none()>
                                <span class=RESULT_LINE>"Custom"</span>
                            </Show>
                        </div>
                    </div>
                    <fieldset class=FIELD>
                        <legend>"Permissions"</legend>
                        <For
                            each=catalog
                            key=|p| p.id.clone()
                            children=move |p| {
                                let id = p.id.clone();
                                let id2 = p.id.clone();
                                view! {
                                    <label class="flex items-center gap-2 text-[13px] font-normal">
                                        <input type="checkbox"
                                            prop:checked=move || chosen.get().contains(&id)
                                            on:change=move |_| toggle(id2.clone()) />
                                        {p.label.clone()}
                                    </label>
                                }
                            }
                        />
                    </fieldset>
                    <div class=BANNER_ERROR>
                        <label class="flex items-center gap-2">
                            <input type="checkbox" role="switch"
                                prop:checked=move || publish.get()
                                on:change=move |ev| {
                                    if event_target_checked(&ev) {
                                        confirm_publish.set(true);
                                    } else {
                                        publish.set(false);
                                    }
                                } />
                            <strong>"Publish to BIR"</strong>
                        </label>
                        <p class="m-0">"Lets them send returns to BIR from their own installed buwiz-forms app. The server never sends anything. No preset turns this on."</p>
                        <Show when=move || confirm_publish.get()>
                            <p class="m-0">
                                <strong>"Are you sure? "</strong>
                                "They will be able to file returns in your name. "
                                <button class=BTN_PRIMARY type="button" on:click=move |_| {
                                    publish.set(true);
                                    confirm_publish.set(false);
                                }>"Yes, allow publishing"</button>
                                " "
                                <button class=BTN_SECONDARY type="button" on:click=move |_| {
                                    publish.set(false);
                                    confirm_publish.set(false);
                                }>"Cancel"</button>
                            </p>
                        </Show>
                    </div>
                    <label class=FIELD>
                        <span>"Last day (optional)"</span>
                        <input class=INPUT type="date" prop:value=move || ends_on.get()
                            on:input=move |ev| ends_on.set(event_target_value(&ev)) />
                    </label>
                    <p class=RESULT_LINE>{summary}</p>
                    <button class=BTN_PRIMARY type="submit" disabled=move || offer.pending().get()>"Send offer"</button>
                    <Show when=offered>
                        <p class=BANNER_OK>"Offer sent. It starts working when the workspace accepts it."</p>
                    </Show>
                </form>
            </Show>

            <p class=SECTION_LABEL>"Open passes"</p>
            <Show when=move || open().is_empty()>
                <p class=RESULT_LINE>"No open passes."</p>
            </Show>
            <ul>
                <For
                    each=open
                    key=|p| format!("{}{}{}", p.pass_id, p.status, p.updated_at)
                    children=move |p: PassView| {
                        let (pid, gid) = (p.profile_id.clone(), p.pass_id.clone());
                        let (pid3, gid3) = (pid.clone(), gid.clone());
                        let label = end_label(&p);
                        let changeable = can_change(&p);
                        let this = StoredValue::new(p.clone());
                        let is_editing = {
                            let gid = gid.clone();
                            move || editing.get().as_deref() == Some(gid.as_str())
                        };
                        let line = format!(
                            "{} ····{} · {} · {} · {} access{}{}",
                            p.profile_name,
                            p.tin_last4,
                            who(&p),
                            p.status,
                            what(&p),
                            if p.can_publish { " · can publish to BIR" } else { "" },
                            p.expires_at.as_deref().map(|e| format!(" · ends {}", e.get(..10).unwrap_or(e))).unwrap_or_default(),
                        );
                        view! {
                            <li class="mb-4 rounded-xl border border-border-subtle p-4">
                                <p class=RESULT_LINE>{line}</p>
                                <div class="mt-2 flex flex-wrap gap-2">
                                    <Show when=move || changeable>
                                        <button class=BTN_SECONDARY type="button"
                                            on:click=move |_| this.with_value(|pass| start_change(pass))>"Change"</button>
                                    </Show>
                                    <button class=BTN_SECONDARY type="button" on:click=move |_| {
                                        step.dispatch(PassStep { profile_id: pid.clone(), pass_id: gid.clone(), step: "end".into() });
                                    }>{label}</button>
                                </div>
                                <Show when=is_editing.clone()>
                                    <form class=FIELD_GROUP on:submit={
                                        let (pid, gid) = (pid3.clone(), gid3.clone());
                                        move |event: leptos::ev::SubmitEvent| {
                                            event.prevent_default();
                                            change.dispatch(ChangePass {
                                                profile_id: pid.clone(),
                                                pass_id: gid.clone(),
                                                permissions: edit_chosen.get_untracked(),
                                                allow_publish: edit_publish.get_untracked(),
                                                ends_on: edit_ends.get_untracked(),
                                            });
                                        }
                                    }>
                                        <fieldset class=FIELD>
                                            <legend>"Permissions"</legend>
                                            <For
                                                each=catalog
                                                key=|p| p.id.clone()
                                                children=move |p| {
                                                    let id = p.id.clone();
                                                    let id2 = p.id.clone();
                                                    view! {
                                                        <label class="flex items-center gap-2 text-[13px] font-normal">
                                                            <input type="checkbox"
                                                                prop:checked=move || edit_chosen.get().contains(&id)
                                                                on:change=move |_| toggle_in(edit_chosen, id2.clone()) />
                                                            {p.label.clone()}
                                                        </label>
                                                    }
                                                }
                                            />
                                        </fieldset>
                                        <div class=BANNER_ERROR>
                                            <label class="flex items-center gap-2">
                                                <input type="checkbox" role="switch"
                                                    prop:checked=move || edit_publish.get()
                                                    on:change=move |ev| {
                                                        if event_target_checked(&ev) {
                                                            edit_confirm.set(true);
                                                        } else {
                                                            edit_publish.set(false);
                                                        }
                                                    } />
                                                <strong>"Publish to BIR"</strong>
                                            </label>
                                            <Show when=move || edit_confirm.get()>
                                                <p class="m-0">
                                                    <strong>"Are you sure? "</strong>
                                                    "They will be able to file returns in your name. "
                                                    <button class=BTN_PRIMARY type="button" on:click=move |_| {
                                                        edit_publish.set(true);
                                                        edit_confirm.set(false);
                                                    }>"Yes, allow publishing"</button>
                                                    " "
                                                    <button class=BTN_SECONDARY type="button" on:click=move |_| {
                                                        edit_publish.set(false);
                                                        edit_confirm.set(false);
                                                    }>"Cancel"</button>
                                                </p>
                                            </Show>
                                        </div>
                                        <label class=FIELD>
                                            <span>"Last day (empty: no end date)"</span>
                                            <input class=INPUT type="date" prop:value=move || edit_ends.get()
                                                on:input=move |ev| edit_ends.set(event_target_value(&ev)) />
                                        </label>
                                        <p class=RESULT_LINE>{move || {
                                            let name = preset_of(edit_chosen).map(|p| p.label).unwrap_or_else(|| "Custom".into());
                                            format!(
                                                "{name} · {} of {} permissions · {}",
                                                edit_chosen.get().len(),
                                                catalog().len(),
                                                if edit_publish.get() { "can publish to BIR" } else { "cannot publish to BIR" }
                                            )
                                        }}</p>
                                        <div class="flex flex-wrap gap-2">
                                            <button class=BTN_PRIMARY type="submit" disabled=move || change.pending().get()>"Save changes"</button>
                                            <button class=BTN_SECONDARY type="button" on:click=move |_| editing.set(None)>"Cancel"</button>
                                        </div>
                                    </form>
                                </Show>
                            </li>
                        }
                    }
                />
            </ul>

            <Show when=move || orgs.get().and_then(Result::ok).is_some_and(|l| !l.organizations.is_empty())>
                <p class=SECTION_LABEL>"Ask a taxpayer for access"</p>
                <p class=RESULT_LINE>"For your workspace. You need the taxpayer's TIN; they approve the request."</p>
                <form class=FIELD_GROUP on:submit=move |event| {
                    event.prevent_default();
                    let org = ask_org.get_untracked();
                    let org = if org.is_empty() {
                        orgs.get_untracked()
                            .and_then(Result::ok)
                            .and_then(|l| l.organizations.first().map(|o| o.organization_id.clone()))
                            .unwrap_or_default()
                    } else {
                        org
                    };
                    ask.dispatch(AskForPass {
                        organization_id: org,
                        tin_root: ask_tin.get_untracked(),
                        branch_code: ask_branch.get_untracked(),
                        preset: ask_preset.get_untracked(),
                    });
                }>
                    <label class=FIELD>
                        <span>"Workspace"</span>
                        <select class=INPUT on:change=move |ev| ask_org.set(event_target_value(&ev))>
                            <For
                                each=move || orgs.get().and_then(Result::ok).map(|l| l.organizations).unwrap_or_default()
                                key=|o| o.organization_id.clone()
                                children=move |o| view! { <option value=o.organization_id.clone()>{o.name}</option> }
                            />
                        </select>
                    </label>
                    <label class=FIELD>
                        <span>"TIN (9 digits)"</span>
                        <input class=INPUT prop:value=move || ask_tin.get()
                            on:input=move |ev| ask_tin.set(event_target_value(&ev)) />
                    </label>
                    <label class=FIELD>
                        <span>"Branch code"</span>
                        <input class=INPUT prop:value=move || ask_branch.get()
                            on:input=move |ev| ask_branch.set(event_target_value(&ev)) />
                    </label>
                    <label class=FIELD>
                        <span>"Access to ask for"</span>
                        <select class=INPUT prop:value=move || ask_preset.get()
                            on:change=move |ev| ask_preset.set(event_target_value(&ev))>
                            <For
                                each=presets
                                key=|p| p.id.clone()
                                children=move |p| view! { <option value=p.id.clone()>{p.label}</option> }
                            />
                        </select>
                    </label>
                    <button class=BTN_SECONDARY type="submit" disabled=move || ask.pending().get()>"Send request"</button>
                    <Show when=asked>
                        <p class=BANNER_OK>"Request sent. The owner sees it on their Tax profiles page."</p>
                    </Show>
                </form>
            </Show>
        </section>
    }
}

/// The firm's notice on its workspace home: offers waiting for one of the
/// caller's workspaces to accept. Shows nothing when there are none.
#[island]
pub fn WorkspaceWaitingNotice() -> impl IntoView {
    let loaded = browser_load(list_passes);
    let waiting = move || {
        loaded
            .get()
            .and_then(Result::ok)
            .map(|list| waiting_for_workspace(&list.passes))
            .unwrap_or_default()
    };
    view! {
        <Show when=move || !waiting().is_empty()>
            <section class=PANEL aria-label="Waiting for your workspace">
                <p class=SECTION_LABEL>"Waiting for your workspace"</p>
                <ul>
                    <For
                        each=waiting
                        key=|p| format!("{}{}", p.pass_id, p.updated_at)
                        children=move |p: PassView| view! {
                            <li class=RESULT_LINE>{format!(
                                "For {}: {} access to {} ····{}, offered by its owner",
                                who(&p),
                                what(&p),
                                p.profile_name,
                                p.tin_last4,
                            )}</li>
                        }
                    />
                </ul>
                <a class=BTN_PRIMARY href="/tax-profiles#access-passes">"Answer on Tax profiles"</a>
            </section>
        </Show>
    }
}

/// Offers a workspace of the caller's has yet to accept, newest first (the
/// list already comes newest first).
fn waiting_for_workspace(passes: &[PassView]) -> Vec<PassView> {
    passes
        .iter()
        .filter(|p| p.my_side == "grantee" && p.waiting_on_me)
        .cloned()
        .collect()
}

fn who(pass: &PassView) -> String {
    pass.grantee_name
        .clone()
        .or_else(|| pass.grantee_slug.clone())
        .unwrap_or_else(|| pass.grantee_id.clone())
}

fn what(pass: &PassView) -> String {
    match pass.preset.as_deref() {
        Some("read_only") => "Read-only".into(),
        Some("safe") => "Safe".into(),
        Some("full") => "Full".into(),
        _ => format!("custom ({} permissions)", pass.permissions.len()),
    }
}

/// Only the owner changes a pass, and only one that is offered or working
/// (a request is approved or declined instead; an expired one is replaced).
fn can_change(pass: &PassView) -> bool {
    pass.my_side == "owner" && matches!(pass.status.as_str(), "offered" | "active")
}

/// The pass's last day as `YYYY-MM-DD` in Philippine time. A pass ends at
/// the next midnight there, which is 16:00 UTC of the last day.
fn last_day(pass: &PassView) -> Option<String> {
    pass.expires_at.as_deref()?.get(..10).map(str::to_owned)
}

fn end_label(pass: &PassView) -> &'static str {
    match (pass.my_side.as_str(), pass.status.as_str()) {
        ("owner", "requested") | ("grantee", "offered") => "Decline",
        ("owner", "offered") | ("grantee", "requested") => "Withdraw",
        ("owner", _) => "Revoke",
        _ => "Give back",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass(side: &str, status: &str, expires_at: Option<&str>) -> PassView {
        PassView {
            pass_id: "g1".into(),
            profile_id: "p1".into(),
            profile_name: "Ana".into(),
            tin_last4: "1234".into(),
            grantee_kind: "organization".into(),
            grantee_id: "o1".into(),
            grantee_name: None,
            grantee_slug: None,
            permissions: vec!["profile.view".into()],
            preset: None,
            can_publish: false,
            status: status.into(),
            expires_at: expires_at.map(str::to_owned),
            created_at: String::new(),
            updated_at: String::new(),
            my_side: side.into(),
            waiting_on_me: false,
        }
    }

    #[test]
    fn the_workspace_notice_lists_only_offers_it_must_answer() {
        let mut offer = pass("grantee", "offered", None);
        offer.waiting_on_me = true;
        let mut request = pass("owner", "requested", None);
        request.waiting_on_me = true;
        let sent = pass("grantee", "requested", None);
        let working = pass("grantee", "active", None);
        let shown = waiting_for_workspace(&[request, offer.clone(), sent, working]);
        assert_eq!(shown, [offer]);
    }

    #[test]
    fn only_the_owner_changes_an_offered_or_working_pass() {
        assert!(can_change(&pass("owner", "offered", None)));
        assert!(can_change(&pass("owner", "active", None)));
        for status in ["requested", "expired"] {
            assert!(!can_change(&pass("owner", status, None)), "{status}");
        }
        assert!(!can_change(&pass("grantee", "active", None)));
    }

    #[test]
    fn the_last_day_is_the_utc_date_of_the_end_instant() {
        // Ends at midnight Manila on 2 March = 16:00 UTC on 1 March.
        let ending = pass("owner", "active", Some("2026-03-01T16:00:00.000Z"));
        assert_eq!(last_day(&ending).as_deref(), Some("2026-03-01"));
        assert_eq!(last_day(&pass("owner", "active", None)), None);
    }
}
