//! Owner invites and firm publishers.
//!
//! A firm that created a draft invites the taxpayer by email and shares the
//! link itself; the taxpayer opens it signed in with that verified email and
//! becomes the owner, and the firm keeps a Full pass without publishing. A
//! firm also picks which of its members may publish to BIR; publishing needs
//! that pick *and* the owner's switch on the firm's pass.

use crate::app::{
    AcceptOwnerInvite, ChangeFirmPublisher, InviteOwner, WithdrawOwnerInvite, browser_load,
    list_firm_publishers, list_organizations, list_passes, list_tax_profiles, preview_owner_invite,
};
use crate::contracts::{FirmPublisherListResponse, OwnerInviteView, PassListResponse};
use crate::ui::classes::{
    BANNER_ERROR, BANNER_OK, BTN_PRIMARY, BTN_SECONDARY, FIELD, FIELD_GROUP, INPUT, PANEL,
    RESULT_LINE, SECTION_LABEL,
};
use crate::ui::page_shell;
use leptos::prelude::*;

/// The firm's side: invite taxpayers to own drafts, and pick publishers.
#[island]
pub fn FirmPanel() -> impl IntoView {
    let orgs = browser_load(list_organizations);
    let passes = browser_load(list_passes);
    let org_id = RwSignal::new(String::new());
    let drafts = RwSignal::new(Vec::<(String, String)>::new());
    let invites = RwSignal::new(Vec::<OwnerInviteView>::new());
    let publishers = RwSignal::new(None::<FirmPublisherListResponse>);
    let invite = ServerAction::<InviteOwner>::new();
    let withdraw = ServerAction::<WithdrawOwnerInvite>::new();
    let change = ServerAction::<ChangeFirmPublisher>::new();
    let draft_id = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let member_email = RwSignal::new(String::new());

    let first_org = move || {
        orgs.get()
            .and_then(Result::ok)
            .and_then(|l| l.organizations.first().map(|o| o.organization_id.clone()))
            .unwrap_or_default()
    };
    let current_org = move || {
        let chosen = org_id.get();
        if chosen.is_empty() { first_org() } else { chosen }
    };
    let take_invites = move |value: Option<Result<PassListResponse, server_fn::ServerFnError>>| {
        if let Some(Ok(list)) = value {
            invites.set(list.invites);
        }
    };
    Effect::new(move |_| take_invites(passes.get()));
    Effect::new(move |_| take_invites(withdraw.value().get()));
    // Reload drafts and publishers whenever the workspace changes or an
    // invite is sent.
    Effect::new(move |_| {
        let org = current_org();
        let _ = invite.value().get();
        if org.is_empty() {
            return;
        }
        let org_for_profiles = org.clone();
        leptos::task::spawn_local(async move {
            if let Ok(list) = list_tax_profiles(Some(org_for_profiles)).await {
                drafts.set(draft_choices(list.profiles));
            }
            if let Ok(list) = list_firm_publishers(org).await {
                publishers.set(Some(list));
            }
        });
    });
    Effect::new(move |_| {
        if let Some(Ok(list)) = change.value().get() {
            publishers.set(Some(list));
        }
    });

    let error = move || {
        invite
            .value()
            .get()
            .and_then(Result::err)
            .or_else(|| withdraw.value().get().and_then(Result::err))
            .or_else(|| change.value().get().and_then(Result::err))
            .map(|e| e.to_string())
    };
    let link = move || {
        invite.value().get().and_then(Result::ok).map(|created| {
            let origin = browser_origin();
            format!("{origin}{}", created.link_path)
        })
    };
    let has_orgs = move || has_any_org(orgs.get().and_then(Result::ok).as_ref());

    view! {
        <Show when=has_orgs>
            <section class=PANEL>
                <p class=SECTION_LABEL>"For your firm"</p>
                {move || error().map(|e| view! { <p class=BANNER_ERROR>{e}</p> })}
                <label class=FIELD>
                    <span>"Workspace"</span>
                    <select class=INPUT on:change=move |ev| org_id.set(event_target_value(&ev))>
                        <For
                            each=move || orgs.get().and_then(Result::ok).map(|l| l.organizations).unwrap_or_default()
                            key=|o| o.organization_id.clone()
                            children=move |o| view! { <option value=o.organization_id.clone()>{o.name}</option> }
                        />
                    </select>
                </label>

                <p class=SECTION_LABEL>"Invite the taxpayer to own a draft"</p>
                <p class=RESULT_LINE>
                    "Drafts are profiles your firm created that no taxpayer owns yet. Only someone signed in with the email you enter can accept. When they do, they own the profile and your firm keeps working on it under a Full pass without publishing, which they can change or end."
                </p>
                <Show
                    when=move || !drafts.get().is_empty()
                    fallback=|| view! { <p class=RESULT_LINE>"This workspace has no drafts."</p> }
                >
                    <form class=FIELD_GROUP on:submit=move |event| {
                        event.prevent_default();
                        let chosen = draft_id.get_untracked();
                        let profile_id = if chosen.is_empty() {
                            drafts.get_untracked().first().map(|d| d.0.clone()).unwrap_or_default()
                        } else {
                            chosen
                        };
                        invite.dispatch(InviteOwner {
                            profile_id,
                            email: email.get_untracked(),
                            days: None,
                        });
                    }>
                        <label class=FIELD>
                            <span>"Draft"</span>
                            <select class=INPUT on:change=move |ev| draft_id.set(event_target_value(&ev))>
                                <For
                                    each=move || drafts.get()
                                    key=|d| d.0.clone()
                                    children=move |d| view! { <option value=d.0.clone()>{d.1.clone()}</option> }
                                />
                            </select>
                        </label>
                        <label class=FIELD>
                            <span>"Taxpayer's email"</span>
                            <input class=INPUT type="email" required prop:value=move || email.get()
                                on:input=move |ev| email.set(event_target_value(&ev)) />
                        </label>
                        <button class=BTN_PRIMARY type="submit" disabled=move || invite.pending().get()>"Create invite link"</button>
                    </form>
                </Show>
                {move || link().map(|link| view! {
                    <div class=BANNER_OK>
                        <p class="m-0">"Send this link to the taxpayer yourself. It works for 14 days and is shown only now."</p>
                        <input class=INPUT readonly prop:value=link />
                    </div>
                })}

                <Show when=move || !invites.get().is_empty()>
                    <p class=SECTION_LABEL>"Invites waiting"</p>
                    <ul>
                        <For
                            each=move || invites.get()
                            key=|i| format!("{}{}", i.invite_id, i.status)
                            children=move |i: OwnerInviteView| {
                                let (pid, iid) = (i.profile_id.clone(), i.invite_id.clone());
                                let line = format!(
                                    "{} ····{} · for {} · {} · until {}",
                                    i.profile_name,
                                    i.tin_last4,
                                    i.email,
                                    i.status,
                                    i.expires_at.get(..10).unwrap_or(&i.expires_at),
                                );
                                view! {
                                    <li class="mb-4 rounded-xl border border-border-subtle p-4">
                                        <p class=RESULT_LINE>{line}</p>
                                        <button class=BTN_SECONDARY type="button" on:click=move |_| {
                                            withdraw.dispatch(WithdrawOwnerInvite { profile_id: pid.clone(), invite_id: iid.clone() });
                                        }>"Withdraw"</button>
                                    </li>
                                }
                            }
                        />
                    </ul>
                </Show>

                <p class=SECTION_LABEL>"Who may publish to BIR"</p>
                <p class=RESULT_LINE>
                    "A member here can publish a taxpayer's returns only when the taxpayer switched on publishing for your firm's pass too. Changing this list needs workspace admin rights and a recent second-factor check."
                </p>
                <Show
                    when=move || publishers.get().is_some_and(|l| !l.publishers.is_empty())
                    fallback=|| view! { <p class=RESULT_LINE>"Nobody yet."</p> }
                >
                    <ul>
                        <For
                            each=move || publishers.get().map(|l| l.publishers).unwrap_or_default()
                            key=|p| p.user_id.clone()
                            children=move |p| {
                                let user_id = p.user_id.clone();
                                view! {
                                    <li class="mb-2 flex flex-wrap items-center gap-2">
                                        <span class=RESULT_LINE>{p.email.clone()}</span>
                                        <button class=BTN_SECONDARY type="button" on:click=move |_| {
                                            change.dispatch(ChangeFirmPublisher {
                                                organization_id: current_org(),
                                                email: None,
                                                user_id: Some(user_id.clone()),
                                                remove: true,
                                            });
                                        }>"Remove"</button>
                                    </li>
                                }
                            }
                        />
                    </ul>
                </Show>
                <form class=FIELD_GROUP on:submit=move |event| {
                    event.prevent_default();
                    change.dispatch(ChangeFirmPublisher {
                        organization_id: current_org(),
                        email: Some(member_email.get_untracked()),
                        user_id: None,
                        remove: false,
                    });
                }>
                    <label class=FIELD>
                        <span>"Member's sign-in email"</span>
                        <input class=INPUT type="email" required prop:value=move || member_email.get()
                            on:input=move |ev| member_email.set(event_target_value(&ev)) />
                    </label>
                    <button class=BTN_SECONDARY type="submit" disabled=move || change.pending().get()>"Let them publish"</button>
                </form>
            </section>
        </Show>
    }
}

