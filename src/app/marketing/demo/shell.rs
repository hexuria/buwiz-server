//! The desktop app window the home-page demos share, drawn after Buwiz's AI Co-worker app:
//! a sidebar (the co-worker's tile, Chats, Mini Apps and Plugins), the chat (a floating name
//! chip, the thread, the composer pill) and the details pane (avatar, model, tabs, the Computer
//! thumbnail, connected plugins and Routines).
//!
//! The Computer screen can grow out of the pane to fill the window and shrink back; it shows
//! whatever the co-worker has open (a browser, a spreadsheet, a PDF).
//!
//! Up to 1080px wide and 640px tall. Below `lg` the sidebar hides; below `md` the pane hides.

use std::rc::Rc;
use std::time::Duration;

use leptos::{ev, html, prelude::*};

use super::kit::Rig;
use super::player::DemoPointer;

/// The Computer screen is laid out at this size, then scaled into the pane's thumbnail.
const VW: f64 = 960.;
const VH: f64 = 600.;
const GROW_MS: u32 = 380;
const SHRINK_MS: u32 = 340;

/// What the Computer screen draws: `true` for the full-window copy, `false` for the thumbnail.
/// Only the full-window copy should attach node refs the cursor aims at.
pub type Desktop = Rc<dyn Fn(bool) -> AnyView>;

#[derive(Clone, Copy, PartialEq)]
enum Zoom {
    Closed,
    Open,
    Closing,
}

fn two_frames(f: impl FnOnce() + 'static) {
    request_animation_frame(move || request_animation_frame(f));
}

/// The Computer screen: the pane's thumbnail and the full-window copy that grows out of it.
/// With reduced motion it crossfades instead.
#[derive(Clone, Copy)]
pub struct AgentScreen {
    zoom: RwSignal<Zoom>,
    style: RwSignal<String>,
    run: StoredValue<u32>,
    reduced: bool,
    big: NodeRef<html::Div>,
    thumb: NodeRef<html::Div>,
}

impl AgentScreen {
    pub fn new() -> Self {
        let reduced = window()
            .match_media("(prefers-reduced-motion: reduce)")
            .ok()
            .flatten()
            .is_some_and(|m| m.matches());
        Self {
            zoom: RwSignal::new(Zoom::Closed),
            style: RwSignal::new(String::new()),
            run: StoredValue::new(0),
            reduced,
            big: NodeRef::new(),
            thumb: NodeRef::new(),
        }
    }

    pub fn reset(self) {
        self.bump();
        self.zoom.set(Zoom::Closed);
        self.style.set(String::new());
    }

    fn bump(self) -> u32 {
        self.run
            .try_update_value(|g| {
                *g += 1;
                *g
            })
            .unwrap_or_default()
    }

    /// The transform that puts the full-window screen exactly over the thumbnail.
    fn over_thumb(self) -> String {
        let Some(o) = self.big.get_untracked().map(|b| b.get_bounding_client_rect()) else {
            return String::new();
        };
        let t = self
            .thumb
            .get_untracked()
            .map(|r| r.get_bounding_client_rect())
            .filter(|r| r.width() > 0.);
        match t {
            Some(t) if o.width() > 0. && o.height() > 0. => format!(
                "transform:translate({:.1}px,{:.1}px) scale({:.4},{:.4});",
                t.left() - o.left(),
                t.top() - o.top(),
                t.width() / o.width(),
                t.height() / o.height()
            ),
            _ => String::new(),
        }
    }

    pub fn expand(self) {
        let run = self.bump();
        let (g, style) = (self.run, self.style);
        let (from, to) = if self.reduced {
            (
                "opacity:0;transition:none;".to_string(),
                "opacity:1;transition:opacity 220ms ease-out;".to_string(),
            )
        } else {
            (
                format!("{}opacity:.55;transition:none;", self.over_thumb()),
                format!("transform:none;opacity:1;transition:transform {GROW_MS}ms cubic-bezier(.2,.8,.2,1),opacity 160ms ease-out;"),
            )
        };
        style.set(from);
        self.zoom.set(Zoom::Open);
        two_frames(move || {
            if g.try_get_value() == Some(run) {
                style.set(to);
            }
        });
    }

