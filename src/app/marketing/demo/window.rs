//! The OpenGrok app window: the coworker rail, the chat (header, thread, composer) and the
//! computer pane on the right (toolbar, the coworker's screen as a thumbnail, Routines). The
//! screen can grow out of the pane to fill the window and shrink back, driven from the script.
//! A sheet (Plugins) can cover the pane.
//!
//! Up to 1080px wide and 640px tall. Below `lg` the rail hides; below `md` the pane folds into a
//! strip under the chat header.

use std::rc::Rc;
use std::time::Duration;

use leptos::{ev, html, prelude::*};

use super::player::Aim;

/// The screen is laid out at this size, then scaled into the pane's thumbnail.
const VW: f64 = 960.;
const VH: f64 = 600.;
const GROW_MS: u32 = 380;
const SHRINK_MS: u32 = 340;

/// A coworker: rail avatar and chat header.
#[derive(Clone, Copy, PartialEq)]
pub struct Coworker {
    pub initials: &'static str,
    pub name: &'static str,
    /// Avatar background, a Tailwind class.
    pub color: &'static str,
}

pub const FINANCE: Coworker = Coworker {
    initials: "Fi",
    name: "Finance",
    color: "bg-[#f0a04b]",
};

const fn mate(initials: &'static str, color: &'static str) -> Coworker {
    Coworker {
        initials,
        name: initials,
        color,
    }
}

pub const FINANCE_RAIL: &[Coworker] = &[
    mate("OC", "bg-[#7dcea0]"),
    FINANCE,
    mate("Co", "bg-[#7c6cf0]"),
    mate("Tr", "bg-[#3b82f6]"),
];

/// What the coworker's screen draws: `true` for the full-window copy, `false` for thumbnails.
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