#[component]
pub fn OwnerInvitePage() -> impl IntoView {
    page_shell(
        "Own your tax profile",
        "A firm prepared a tax profile for you and invited you to own it.",
        view! { <OwnerInviteAccept /> },
    )
}

/// The taxpayer's side: `/invites/{id}#t={secret}`.
#[island]
pub fn OwnerInviteAccept() -> impl IntoView {
    let (invite_id, token) = invite_from_location();
    let preview = {
        let (invite_id, token) = (invite_id.clone(), token.clone());
        browser_load(move || preview_owner_invite(invite_id, token))
    };
    let accept = ServerAction::<AcceptOwnerInvite>::new();
    let accepted = move || accept.value().get().and_then(Result::ok);
    let error = move || {
        accept
            .value()
            .get()
            .and_then(Result::err)
            .or_else(|| preview.get().and_then(Result::err))
            .map(|e| e.to_string())
    };
    let ids = StoredValue::new((invite_id, token));

    view! {
        <section class=PANEL>
            {move || error().map(|e| view! {
                <p class=BANNER_ERROR>{e}" Sign in with the email the firm invited, and open the full link they sent."</p>
            })}
            {move || preview.get().and_then(Result::ok).map(|p| {
                let open = p.status == "open";
                let can_accept = open && p.email_matches;
                let line = format!(
                    "{} invites you to own {} ····{}.",
                    if p.firm_name.is_empty() { "A firm".to_owned() } else { p.firm_name.clone() },
                    p.profile_name,
                    p.tin_last4,
                );
                let note = if !open {
                    format!("This invite is {}.", p.status)
                } else if !p.email_matches {
                    format!("This invite is for {}. Sign in with that email to accept.", p.email)
                } else {
                    "You will own it: you can publish, edit, see everything, and end the firm's access whenever you choose. The firm keeps working on it under a Full pass without publishing until you change or end that.".to_owned()
                };
                view! {
                    <p class=RESULT_LINE><strong>{line}</strong></p>
                    <p class=RESULT_LINE>{note}</p>
                    <Show when=move || can_accept && accepted().is_none()>
                        <button class=BTN_PRIMARY type="button" disabled=move || accept.pending().get()
                            on:click=move |_| {
                                let (invite_id, token) = ids.get_value();
                                accept.dispatch(AcceptOwnerInvite { invite_id, token });
                            }>"Accept and own it"</button>
                    </Show>
                }
            })}
            {move || accepted().map(|profile| view! {
                <p class=BANNER_OK>
                    {format!("You now own {} ····{}. ", profile.display_name, profile.tin_last4)}
                    <a href="/tax-profiles">"Go to your tax profiles"</a>
                </p>
            })}
        </section>
    }
}