    pub fn shrink(self) {
        let run = self.bump();
        let to = if self.reduced {
            "opacity:0;transition:opacity 200ms ease-in;".to_string()
        } else {
            format!(
                "{}opacity:.4;transition:transform {SHRINK_MS}ms cubic-bezier(.4,0,.2,1),opacity {SHRINK_MS}ms ease-in;",
                self.over_thumb()
            )
        };
        self.zoom.set(Zoom::Closing);
        self.style.set(to);
        let (g, zoom, style) = (self.run, self.zoom, self.style);
        set_timeout(
            move || {
                if g.try_get_value() == Some(run) {
                    zoom.set(Zoom::Closed);
                    style.set(String::new());
                }
            },
            Duration::from_millis((SHRINK_MS + 30) as u64),
        );
    }
}

/// The co-worker's avatar: a rounded tile in its color.
pub fn avatar(size: &'static str, icon_size: &'static str) -> impl IntoView {
    view! {
        <span class=format!("grid shrink-0 place-items-center rounded-[28%] bg-[#14C370] text-white {size}")>
            <span class=format!("material-symbols-outlined {icon_size}")>"receipt_long"</span>
        </span>
    }
}

fn thumb(desktop: Desktop, r: NodeRef<html::Div>) -> impl IntoView {
    let k = RwSignal::new(0.);
    let measure = move || {
        if let Some(el) = r.get_untracked() {
            let w = el.get_bounding_client_rect().width();
            if w > 0. {
                k.set(w / VW);
            }
        }
    };
    Effect::new(move |_| {
        if r.get().is_some() {
            request_animation_frame(measure);
        }
    });
    let resize = window_event_listener(ev::resize, move |_| measure());
    on_cleanup(move || resize.remove());
    view! {
        <div node_ref=r class="gs-line relative aspect-[16/10] w-full overflow-hidden rounded-xl border">
            <div class="pointer-events-none absolute left-0 top-0 origin-top-left" style=move || format!("width:{VW}px;height:{VH}px;transform:scale({:.4})", k.get())>
                {desktop(false)}
            </div>
        </div>
    }
}

const PANE_LABEL: &str = "gs-f mb-1.5 mt-4 w-full text-[11px] font-semibold";

