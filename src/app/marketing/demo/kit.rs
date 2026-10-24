//! What every demo film is made of: the rig (timeline, cursor, window refs and the pane's
//! state), the chat thread, the messages every film uses (your bubble, the co-worker's bubble,
//! a tool call, a file), cards, the login card and the Touch ID sheet.

use std::time::Duration;

use leptos::{ev, html, prelude::*};

use super::player::{in_view, Aim, DemoCursor, PlayState, Script, Timeline};
use super::shell::AgentScreen;

/// Cursor targets the window itself owns. Films number theirs from `LIT_FILM`.
pub const LIT_NONE: u8 = 0;
pub const LIT_PLUGINS: u8 = 1;
pub const LIT_PLUS: u8 = 2;
pub const LIT_FILM: u8 = 10;

/// The details pane's tabs.
#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Details,
    Library,
    Computer,
    Connect,
}

/// A routine in the pane.
#[derive(Clone, Copy)]
pub struct Routine {
    pub name: &'static str,
    pub when: &'static str,
    /// Its last run.
    pub note: RwSignal<&'static str>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Touch {
    Off,
    Waiting,
    Ok,
}

#[derive(Clone, Copy)]
pub struct Rig {
    pub tl: Timeline,
    pub cur: DemoCursor,
    pub win: NodeRef<html::Div>,
    pub screen: AgentScreen,
    pub scroller: NodeRef<html::Div>,
    pub composer: NodeRef<html::Div>,
    pub plus_btn: NodeRef<html::Div>,
    pub plugins_btn: NodeRef<html::Div>,
    pub act: RwSignal<Option<usize>>,
    pub caption: RwSignal<&'static str>,
    pub draft: RwSignal<&'static str>,
    pub lit: RwSignal<u8>,
    /// The id of the tool call still running; 0 when none is.
    pub busy: RwSignal<u32>,
    pub touch: RwSignal<Touch>,
    /// What the Touch ID sheet says it fills: (account, site).
    pub touch_for: RwSignal<(&'static str, &'static str)>,
    pub plugins: RwSignal<Vec<&'static str>>,
    pub routines: RwSignal<Vec<Routine>>,
    pub tab: RwSignal<Tab>,
}

impl Rig {
    pub fn new() -> Self {
        let win = NodeRef::new();
        let tl = Timeline::new();
        tl.speed.set(2.);
        Self {
            tl,
            cur: DemoCursor::new(tl, win, (0.62, 0.95)),
            win,
            screen: AgentScreen::new(),
            scroller: NodeRef::new(),
            composer: NodeRef::new(),
            plus_btn: NodeRef::new(),
            plugins_btn: NodeRef::new(),
            act: RwSignal::new(None),
            caption: RwSignal::new(""),
            draft: RwSignal::new(""),
            lit: RwSignal::new(LIT_NONE),
            busy: RwSignal::new(0),
            touch: RwSignal::new(Touch::Off),
            touch_for: RwSignal::new(("", "")),
            plugins: RwSignal::new(Vec::new()),
            routines: RwSignal::new(Vec::new()),
            tab: RwSignal::new(Tab::Details),
        }
    }