/// The firm's drafts (profiles it holds that no taxpayer owns yet) as
/// `(profile_id, label)` choices for the invite form.
fn draft_choices(profiles: Vec<crate::contracts::TaxProfileView>) -> Vec<(String, String)> {
    profiles
        .into_iter()
        .filter(|p| p.access == "holding_org")
        .map(|p| (p.profile_id, format!("{} ····{}", p.display_name, p.tin_last4)))
        .collect()
}

/// Whether the caller belongs to any workspace (the firm panel shows only then).
fn has_any_org(list: Option<&crate::contracts::OrganizationListResponse>) -> bool {
    list.is_some_and(|l| !l.organizations.is_empty())
}

/// `(invite_id, secret)` from the path and fragment of
/// `/invites/{id}#t={secret}`.
fn parse_invite_location(path: &str, hash: &str) -> (String, String) {
    let invite_id = path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned();
    let token = hash
        .trim_start_matches('#')
        .split('&')
        .find_map(|pair| pair.strip_prefix("t="))
        .unwrap_or_default()
        .to_owned();
    (invite_id, token)
}

/// `(invite_id, secret)` from the browser's location.
fn invite_from_location() -> (String, String) {
    #[cfg(feature = "hydrate")]
    {
        let location = web_sys::window().map(|w| w.location());
        let path = location
            .as_ref()
            .and_then(|l| l.pathname().ok())
            .unwrap_or_default();
        let hash = location
            .as_ref()
            .and_then(|l| l.hash().ok())
            .unwrap_or_default();
        parse_invite_location(&path, &hash)
    }
    #[cfg(not(feature = "hydrate"))]
    {
        (String::new(), String::new())
    }
}