/// The app window. `children` is the chat thread; `desktop` is the Computer screen; `overlay`
/// sits over everything but the Touch ID sheet and the pointer.
#[component]
pub fn ShellWindow(
    rig: Rig,
    desktop: Desktop,
    /// The chats listed in the sidebar; the first is the open one.
    chats: &'static [&'static str],
    #[prop(optional, into)] overlay: Option<ViewFnOnce>,
    children: Children,
) -> impl IntoView {
    let zoom = rig.screen.zoom;
    let draft = rig.draft;
    let lit_plugins = move || rig.lit.get() == super::kit::LIT_PLUGINS;
    let lit_plus = move || rig.lit.get() == super::kit::LIT_PLUS;

    view! {
        <div node_ref=rig.win class="gs-win gs-chat gs-t relative z-[1] mx-auto flex h-[640px] w-full max-w-[1080px] select-none overflow-hidden rounded-[18px] text-left text-[13px] leading-snug">
            <div class="gs-side gs-line hidden w-[216px] shrink-0 flex-col border-r px-3 pb-3 pt-3 lg:flex">
                <div class="flex items-center gap-1.5">
                    <span class="h-3 w-3 rounded-full bg-[#ff5f57]"></span>
                    <span class="h-3 w-3 rounded-full bg-[#febc2e]"></span>
                    <span class="h-3 w-3 rounded-full bg-[#28c840]"></span>
                    <span class="gs-hover gs-d ml-auto grid h-7 w-7 place-items-center rounded-full">
                        <span class="material-symbols-outlined text-[16px]">"view_sidebar"</span>
                    </span>
                    <span class="gs-hover gs-d grid h-7 w-7 place-items-center rounded-full">
                        <span class="material-symbols-outlined text-[16px]">"add"</span>
                    </span>
                </div>
                <div class="gs-hover mx-auto mt-4 flex w-[96px] flex-col items-center gap-1 rounded-xl py-2.5">
                    {avatar("h-11 w-11", "text-[24px]")}
                    <span class="text-[11px] font-medium">"Tax Co-worker"</span>
                </div>
                <div class="gs-f mb-1.5 mt-4 px-1 text-[11px] font-semibold">"Chats"</div>
                {chats.iter().enumerate().map(|(i, title)| view! {
                    <div class=if i == 0 { "gs-hover flex items-center gap-2 rounded-lg px-1.5 py-1.5" } else { "flex items-center gap-2 rounded-lg px-1.5 py-1.5" }>
                        <span class="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-[#14C370] text-[10px] font-bold text-white">{&title[..1]}</span>
                        <span class="min-w-0 truncate text-[12px]">{*title}</span>
                    </div>
                }).collect_view()}
                <div class="mt-auto flex items-center gap-1.5">
                    <span class="gs-hover grid h-7 w-7 place-items-center rounded-full text-[10px] font-semibold">"UI"</span>
                    <span class="gs-hover flex h-7 items-center gap-1 whitespace-nowrap rounded-full px-2.5 text-[11.5px] font-medium">
                        "Mini Apps"<span class="material-symbols-outlined gs-d text-[14px]">"apps"</span>
                    </span>
                    <div node_ref=rig.plugins_btn class=move || if lit_plugins() {
                        "gs-hover flex h-7 items-center gap-1 whitespace-nowrap rounded-full px-2.5 text-[11.5px] font-medium ring-2 ring-[#14C370]/70"
                    } else {
                        "gs-hover flex h-7 items-center gap-1 whitespace-nowrap rounded-full px-2.5 text-[11.5px] font-medium"
                    }>
                        <span class="material-symbols-outlined gs-d text-[14px]">"extension"</span>"Plugins"
                    </div>
                </div>
            </div>

            <div class="relative flex min-w-0 flex-1 flex-col">
                <div class="gs-fade pointer-events-none absolute inset-x-0 top-0 z-[4] h-14"></div>
                <div class="gs-bubble gs-line absolute left-1/2 top-2.5 z-[5] flex -translate-x-1/2 items-center gap-1.5 rounded-full border px-2.5 py-1 text-[12px] font-semibold shadow-sm">
                    {avatar("h-[18px] w-[18px]", "text-[12px]")}
                    "Tax Co-worker"
                </div>
                <div node_ref=rig.scroller class="min-h-0 flex-1 overflow-y-auto">
                    <div class="flex flex-col gap-2.5 px-4 pb-4 pt-14 text-[13.5px] md:px-8">{children()}</div>
                </div>
                <div class="shrink-0 px-4 pb-4 pt-1 md:px-8">
                    <div class="gs-bubble gs-line flex min-h-11 items-center gap-2 rounded-[22px] border py-1.5 pl-1.5 pr-1.5">
                        <div node_ref=rig.plus_btn class=move || if lit_plus() {
                            "gs-hover grid h-8 w-8 shrink-0 place-items-center rounded-full ring-2 ring-[#14C370]/70"
                        } else {
                            "gs-hover grid h-8 w-8 shrink-0 place-items-center rounded-full"
                        }>
                            <span class="material-symbols-outlined gs-d text-[18px]">"add"</span>
                        </div>
                        <div node_ref=rig.composer class="min-w-0 flex-1 truncate text-[13.5px]">
                            {move || match draft.get() {
                                "" => view! { <span class="gs-f">"Message Tax Co-worker"</span> }.into_any(),
                                t => view! { <span>{t}<span class="gc-caret"></span></span> }.into_any(),
                            }}
                        </div>
                        <span class="gs-hover gs-d hidden h-7 items-center gap-0.5 rounded-full px-2.5 text-[11.5px] sm:flex">
                            "Auto"<span class="material-symbols-outlined text-[14px]">"expand_more"</span>
                        </span>
                        <span class="material-symbols-outlined gs-d text-[18px]">"mic"</span>
                        <span class="grid h-8 w-8 shrink-0 place-items-center rounded-full bg-[#14C370] text-white">
                            <span class="material-symbols-outlined text-[18px]">"graphic_eq"</span>
                        </span>
                    </div>
                </div>
            </div>

            <aside class="gs-pane gs-line relative hidden w-[264px] shrink-0 flex-col items-center overflow-hidden border-l px-4 pt-6 md:flex">
                {avatar("h-[60px] w-[60px]", "text-[32px]")}
                <div class="mt-2 text-[14px] font-semibold">"Tax Co-worker"</div>
                <div class="gs-f text-[11px]">"Add a label"</div>
                <span class="gs-hover gs-d mt-2 flex items-center gap-0.5 rounded-full px-2.5 py-1 text-[11px]">
                    "Auto"<span class="material-symbols-outlined text-[14px]">"expand_more"</span>
                </span>
                <div class="mt-3 flex items-center gap-0.5 text-[11.5px]">
                    <span class="gs-hover rounded-full px-2.5 py-1 font-medium">"Details"</span>
                    <span class="gs-d px-2 py-1">"Library"</span>
                    <span class="gs-d px-2 py-1">"Computer"</span>
                    <span class="gs-d px-2 py-1">"Connect"</span>
                </div>
                <div class=PANE_LABEL>"Computer"</div>
                {thumb(desktop.clone(), rig.screen.thumb)}
                <div class=PANE_LABEL>"Plugins"</div>
                <div class="flex w-full flex-wrap gap-1.5">
                    {move || {
                        let on = rig.plugins.get();
                        if on.is_empty() {
                            view! { <span class="gs-f text-[11.5px]">"None yet"</span> }.into_any()
                        } else {
                            on.into_iter().map(|p| view! {
                                <span class="gs-card gc-card-in flex items-center gap-1 rounded-full py-0.5 pl-1 pr-2 text-[11.5px] font-medium">
                                    {super::kit::plugin_mark(p)}{p}
                                </span>
                            }).collect_view().into_any()
                        }
                    }}
                </div>
                <div class=PANE_LABEL>"Routines"</div>
                {move || {
                    let list = rig.routines.get();
                    if list.is_empty() {
                        view! {
                            <div class="gs-hover gs-d w-full rounded-xl px-3 py-2.5 text-[11.5px] leading-snug">
                                "No routines yet. Ask Tax Co-worker in chat to run something on a schedule."
                            </div>
                        }.into_any()
                    } else {
                        list.into_iter().map(|r| view! {
                            <div class="gs-card gc-card-in mb-1.5 w-full rounded-xl px-3 py-2">
                                <div class="flex items-center gap-1.5 text-[12px] font-semibold">
                                    <span class="material-symbols-outlined text-[15px] text-[#14C370]">"schedule"</span>
                                    {r.name}
                                </div>
                                <div class="gs-d mt-0.5 text-[11px]">{r.when}</div>
                                <div class="gs-f mt-0.5 text-[11px]">{move || r.note.get()}</div>
                            </div>
                        }).collect_view().into_any()
                    }
                }}
            </aside>

            <div class=move || if zoom.get() == Zoom::Open {
                "pointer-events-none absolute inset-0 z-20 bg-black/45 opacity-100 transition-opacity duration-300"
            } else {
                "pointer-events-none absolute inset-0 z-20 bg-black/45 opacity-0 transition-opacity duration-300"
            }></div>
            <div
                node_ref=rig.screen.big
                class=move || if zoom.get() == Zoom::Closed {
                    "invisible absolute inset-1.5 z-30 origin-top-left overflow-hidden rounded-2xl md:inset-3"
                } else {
                    "absolute inset-1.5 z-30 origin-top-left overflow-hidden rounded-2xl shadow-[0_24px_80px_rgba(0,0,0,.55)] will-change-transform md:inset-3"
                }
                style=move || rig.screen.style.get()
                aria-hidden=move || (zoom.get() == Zoom::Closed).to_string()
            >
                {desktop(true)}
            </div>

            {overlay.map(|o| o.run())}
            {super::kit::touch_sheet(rig)}
            <DemoPointer cursor=rig.cur />
        </div>
    }
}