    /// Back to the top of a run, with the plugins and routines the film starts with.
    pub fn reset(self, plugins: &[&'static str], routines: Vec<Routine>) {
        self.draft.set("");
        self.lit.set(LIT_NONE);
        self.busy.set(0);
        self.touch.set(Touch::Off);
        self.plugins.set(plugins.to_vec());
        self.routines.set(routines);
        self.tab.set(Tab::Details);
        self.screen.reset();
        self.cur.reset();
    }

    /// The plugins are now `plugins`; the pane shows them under Connect.
    pub fn connect(self, plugins: &[&'static str]) {
        self.plugins.set(plugins.to_vec());
        self.tab.set(Tab::Connect);
    }

    /// Adds a routine; the pane shows it under Details.
    pub fn add_routine(self, routine: Routine) {
        self.routines.update(|v| v.push(routine));
        self.tab.set(Tab::Details);
    }

    /// Grows the Computer screen out of the Computer tab's thumbnail.
    pub fn open_screen(self) {
        self.tab.set(Tab::Computer);
        self.screen.expand();
    }

    pub fn act(self, n: usize, caption: &'static str) {
        self.act.set(Some(n));
        self.caption.set(caption);
    }

    pub fn lit_is(self, id: u8) -> impl Fn() -> bool + Copy + Send + Sync {
        let lit = self.lit;
        move || lit.get() == id
    }

    /// `base`, ringed while the cursor rests on `id`.
    pub fn lit_class(self, id: u8, base: &'static str) -> impl Fn() -> String + Copy + Send + Sync {
        let on = self.lit_is(id);
        move || {
            if on() {
                format!("{base} ring-2 ring-[#14C370]/70 ring-offset-1 ring-offset-transparent")
            } else {
                base.to_string()
            }
        }
    }

    /// Rest on `aim` with `id` lit, press, then `f`.
    pub fn tap(self, s: &mut Script, aim: impl Into<Aim>, id: u8, f: impl FnOnce() + 'static) {
        s.move_to(self.cur, aim)
            .then(move || self.lit.set(id))
            .wait(220)
            .press(self.cur)
            .wait(170)
            .then(move || {
                self.lit.set(LIT_NONE);
                f();
            })
            .wait(320);
    }

    /// Type `text` into the composer, then `send` (which pushes the bubble).
    pub fn say(self, s: &mut Script, text: &'static str, send: impl FnOnce() + 'static) {
        s.move_to(self.cur, self.composer)
            .wait(150)
            .typing(text, 28, move |t| self.draft.set(t))
            .wait(300)
            .then(move || {
                self.draft.set("");
                send();
            });
    }

    /// Touch ID: the sheet comes up, reads a finger, and goes.
    pub fn touch_id(self, s: &mut Script, account: &'static str, site: &'static str) {
        s.then(move || {
            self.touch_for.set((account, site));
            self.touch.set(Touch::Waiting);
        })
        .wait(1300)
        .then(move || self.touch.set(Touch::Ok))
        .wait(600)
        .then(move || self.touch.set(Touch::Off))
        .wait(300);
    }

    /// The end of a run: hold the last frame, then play again if still on screen.
    pub fn end(self, s: &mut Script, caption: &'static str) {
        s.then(move || self.caption.set(caption))
            .wait(4500)
            .then(move || {
                if in_view(self.win) {
                    self.tl.play();
                }
            });
    }

    /// Starts the first time the window scrolls into view, and again after a run that ended
    /// off screen.
    pub fn autoplay(self) {
        let (tl, win) = (self.tl, self.win);
        let start = move || {
            if matches!(
                tl.state.try_get_untracked(),
                Some(PlayState::Idle | PlayState::Done)
            ) && in_view(win)
            {
                tl.play();
            }
        };
        Effect::new(move |_| set_timeout(start, Duration::from_millis(300)));
        let scroll = window_event_listener(ev::scroll, move |_| start());
        on_cleanup(move || scroll.remove());
    }
}

/// A film's chat thread.
pub struct Chat<T: Send + Sync + 'static> {
    msgs: RwSignal<Vec<(u32, T)>>,
    next: StoredValue<u32>,
    scroller: NodeRef<html::Div>,
}

impl<T: Send + Sync + 'static> Clone for Chat<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Send + Sync + 'static> Copy for Chat<T> {}

impl<T: Clone + Send + Sync + 'static> Chat<T> {
    pub fn new(rig: Rig) -> Self {
        Self {
            msgs: RwSignal::new(Vec::new()),
            next: StoredValue::new(0),
            scroller: rig.scroller,
        }
    }

    pub fn clear(self) {
        self.msgs.set(Vec::new());
    }

    pub fn list(self) -> Vec<(u32, T)> {
        self.msgs.get()
    }

    pub fn push(self, body: T) {
        self.push_id(body);
    }

    /// Adds `body` to the thread and scrolls down; returns its id.
    pub fn push_id(self, body: T) -> u32 {
        let id = self
            .next
            .try_update_value(|n| {
                *n += 1;
                *n
            })
            .unwrap_or_default();
        self.msgs.update(|v| v.push((id, body)));
        let scroller = self.scroller;
        // Twice: cards that slide in settle a frame later.
        let down = move || {
            if let Some(el) = scroller.get_untracked() {
                el.set_scroll_top(el.scroll_height());
            }
        };
        request_animation_frame(down);
        set_timeout(down, Duration::from_millis(320));
        id
    }
}