fn browser_origin() -> String {
    #[cfg(feature = "hydrate")]
    {
        web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_default()
    }
    #[cfg(not(feature = "hydrate"))]
    {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_invite_link_gives_its_id_and_secret() {
        assert_eq!(
            parse_invite_location("/invites/i-1", "#t=s3cret"),
            ("i-1".to_owned(), "s3cret".to_owned())
        );
        assert_eq!(
            parse_invite_location("/invites/i-1/", "#x=1&t=abc&y=2"),
            ("i-1".to_owned(), "abc".to_owned())
        );
        assert_eq!(parse_invite_location("/invites/i-1", ""), ("i-1".to_owned(), String::new()));
    }

    #[test]
    fn only_the_firms_own_drafts_are_offered() {
        let view = |id: &str, access: &str| -> crate::contracts::TaxProfileView {
            let text = |key: &str| (key.to_owned(), serde_json::json!(""));
            let mut fields: serde_json::Map<String, serde_json::Value> = [
                "id", "full_name", "rdo_code", "address", "line_of_business", "zip_code", "phone",
                "email", "taxpayer_type", "holder_kind", "ownership_status", "verification_status",
                "claim_status",
            ]
            .into_iter()
            .map(text)
            .collect();
            fields.extend([
                ("profile_id".to_owned(), serde_json::json!(id)),
                ("access".to_owned(), serde_json::json!(access)),
                ("display_name".to_owned(), serde_json::json!("Client")),
                ("tin_last4".to_owned(), serde_json::json!("1234")),
                ("is_vat_registered".to_owned(), serde_json::json!(false)),
                ("revision".to_owned(), serde_json::json!(1)),
            ]);
            serde_json::from_value(serde_json::Value::Object(fields)).expect("view")
        };
        assert_eq!(
            draft_choices(vec![view("a", "holding_org"), view("b", "owner"), view("c", "pass")]),
            vec![("a".to_owned(), "Client ····1234".to_owned())]
        );
    }

    #[test]
    fn the_firm_panel_needs_a_workspace() {
        use crate::contracts::{OrganizationListResponse, OrganizationSummary};
        assert!(!has_any_org(None));
        assert!(!has_any_org(Some(&OrganizationListResponse { organizations: vec![] })));
        let org = OrganizationSummary {
            organization_id: "o".into(),
            name: "Firm".into(),
            slug: String::new(),
            status: "active".into(),
            current_user_role: "owner".into(),
            permissions: vec![],
            created_at_ms: 0,
        };
        assert!(has_any_org(Some(&OrganizationListResponse { organizations: vec![org] })));
    }
}
