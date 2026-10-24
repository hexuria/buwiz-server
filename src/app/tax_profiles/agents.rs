//! Connected agents: AI apps (Claude Code, Codex, Grok and the like) the
//! taxpayer allowed on some of their profiles, each with an End button.

use crate::app::{RevokeAgentConnection, SetAgentMayQueue, browser_load, list_agent_connections};
use crate::contracts::{AgentConnectionListResponse, AgentConnectionView};
use crate::ui::classes::{BANNER_ERROR, BTN_SECONDARY, PANEL, RESULT_LINE, SECTION_LABEL};
use leptos::prelude::*;

#[island]
pub fn AgentsPanel() -> impl IntoView {
    let loaded = browser_load(list_agent_connections);
    let list = RwSignal::new(None::<AgentConnectionListResponse>);
    let revoke = ServerAction::<RevokeAgentConnection>::new();
    let queue = ServerAction::<SetAgentMayQueue>::new();

    let take =
        move |value: Option<Result<AgentConnectionListResponse, server_fn::ServerFnError>>| {
            if let Some(Ok(value)) = value {
                list.set(Some(value));
            }
        };
    Effect::new(move |_| take(loaded.get()));
    Effect::new(move |_| take(revoke.value().get()));
    Effect::new(move |_| take(queue.value().get()));

    let error = move || {
        revoke
            .value()
            .get()
            .and_then(Result::err)
            .map(|e| e.to_string())
    };
    let connections = move || list.get().map(|l| l.connections).unwrap_or_default();

    view! {
        <section class=PANEL>
            <p class=SECTION_LABEL>"Connected agents"</p>
            <p class=RESULT_LINE>
                "AI apps you allowed on your profiles through MCP. An agent never gets more than you have, and stops the moment you end it. Ending one does not sign you out. Allow queue lets that agent leave a filing for your computer. Your app sends it. This server never contacts BIR."
            </p>
            {move || error().map(|e| view! { <p class=BANNER_ERROR>{e}</p> })}
            <Show when=move || connections().is_empty()>
                <p class=RESULT_LINE>"No agents connected."</p>
            </Show>
            <ul>
                <For
                    each=connections
                    key=|c| c.connection_id.clone()
                    children=move |c: AgentConnectionView| {
                        let id = c.connection_id.clone();
                        let queue_id = c.connection_id.clone();
                        let may_queue = c.may_queue;
                        view! {
                            <li class="mb-4 rounded-xl border border-border-subtle p-4">
                                <p class=RESULT_LINE>{line(&c)}</p>
                                <div class="mt-2 flex flex-wrap gap-2">
                                    <button class=BTN_SECONDARY type="button" on:click=move |_| {
                                        queue.dispatch(SetAgentMayQueue {
                                            connection_id: queue_id.clone(),
                                            may_queue: !may_queue,
                                        });
                                    }>{if may_queue { "Stop queue" } else { "Allow queue" }}</button>
                                    <button class=BTN_SECONDARY type="button" on:click=move |_| {
                                        revoke.dispatch(RevokeAgentConnection { connection_id: id.clone() });
                                    }>"End"</button>
                                </div>
                            </li>
                        }
                    }
                />
            </ul>
        </section>
    }
}

fn line(c: &AgentConnectionView) -> String {
    let what = match c.preset.as_deref() {
        Some("read_only") => "Read-only".to_owned(),
        Some("safe") => "Safe".to_owned(),
        Some("full") => "Full".to_owned(),
        _ => format!("custom ({} permissions)", c.permissions.len()),
    };
    let profiles = c
        .profiles
        .iter()
        .map(|p| format!("{} ····{}", p.profile_name, p.tin_last4))
        .collect::<Vec<_>>()
        .join(", ");
    let used = c
        .last_used_at
        .as_deref()
        .map(|t| format!(" · last used {}", t.get(..10).unwrap_or(t)))
        .unwrap_or_else(|| " · not used yet".to_owned());
    let queue = if c.may_queue { "queue on" } else { "queue off" };
    format!(
        "{} (names itself) · {what} · {queue} · {profiles} · allowed {}{used}",
        c.client_name,
        c.created_at.get(..10).unwrap_or(&c.created_at),
    )
}