/// The coworker's screen: the pane's thumbnail and the full-window copy that grows out of it.
/// With reduced motion it crossfades instead.
#[derive(Clone, Copy)]
pub struct AgentScreen {
    zoom: RwSignal<Zoom>,
    style: RwSignal<String>,
    run: StoredValue<u32>,
    reduced: bool,
    big: NodeRef<html::Div>,
    thumb_wide: NodeRef<html::Div>,
    thumb_strip: NodeRef<html::Div>,
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
            thumb_wide: NodeRef::new(),
            thumb_strip: NodeRef::new(),
        }
    }

    /// Back in the pane at once, for a reset.
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

    /// The transform that puts the full-window screen exactly over the visible thumbnail.
    fn over_thumb(self) -> String {
        let Some(o) = self.big.get_untracked().map(|b| b.get_bounding_client_rect()) else {
            return String::new();
        };
        let t = [self.thumb_wide, self.thumb_strip]
            .into_iter()
            .find_map(|r| {
                let rect = r.get_untracked()?.get_bounding_client_rect();
                (rect.width() > 0.).then_some(rect)
            });
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

    /// The thumbnail grows to fill the window (FLIP: start over the thumbnail, animate to none).
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

    /// And back into the pane.
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

/// One icon in the pane's toolbar. It exists twice, in the pane and in the narrow strip; `aim`
/// points the cursor at whichever is on screen.
#[derive(Clone, Copy)]
pub struct PaneIcon {
    wide: NodeRef<html::Div>,
    strip: NodeRef<html::Div>,
    /// The cursor is on it: hover background and a ring.
    lit: Signal<bool>,
}

impl PaneIcon {
    pub fn new() -> Self {
        Self {
            wide: NodeRef::new(),
            strip: NodeRef::new(),
            lit: Signal::stored(false),
        }
    }

    pub fn aim(self) -> Aim {
        Aim::either(self.wide, self.strip)
    }

    pub fn lit(self, lit: impl Into<Signal<bool>>) -> Self {
        Self {
            lit: lit.into(),
            ..self
        }
    }
}

/// Monitor, plug (Plugins) and wrench (Tools).
#[derive(Clone, Copy)]
pub struct PaneIcons {
    pub monitor: PaneIcon,
    pub plug: PaneIcon,
    pub wrench: PaneIcon,
}

fn svg_monitor() -> impl IntoView {
    view! { <svg viewBox="0 0 24 24" class="h-[18px] w-[18px]" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="2"></rect><path d="M8 20h8M12 16v4"></path></svg> }
}
fn svg_plug() -> impl IntoView {
    view! { <svg viewBox="0 0 24 24" class="h-[18px] w-[18px]" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 3v5M15 3v5M7 8h10v3a5 5 0 0 1-10 0zM12 16v5"></path></svg> }
}
fn svg_wrench() -> impl IntoView {
    view! { <svg viewBox="0 0 24 24" class="h-[18px] w-[18px]" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"></path></svg> }
}

fn icon(i: PaneIcon, wide: bool, label: &'static str, glyph: impl IntoView + 'static) -> impl IntoView {
    let size = if wide { "h-8 w-8" } else { "h-7 w-7" };
    view! {
        <div
            node_ref=if wide { i.wide } else { i.strip }
            aria-label=label
            class=move || {
                let ring = if i.lit.get() { " bg-gc-bg-hover ring-2 ring-gc-accent/60" } else { "" };
                format!("relative grid shrink-0 place-items-center rounded-md text-gc-text-muted transition-colors duration-200 {size}{ring}")
            }
        >
            {glyph}
        </div>
    }
}

fn toolbar(icons: PaneIcons, wide: bool) -> impl IntoView {
    view! {
        {icon(icons.monitor, wide, "Screen", svg_monitor())}
        {icon(icons.plug, wide, "Plugins", svg_plug())}
        {icon(icons.wrench, wide, "Tools", svg_wrench())}
    }
}

/// The pane's thumbnail: the same screen, scaled to fit.
fn thumb(desktop: Desktop, r: NodeRef<html::Div>, class: &'static str) -> impl IntoView {
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
        <div node_ref=r class=format!("relative aspect-[16/10] shrink-0 overflow-hidden border border-gc-border {class}")>
            <div class="pointer-events-none absolute left-0 top-0 origin-top-left" style=move || format!("width:{VW}px;height:{VH}px;transform:scale({:.4})", k.get())>
                {desktop(false)}
            </div>
        </div>
    }
}

/// The OpenGrok window. `children` is the chat thread; `desktop` is the coworker's screen,
/// drawn in the thumbnail and, grown, over the whole window; `overlay` sits on top of it all.
#[component]
pub fn AgentWindow(
    /// The window itself: the cursor's stage and the autoplay trigger.
    win: NodeRef<html::Div>,
    screen: AgentScreen,
    who: Coworker,
    rail: &'static [Coworker],
    icons: PaneIcons,
    scroller: NodeRef<html::Div>,
    composer: NodeRef<html::Div>,
    /// What's being typed into the composer.
    #[prop(into)]
    draft: Signal<&'static str>,
    desktop: Desktop,
    /// The one routine in the pane: name and when.
    routine: (&'static str, &'static str),
    #[prop(into)] overlay: ViewFnOnce,
    /// A sheet over the computer pane, such as Plugins.
    #[prop(into)]
    sheet: ViewFn,
    #[prop(into)] sheet_open: Signal<bool>,
    children: Children,
) -> impl IntoView {
    let zoom = screen.zoom;

    view! {
        <div node_ref=win class="og-win relative z-[1] mx-auto flex h-[640px] w-full max-w-[1080px] select-none text-left text-[13px] leading-snug">
            <div class="hidden w-16 shrink-0 flex-col items-center gap-3 border-r border-gc-border bg-gc-bg-elevated py-3 lg:flex">
                <div class="flex gap-1">
                    <span class="h-2.5 w-2.5 rounded-full bg-[#ff5f57]"></span>
                    <span class="h-2.5 w-2.5 rounded-full bg-[#febc2e]"></span>
                    <span class="h-2.5 w-2.5 rounded-full bg-[#28c840]"></span>
                </div>
                {rail.iter().map(|c| view! {
                    <span class=format!(
                        "grid h-9 w-9 place-items-center rounded-full {} text-[11px] font-bold text-[#1a1a1a] {}",
                        c.color,
                        if *c == who { "ring-2 ring-gc-text/70 ring-offset-2 ring-offset-gc-bg-elevated" } else { "" }
                    )>{c.initials}</span>
                }).collect_view()}
                <span class="mt-auto grid h-8 w-8 place-items-center rounded-full bg-gc-bg-muted text-[10px] font-bold text-gc-text-muted">"JD"</span>
            </div>

            <div class="relative flex min-w-0 flex-1 flex-col bg-gc-bg-card">
                <div class="flex h-12 shrink-0 items-center gap-2 border-b border-gc-border px-4">
                    <span class=format!("grid h-8 w-8 place-items-center rounded-full {} text-[10px] font-bold text-[#1a1a1a]", who.color)>{who.initials}</span>
                    <span class="font-semibold text-gc-text">{who.name}</span>
                    <span class="text-[11px] text-gc-text-faint">"OpenGrok"</span>
                </div>

                <div class="relative flex shrink-0 items-center gap-2 border-b border-gc-border bg-gc-bg-elevated px-3 py-2 md:hidden">
                    {thumb(desktop.clone(), screen.thumb_strip, "w-[68px] rounded-md")}
                    <span class="hidden min-w-0 truncate text-[11.5px] text-gc-text-muted min-[400px]:inline">{format!("{}'s screen", who.name)}</span>
                    <div class="ml-auto flex items-center gap-0.5">{toolbar(icons, false)}</div>
                </div>

                <div node_ref=scroller class="min-h-0 flex-1 overflow-y-auto">
                    <div class="flex flex-col gap-3 px-4 py-5 text-[14px] md:px-6">{children()}</div>
                </div>
                <div class="shrink-0 px-4 pb-4 pt-1">
                    <div
                        node_ref=composer
                        class=move || if draft.get().is_empty() {
                            "min-h-11 rounded-[22px] border border-gc-border bg-gc-bg-muted px-[18px] py-2.5 text-[14px] text-gc-text-faint"
                        } else {
                            "min-h-11 rounded-[22px] border border-gc-border bg-gc-bg-muted px-[18px] py-2.5 text-[14px] text-gc-text"
                        }
                    >
                        {move || match draft.get() {
                            "" => view! { {format!("Message {}", who.name)} }.into_any(),
                            t => view! { {t}<span class="gc-caret"></span> }.into_any(),
                        }}
                    </div>
                </div>
            </div>

            <aside class="relative hidden w-[272px] shrink-0 flex-col border-l border-gc-border bg-gc-bg-elevated md:flex lg:w-[300px]">
                <div class="relative flex h-12 shrink-0 items-center gap-1 px-3">
                    {toolbar(icons, true)}
                </div>
                <div class="px-4 pt-2">
                    {thumb(desktop.clone(), screen.thumb_wide, "w-full rounded-xl")}
                    <div class="mt-2 text-center text-[12px] text-gc-text-muted">{format!("{}'s screen", who.name)}</div>
                </div>
                <div class="mt-6 px-4">
                    <div class="mb-3 text-[13px] text-gc-text-muted">"Routines"</div>
                    <div class="flex items-center gap-2 text-gc-text">
                        <span class="material-symbols-outlined text-[18px] text-gc-text-muted">"schedule"</span>
                        <span class="font-medium">{routine.0}</span>
                        <span class="text-[12px] text-gc-text-faint">{routine.1}</span>
                    </div>
                </div>
            </aside>

            <Show when=move || sheet_open.get()>
                <div class="gc-card-in absolute bottom-0 right-0 top-12 z-[15] flex w-[min(88%,300px)] flex-col border-l border-gc-border bg-gc-bg-elevated shadow-2xl md:top-0 md:w-[272px] md:shadow-none lg:w-[300px]">
                    {sheet.run()}
                </div>
            </Show>

            <div class=move || if zoom.get() == Zoom::Open {
                "pointer-events-none absolute inset-0 z-20 bg-black/45 opacity-100 transition-opacity duration-300"
            } else {
                "pointer-events-none absolute inset-0 z-20 bg-black/45 opacity-0 transition-opacity duration-300"
            }></div>
            <div
                node_ref=screen.big
                class=move || if zoom.get() == Zoom::Closed {
                    "invisible absolute inset-1.5 z-30 origin-top-left overflow-hidden rounded-2xl md:inset-3"
                } else {
                    "absolute inset-1.5 z-30 origin-top-left overflow-hidden rounded-2xl shadow-[0_24px_80px_rgba(0,0,0,.55)] will-change-transform md:inset-3"
                }
                style=move || screen.style.get()
                aria-hidden=move || (zoom.get() == Zoom::Closed).to_string()
            >
                {desktop(true)}
            </div>

            {overlay.run()}
        </div>
    }
}

/// The coworker's desktop: a top bar with a title, whatever runs on it, and a dock.
pub fn box_desktop(title: impl IntoView + 'static, body: impl IntoView + 'static) -> impl IntoView {
    view! {
        <div class="lx-desk relative h-full w-full select-none overflow-hidden text-left">
            <div class="absolute inset-x-0 top-0 z-10 flex h-8 items-center justify-between gap-2 bg-black/35 px-3 text-[11px] text-white/90">
                <span class="font-medium">"Activities"</span>
                <span class="truncate">{title}</span>
                <span class="flex items-center gap-2">
                    <span class="material-symbols-outlined text-[14px]">"wifi"</span>
                    <span class="material-symbols-outlined text-[14px]">"battery_full"</span>
                </span>
            </div>
            {body}
            <div class="absolute bottom-2.5 left-1/2 flex w-max -translate-x-1/2 items-center gap-2 rounded-2xl border border-white/15 bg-black/40 px-2 py-1.5">
                {["public", "terminal", "folder", "receipt_long"]
                    .into_iter()
                    .map(|i| view! { <span class="lx-dock-item material-symbols-outlined text-[20px] text-[#cbd5e1]">{i}</span> })
                    .collect_view()}
            </div>
        </div>
    }
}

/// A browser on the desktop: a tab, the address bar, and the page.
pub fn chrome(tab: &'static str, host: &'static str, path: &'static str, page: impl IntoView + 'static) -> impl IntoView {
    view! {
        <div class="absolute bottom-[68px] left-1/2 top-11 flex w-[min(94%,780px)] -translate-x-1/2 flex-col overflow-hidden rounded-[10px] bg-white shadow-[0_18px_40px_rgba(0,0,0,.45)] ring-1 ring-black/20">
            <div class="flex h-8 shrink-0 items-end gap-1 bg-[#dee1e6] pl-2 pr-1">
                <span class="flex min-w-0 max-w-[230px] items-center gap-1.5 rounded-t-lg bg-white px-3 py-1.5 text-[11.5px] text-[#202124]">
                    <span class="grid h-3.5 w-3.5 place-items-center rounded-[3px] bg-[#0f766e] text-[8px] font-black text-white">"B"</span>
                    <span class="truncate">{tab}</span>
                </span>
            </div>
            <div class="flex h-9 shrink-0 items-center gap-2 border-b border-[#e5e7eb] px-2.5 text-[#5f6368]">
                <span class="material-symbols-outlined text-[17px]">"arrow_back"</span>
                <span class="material-symbols-outlined text-[17px]">"refresh"</span>
                <span class="flex min-w-0 flex-1 items-center gap-1.5 rounded-full bg-[#f1f3f4] px-3 py-1 text-[12.5px]">
                    <span class="material-symbols-outlined text-[14px]">"lock"</span>
                    <span class="truncate"><span class="text-[#202124]">{host}</span>{path}</span>
                </span>
            </div>
            <div class="relative min-h-0 flex-1 overflow-hidden bg-[#f4f4f5]">
                {page}
            </div>
        </div>
    }
}