/// The messages every film uses.
#[derive(Clone, Copy)]
pub enum Line {
    Me(&'static str),
    Ai(&'static str),
    /// A tool call: (plugin, call). It spins while it is the rig's `busy` one.
    Tool(&'static str, &'static str),
    /// A file you attached: (name, detail).
    File(&'static str, &'static str),
}

/// Runs a tool call: the line spins for `ms`, then checks off.
pub fn tool<T: Clone + Send + Sync + 'static>(
    s: &mut Script,
    rig: Rig,
    chat: Chat<T>,
    wrap: fn(Line) -> T,
    plugin: &'static str,
    call: &'static str,
    ms: u32,
) {
    s.then(move || {
        let id = chat.push_id(wrap(Line::Tool(plugin, call)));
        rig.busy.set(id);
    })
    .wait(ms)
    .then(move || rig.busy.set(0))
    .wait(200);
}

pub const BUBBLE_ME: &str =
    "gc-card-in gs-user max-w-[78%] self-end rounded-[18px] px-3.5 py-2 leading-snug";
pub const BUBBLE_AI: &str =
    "gc-card-in gs-bubble gs-line max-w-[80%] self-start rounded-[16px] border px-3.5 py-2.5 leading-snug";

pub fn line_view(rig: Rig, id: u32, line: Line) -> AnyView {
    match line {
        Line::Me(t) => view! { <div class=BUBBLE_ME>{t}</div> }.into_any(),
        Line::Ai(t) => view! { <div class=BUBBLE_AI>{t}</div> }.into_any(),
        Line::Tool(plugin, call) => view! {
            <div class="gc-card-in gs-f flex items-center gap-1.5 self-start text-[11.5px]">
                {move || if rig.busy.get() == id {
                    view! { <span class="h-3 w-3 animate-spin rounded-full border-[1.5px] border-current border-t-transparent"></span> }.into_any()
                } else {
                    view! { <span class="material-symbols-outlined text-[14px] text-[#14C370]">"check_circle"</span> }.into_any()
                }}
                {plugin_mark(plugin)}
                <span class="font-mono">{plugin}" · "{call}</span>
            </div>
        }
        .into_any(),
        Line::File(name, detail) => file_tile(name, detail, "gc-card-in self-end").into_any(),
    }
}

/// A spreadsheet's tile, in the thread or held by the cursor.
pub fn file_tile(name: &'static str, detail: &'static str, extra: &'static str) -> impl IntoView {
    view! {
        <div class=format!("gs-card flex items-center gap-2.5 rounded-xl px-2.5 py-2 {extra}")>
            <span class="grid h-9 w-9 place-items-center rounded-lg bg-[#1d6f42] text-[9px] font-black text-white">"XLSX"</span>
            <span class="flex flex-col leading-tight">
                <span class="text-[12.5px] font-semibold">{name}</span>
                <span class="gs-f text-[11px]">{detail}</span>
            </span>
        </div>
    }
}

/// A plugin's mark.
pub fn plugin_mark(plugin: &'static str) -> impl IntoView {
    let (letter, tone) = match plugin {
        "gmail" | "Gmail" => ("M", "bg-[#ea4335]"),
        "browser" => ("◎", "bg-[#3b82f6]"),
        "routine" => ("↻", "bg-[#8b5cf6]"),
        "plugins" => ("+", "bg-[#71717a]"),
        _ => ("B", "bg-[#0f766e]"),
    };
    view! { <span class=format!("grid h-4 w-4 shrink-0 place-items-center rounded-[4px] {tone} text-[9px] font-black text-white")>{letter}</span> }
}

/// A generated card in the thread: a header (icon, title, tag) over `body`.
pub fn card(icon: &'static str, title: &'static str, tag: &'static str, body: impl IntoView + 'static) -> impl IntoView {
    view! {
        <div class="gc-card-in gs-card w-[min(440px,100%)] self-start overflow-hidden rounded-[14px] text-[12.5px]">
            <div class="gs-line flex items-center gap-2 border-b px-3 py-2">
                <span class="material-symbols-outlined text-[16px] text-[#14C370]">{icon}</span>
                <span class="font-semibold">{title}</span>
                {(!tag.is_empty()).then(|| view! { <span class="gs-hover gs-d ml-auto rounded-full px-2 py-0.5 text-[10.5px]">{tag}</span> })}
            </div>
            <div class="px-3 py-2.5">{body}</div>
        </div>
    }
}

pub const BTN: &str = "gs-line inline-flex items-center gap-1 rounded-full border px-3 py-1 text-[12px] font-semibold";
pub const BTN_PRIMARY: &str = "gs-btn inline-flex items-center gap-1 rounded-full px-3.5 py-1 text-[12px] font-semibold";

/// Rows of label / value.
pub fn rows(items: &'static [(&'static str, &'static str)]) -> impl IntoView {
    view! {
        <div class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
            {items.iter().map(|&(k, v)| view! {
                <span class="gs-d">{k}</span><span class="text-right tabular-nums">{v}</span>
            }).collect_view()}
        </div>
    }
}

/// The login card (`request_user_form`): Email and Password, filled from a saved login.
/// `state`: 0 empty, 1 saved logins showing, 2 filled and locked, 3 logged in.
pub fn login_card(
    rig: Rig,
    plugin: &'static str,
    title: &'static str,
    site: &'static str,
    account: &'static str,
    state: RwSignal<u8>,
    refs: (NodeRef<html::Div>, NodeRef<html::Div>, NodeRef<html::Div>),
    lits: (u8, u8, u8),
) -> impl IntoView {
    let (field, saved, login) = refs;
    let field_cls = move |locked: bool| {
        if locked {
            "gs-line gs-hover flex h-8 items-center rounded-lg border px-2.5 text-[12.5px]"
        } else {
            "gs-line flex h-8 items-center rounded-lg border px-2.5 text-[12.5px]"
        }
    };
    view! {
        <div class="gc-card-in gs-card relative w-[min(380px,100%)] self-start rounded-[14px] p-3.5 text-[12.5px]">
            <div class="flex items-center gap-2 font-semibold">
                {plugin_mark(plugin)}{title}
                <span class="gs-f ml-auto text-[11px] font-normal">{site}</span>
            </div>
            {move || if state.get() == 3 {
                view! {
                    <div class="mt-2 flex items-center gap-1.5 text-[#14C370]">
                        <span class="material-symbols-outlined text-[16px]">"check_circle"</span>
                        {format!("Logged in as {account}")}
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="gs-d mb-1 mt-2.5 text-[11px]">"Email"</div>
                    <div node_ref=field class=move || {
                        let base = field_cls(state.get() >= 2);
                        if rig.lit.get() == lits.0 { format!("{base} ring-2 ring-[#14C370]/70") } else { base.to_string() }
                    }>
                        {move || if state.get() >= 2 { view! { <span>{account}</span> }.into_any() } else { view! { <span class="gs-f">"Choose a saved login"</span> }.into_any() }}
                    </div>
                    <div class="gs-d mb-1 mt-2 text-[11px]">"Password"</div>
                    <div class=move || field_cls(state.get() >= 2)>
                        {move || if state.get() >= 2 { view! { <span class="tracking-[2px]">"••••••••••••"</span> }.into_any() } else { view! { <span class="gs-f">"Filled by Touch ID"</span> }.into_any() }}
                    </div>
                    <div class="mt-3 flex items-center gap-2">
                        <span class="gs-f text-[11px]">"The password never enters the chat."</span>
                        <div node_ref=login class=rig.lit_class(lits.2, "gs-btn ml-auto rounded-full px-3.5 py-1 text-[12px] font-semibold")>"Log in"</div>
                    </div>
                    <Show when=move || state.get() == 1>
                        <div class="gc-card-in gs-card absolute left-3.5 right-3.5 top-[84px] z-10 rounded-xl p-1.5 shadow-xl">
                            <div node_ref=saved class=rig.lit_class(lits.1, "gs-hover flex items-center gap-2 rounded-lg px-2 py-1.5")>
                                <span class="material-symbols-outlined text-[16px] text-[#14C370]">"key"</span>
                                <span class="flex-1">{account}</span>
                                <span class="gs-f text-[11px]">"Passwords"</span>
                            </div>
                            <div class="gs-f px-2 pt-1 text-[10.5px]">"Choose an account. Touch ID fills in its password."</div>
                        </div>
                    </Show>
                }.into_any()
            }}
        </div>
    }
}

/// The macOS Touch ID sheet over the window.
pub fn touch_sheet(rig: Rig) -> impl IntoView {
    view! {
        <Show when=move || rig.touch.get() != Touch::Off>
            <div class="absolute inset-0 z-[35] grid place-items-center bg-black/35">
                <div class="gc-card-in w-[min(290px,86%)] rounded-2xl bg-[#f5f5f7] p-5 text-center text-[#16161c] shadow-2xl">
                    {super::shell::avatar("mx-auto mb-2 h-12 w-12")}
                    <div class="text-[13px] font-bold">"Buwiz wants to use a saved password."</div>
                    <div class="mt-1 text-[11px] text-[#5e6278]">{move || { let (a, s) = rig.touch_for.get(); format!("Touch ID to fill {a} for {s}.") }}</div>
                    <div class="mx-auto my-3 h-14 w-14">
                        <svg viewBox="0 0 64 64" fill="none" stroke-width="2.6" stroke-linecap="round" class=move || if rig.touch.get() == Touch::Ok { "h-14 w-14 stroke-[#059669] transition-colors" } else { "h-14 w-14 stroke-[#e5484d] transition-colors" }>
                            <path d="M20 26a12 12 0 0 1 24 0v6" />
                            <path d="M14 30a18 18 0 0 1 36-4" />
                            <path d="M26 28a6 6 0 0 1 12 0v8c0 6-2 10-5 14" />
                            <path d="M32 30v8c0 5-2 9-5 12" />
                            <path d="M20 34c0 7-1 11-3 14" />
                            <path d="M44 36c0 6-1 10-3 14" />
                        </svg>
                    </div>
                    <div class="text-[11px] text-[#5e6278]">{move || if rig.touch.get() == Touch::Ok { "Done" } else { "Touch ID" }}</div>
                </div>
            </div>
        </Show>
    }
}

/// An approval card: what the co-worker wants to do, then Deny and the approve button.
pub fn approve_card(
    rig: Rig,
    icon: &'static str,
    title: &'static str,
    body: impl IntoView + 'static,
    approve: &'static str,
    approve_ref: NodeRef<html::Div>,
    approve_lit: u8,
    settled: Signal<Option<&'static str>>,
) -> impl IntoView {
    view! {
        <div class="gc-card-in gs-card w-[min(440px,100%)] self-start rounded-[14px] p-3.5 text-[12.5px]">
            <div class="flex items-center gap-2 font-semibold">
                <span class="material-symbols-outlined text-[18px] text-[#f59e0b]">{icon}</span>
                {title}
            </div>
            <div class="mt-2">{body}</div>
            {move || match settled.get() {
                Some(done) => view! {
                    <div class="mt-3 flex items-center gap-1.5 font-medium text-[#14C370]">
                        <span class="material-symbols-outlined text-[16px]">"check_circle"</span>{done}
                    </div>
                }.into_any(),
                None => view! {
                    <div class="mt-3 flex justify-end gap-2">
                        <span class=BTN>"Deny"</span>
                        <div node_ref=approve_ref class=rig.lit_class(approve_lit, BTN_PRIMARY)>{approve}</div>
                    </div>
                }.into_any(),
            }}
        </div>
    }
}

/// A checklist row: on (check) or off (dash).
pub fn check_row(on: bool, label: &'static str) -> impl IntoView {
    view! {
        <div class=if on { "flex items-center gap-2 py-0.5" } else { "gs-f flex items-center gap-2 py-0.5" }>
            <span class=if on { "material-symbols-outlined text-[15px] text-[#14C370]" } else { "material-symbols-outlined gs-f text-[15px]" }>
                {if on { "check_box" } else { "check_box_outline_blank" }}
            </span>
            {label}
        </div>
    }
}