/// An idle Computer: nothing open.
pub fn idle_screen() -> impl IntoView {
    view! {
        <div class="grid h-full w-full place-items-center bg-[#0b0b0c] text-white/40">
            <div class="flex flex-col items-center gap-2">
                <span class="material-symbols-outlined text-[48px]">"computer"</span>
                <span class="text-[15px]">"Computer is idle"</span>
            </div>
        </div>
    }
}

/// A file open in the viewer: a title bar with the file's name and actions, then the file.
pub fn viewer(
    kind: &'static str,
    name: &'static str,
    actions: impl IntoView + 'static,
    body: impl IntoView + 'static,
) -> impl IntoView {
    let (badge, tone) = match kind {
        "xlsx" => ("XLSX", "bg-[#1d6f42]"),
        _ => ("PDF", "bg-[#d93025]"),
    };
    view! {
        <div class="flex h-full w-full flex-col bg-[#2b2b2e] text-[#f5f5f5]">
            <div class="flex h-11 shrink-0 items-center gap-2.5 border-b border-white/10 bg-[#1c1c1e] px-4">
                <span class=format!("grid h-6 w-8 place-items-center rounded {tone} text-[9px] font-black text-white")>{badge}</span>
                <span class="text-[13px] font-semibold">{name}</span>
                <span class="ml-auto flex items-center gap-2">{actions}</span>
            </div>
            <div class="relative min-h-0 flex-1 overflow-hidden">{body}</div>
        </div>
    }
}

/// A browser on the co-worker's computer.
pub fn browser(tab: &'static str, host: &'static str, path: &'static str, body: impl IntoView + 'static) -> impl IntoView {
    view! {
        <div class="flex h-full w-full flex-col bg-white text-[#202124]">
            <div class="flex h-9 shrink-0 items-end gap-1 bg-[#dee1e6] pl-3">
                <span class="flex max-w-[260px] items-center gap-1.5 rounded-t-lg bg-white px-3 py-1.5 text-[12px]">
                    <span class="material-symbols-outlined text-[14px] text-[#5f6368]">"public"</span>
                    <span class="truncate">{tab}</span>
                </span>
            </div>
            <div class="flex h-10 shrink-0 items-center gap-2 border-b border-[#e5e7eb] px-3 text-[#5f6368]">
                <span class="material-symbols-outlined text-[18px]">"arrow_back"</span>
                <span class="material-symbols-outlined text-[18px]">"refresh"</span>
                <span class="flex min-w-0 flex-1 items-center gap-1.5 rounded-full bg-[#f1f3f4] px-3 py-1 text-[13px]">
                    <span class="material-symbols-outlined text-[14px]">"lock"</span>
                    <span class="truncate"><span class="text-[#202124]">{host}</span>{path}</span>
                </span>
            </div>
            <div class="relative min-h-0 flex-1 overflow-hidden">{body}</div>
        </div>
    }
}
