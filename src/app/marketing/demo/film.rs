//! The home page's OpenGrok film: a Finance coworker in OpenGrok works a BIR return through the
//! Buwiz Forms plugin (bir-mcp), one tool call at a time, in the order bir-mcp requires.
//!
//! 1. Install the plugin from the computer pane's Plugins sheet.
//! 2. The first call needs a Buwiz sign-in: OAuth on account.buwiz.com, where the user picks
//!    what the agent may do. No preset can publish to BIR.
//! 3. `bir_list_apps`; the user picks the app and confirms real data; `bir_profiles_list`.
//! 4. `bir_dues_list`.
//! 5. A payroll sheet goes in; `bir_form_open` and `bir_form_fill` fill 1601-C; the one box the
//!    sheet can't answer comes back on a `bir_form_card`; `bir_form_validate`. Then the user
//!    either views the printed sheet (`bir_form_preview`) or opens it in Buwiz
//!    (`bir_form_review`), where only their own Submit click files it.
//!
//! It plays at 2× and loops while on screen. Nothing in it takes clicks.

use std::rc::Rc;
use std::time::Duration;

use leptos::{ev, html, prelude::*};

use super::player::{
    in_view, Aim, DemoCursor, DemoPointer, PlayState, ReplayButton, Script, SpeedChips, StepChips,
    Timeline,
};
use super::window::{
    box_desktop, chrome, AgentScreen, AgentWindow, Desktop, PaneIcon, PaneIcons, FINANCE,
    FINANCE_RAIL,
};

const ACTS: &[&str] = &[
    "1 · Install the plugin",
    "2 · Sign in",
    "3 · Tax profile",
    "4 · Deadlines",
    "5 · File a return",
];

const ASK_PROFILE: &str = "What's my tax profile?";
const ASK_APP: &str = "The desktop app. Yes, it's real data.";
const ASK_DUE: &str = "What's due next?";
const ASK_FILE: &str = "Process this and fill up our October 1601-C.";
const SHEET: &str = "payroll-oct-2026.xlsx";

/// The 1601-C boxes on screen, with official labels: (label, value, source). The first three
/// come prefilled when the form opens; the last is the box the sheet can't answer.
const BOXES: &[(&str, &str, &str)] = &[
    ("Return period", "10 / 2026", "period"),
    ("TIN", "123-456-788-00000", "profile"),
    ("Withholding agent name", "DELA CRUZ BAKESHOP", "profile"),
    ("Total amount of compensation", "486,000.00", "ai"),
    ("De minimis benefits", "12,000.00", "ai"),
    ("Mandatory contributions and union dues", "31,500.00", "ai"),
    ("Total non-taxable compensation", "43,500.00", "computed"),
    ("Total taxable compensation", "442,500.00", "computed"),
    ("Total taxes withheld", "18,450.00", "ai"),
];
const PREFILLED: usize = 3;

#[derive(Clone, Copy, PartialEq)]
enum Scene {
    Desk,
    Consent,
    Connected,
    Fill,
    Preview,
    Review,
}

#[derive(Clone, Copy, PartialEq)]
enum Plugin {
    Listed,
    Installing,
    Installed,
}

/// Which control the cursor is on.
#[derive(Clone, Copy, PartialEq)]
enum Lit {
    None,
    Plug,
    Install,
    Safe,
    Allow,
    No,
    CardSend,
    View,
    Open,
    Submit,
}

#[derive(Clone)]
struct Msg {
    id: u32,
    body: Body,
}

#[derive(Clone, Copy)]
enum Body {
    Me(&'static str),
    /// The payroll sheet, with the ask.
    MeSheet,
    Ge(&'static str),
    /// A bir-mcp tool call.
    Tool(&'static str),
    Apps,
    Profile,
    Deadlines,
    /// `bir_form_card`: the box that needs you.
    Card,
    Ready,
}

#[derive(Clone, Copy)]
struct Film {
    tl: Timeline,
    cur: DemoCursor,
    act: RwSignal<Option<usize>>,
    caption: RwSignal<&'static str>,
    msgs: RwSignal<Vec<Msg>>,
    next: StoredValue<u32>,
    draft: RwSignal<&'static str>,
    lit: RwSignal<Lit>,
    sheet_open: RwSignal<bool>,
    plugin: RwSignal<Plugin>,
    dragging: RwSignal<bool>,
    scene: RwSignal<Scene>,
    /// Safe picked on the Allow screen; Read-only until then.
    safe: RwSignal<bool>,
    filled: RwSignal<usize>,
    /// The card's answer to "Special tax rate".
    relief: RwSignal<Option<bool>>,
    valid: RwSignal<bool>,
    submitted: RwSignal<bool>,
    screen: AgentScreen,
    icons: PaneIcons,
    win: NodeRef<html::Div>,
    scroller: NodeRef<html::Div>,
    composer: NodeRef<html::Div>,
    drag_from: NodeRef<html::Div>,
    install_btn: NodeRef<html::Div>,
    safe_row: NodeRef<html::Div>,
    allow_btn: NodeRef<html::Div>,
    no_btn: NodeRef<html::Div>,
    card_send: NodeRef<html::Div>,
    view_btn: NodeRef<html::Div>,
    open_btn: NodeRef<html::Div>,
    submit_btn: NodeRef<html::Div>,
}

impl Film {
    fn reset(self) {
        self.msgs.set(Vec::new());
        self.draft.set("");
        self.lit.set(Lit::None);
        self.sheet_open.set(false);
        self.plugin.set(Plugin::Listed);
        self.dragging.set(false);
        self.scene.set(Scene::Desk);
        self.safe.set(false);
        self.filled.set(PREFILLED);
        self.relief.set(None);
        self.valid.set(false);
        self.submitted.set(false);
        self.screen.reset();
        self.cur.reset();
    }

    fn act(self, n: usize, caption: &'static str) {
        self.act.set(Some(n));
        self.caption.set(caption);
    }

    fn push(self, body: Body) {
        let id = self
            .next
            .try_update_value(|n| {
                *n += 1;
                *n
            })
            .unwrap_or_default();
        self.msgs.update(|v| v.push(Msg { id, body }));
        let scroller = self.scroller;
        // Twice: cards that slide in settle a frame later.
        let down = move || {
            if let Some(el) = scroller.get_untracked() {
                el.set_scroll_top(el.scroll_height());
            }
        };
        request_animation_frame(down);
        set_timeout(down, Duration::from_millis(320));
    }

    fn lit_class(self, which: Lit, base: &'static str) -> impl Fn() -> String + Send + Sync {
        let lit = self.lit;
        move || {
            if lit.get() == which {
                format!("{base} ring-2 ring-gc-accent/70 ring-offset-1")
            } else {
                base.to_string()
            }
        }
    }
}

/// Rest on `aim` with `lit` showing, press, then `f`.
fn tap(s: &mut Script, f: Film, aim: impl Into<Aim>, lit: Lit, then: impl FnOnce() + 'static) {
    s.move_to(f.cur, aim)
        .then(move || f.lit.set(lit))
        .wait(220)
        .press(f.cur)
        .wait(170)
        .then(move || {
            f.lit.set(Lit::None);
            then();
        })
        .wait(320);
}

/// Type `text` into the composer and send it.
fn send(s: &mut Script, f: Film, text: &'static str) {
    s.move_to(f.cur, f.composer)
        .wait(150)
        .typing(text, 30, move |t| f.draft.set(t))
        .wait(300)
        .then(move || {
            f.draft.set("");
            f.push(Body::Me(text));
        });
}

fn act_install(s: &mut Script, f: Film) {
    s.then(move || f.act(0, "Plugins, in Finance's computer pane: add Buwiz Forms."))
        .wait(500);
    tap(s, f, f.icons.plug.aim(), Lit::Plug, move || f.sheet_open.set(true));
    s.wait(400);
    tap(s, f, f.install_btn, Lit::Install, move || {
        f.plugin.set(Plugin::Installing);
        f.caption.set("bir-mcp brings 16 tools: read profiles and deadlines, open, fill and check drafts. None of them files.");
    });
    s.wait(1300)
        .then(move || f.plugin.set(Plugin::Installed))
        .wait(1500)
        .then(move || f.sheet_open.set(false))
        .wait(400);
}

fn act_sign_in(s: &mut Script, f: Film) {
    s.then(move || f.act(1, "The first call needs your Buwiz sign-in."))
        .wait(300);
    send(s, f, ASK_PROFILE);
    s.wait(400)
        .then(move || f.push(Body::Ge("Buwiz needs you to sign in first. Opening account.buwiz.com.")))
        .move_home(f.cur)
        .then(move || {
            f.scene.set(Scene::Consent);
            f.screen.expand();
            f.caption.set("Buwiz's Allow screen: which profiles Finance may reach, and what it may do there.");
        })
        .wait(1200);
    tap(s, f, f.safe_row, Lit::Safe, move || {
        f.safe.set(true);
        f.caption.set("Safe adds saving and updating drafts. No preset can publish to BIR.");
    });
    s.wait(1400);
    tap(s, f, f.allow_btn, Lit::Allow, move || f.scene.set(Scene::Connected));
    s.wait(1100)
        .move_home(f.cur)
        .then(move || f.screen.shrink())
        .wait(600);
}

fn act_profile(s: &mut Script, f: Film) {
    s.then(move || f.act(2, "Finance lists the running Buwiz apps and asks which one. It never picks for you."))
        .then(move || f.push(Body::Tool("bir_list_apps")))
        .wait(400)
        .then(move || f.push(Body::Apps))
        .wait(1500);
    send(s, f, ASK_APP);
    s.wait(400)
        .then(move || f.push(Body::Tool("bir_connect · bir_profiles_list")))
        .wait(500)
        .then(move || {
            f.push(Body::Profile);
            f.caption.set("Real data only after you said so in the chat. Then the profile: name, TIN, RDO.");
        })
        .wait(2600);
}

fn act_deadlines(s: &mut Script, f: Film) {
    s.then(move || f.act(3, "Deadlines come from Buwiz's BIR filing calendar."))
        .wait(300);
    send(s, f, ASK_DUE);
    s.wait(400)
        .then(move || f.push(Body::Tool("bir_dues_list")))
        .wait(500)
        .then(move || f.push(Body::Deadlines))
        .wait(2800);
}

fn act_file(s: &mut Script, f: Film) {
    s.then(move || f.act(4, "Drop in the payroll sheet and ask Finance to fill the return."))
        .wait(400);
    s.move_to(f.cur, f.drag_from)
        .press(f.cur)
        .then(move || f.dragging.set(true))
        .wait(250)
        .move_to(f.cur, f.composer)
        .press(f.cur)
        .then(move || f.dragging.set(false))
        .wait(200)
        .typing(ASK_FILE, 28, move |t| f.draft.set(t))
        .wait(300)
        .then(move || {
            f.draft.set("");
            f.push(Body::MeSheet);
        })
        .wait(500)
        .then(move || f.push(Body::Ge("Reading payroll-oct-2026.xlsx: 14 employees, October 2026.")))
        .wait(400)
        .then(move || f.push(Body::Tool("bir_form_open 1601C · period 10")))
        .move_home(f.cur)
        .then(move || {
            f.scene.set(Scene::Fill);
            f.screen.expand();
            f.caption.set("1601-C opens in Buwiz, prefilled from the profile. Finance fills the amounts from the sheet.");
        })
        .wait(1000);
    for i in PREFILLED + 1..=BOXES.len() {
        s.then(move || f.filled.set(i)).wait(450);
    }
    s.then(move || f.caption.set("Buwiz recomputes after every fill. One box needs an answer the sheet can't give."))
        .wait(1800)
        .then(move || f.screen.shrink())
        .wait(600)
        .then(move || f.push(Body::Tool("bir_form_needs_you · bir_form_card")))
        .wait(400)
        .then(move || {
            f.push(Body::Card);
            f.caption.set("Finance asks you on a form card, right in the chat.");
        })
        .wait(900);
    tap(s, f, f.no_btn, Lit::No, move || f.relief.set(Some(false)));
    s.wait(300);
    tap(s, f, f.card_send, Lit::CardSend, move || {
        f.push(Body::Tool("bir_form_fill · bir_form_validate"));
    });
    s.wait(700)
        .then(move || {
            f.valid.set(true);
            f.push(Body::Ready);
            f.caption.set("0 errors. Finance can't submit, so it hands the return to you.");
        })
        .wait(1500);
    tap(s, f, f.view_btn, Lit::View, move || {
        f.push(Body::Tool("bir_form_preview"));
        f.scene.set(Scene::Preview);
        f.screen.expand();
        f.caption.set("View the form: BIR's printed sheet, filled, as a local file. Nothing is filed.");
    });
    s.wait(2800).then(move || f.screen.shrink()).wait(700);
    tap(s, f, f.open_btn, Lit::Open, move || {
        f.push(Body::Tool("bir_form_review"));
        f.scene.set(Scene::Review);
        f.screen.expand();
        f.caption.set("Buwiz comes forward on the form, still a Draft, with Submit highlighted for you.");
    });
    s.wait(2200);
    tap(s, f, f.submit_btn, Lit::Submit, move || {
        f.submitted.set(true);
        f.caption.set("Your click files it. Only you can send a return to BIR.");
    });
    s.wait(2000)
        .move_home(f.cur)
        .then(move || f.screen.shrink())
        .wait(600)
        .then(move || {
            f.push(Body::Ge("Submitted by you. 0619-E for October is due Nov 10 too. Want me to start it?"));
            f.caption.set("Install, sign in, profile, deadlines, a filled return. You file it.");
        })
        .wait(4200)
        .then(move || {
            if in_view(f.win) {
                f.tl.play();
            }
        });
}

const BUBBLE_ME: &str =
    "gc-card-in max-w-[85%] self-end rounded-[20px] bg-gc-text px-4 py-2.5 text-gc-bg";
const BUBBLE_GE: &str =
    "gc-card-in max-w-[85%] self-start rounded-2xl bg-gc-bg-muted px-3.5 py-2.5 text-gc-text";
const CARD: &str = "gc-card-in w-[min(380px,100%)] self-start rounded-2xl border border-gc-border bg-gc-bg-elevated p-3.5 text-[13px]";
const CHAT_BTN: &str = "inline-flex items-center gap-1.5 rounded-lg border border-gc-border bg-gc-bg px-3 py-1.5 text-[12.5px] font-semibold text-gc-text";
const CHAT_BTN_PRIMARY: &str = "inline-flex items-center gap-1.5 rounded-lg bg-gc-primary px-3 py-1.5 text-[12.5px] font-semibold text-gc-primary-fg";

fn buwiz_badge(class: &'static str) -> impl IntoView {
    view! { <span class=format!("grid place-items-center rounded-md bg-[#0f766e] font-black text-white {class}")>"B"</span> }
}

fn card_title(text: &'static str) -> impl IntoView {
    view! {
        <div class="mb-2 flex items-center gap-2 font-semibold text-gc-text">
            {buwiz_badge("h-5 w-5 text-[11px]")}
            {text}
        </div>
    }
}

fn sheet_chip() -> impl IntoView {
    view! {
        <span class="flex items-center gap-2.5 rounded-xl border border-gc-border bg-gc-bg-elevated px-3 py-2 text-gc-text">
            <span class="grid h-8 w-8 place-items-center rounded-md bg-[#1d6f42] text-[9px] font-black text-white">"XLSX"</span>
            <span class="flex flex-col leading-tight">
                <span class="text-[12.5px] font-semibold">{SHEET}</span>
                <span class="text-[11px] text-gc-text-faint">"14 rows · 18 KB"</span>
            </span>
        </span>
    }
}

fn message(f: Film, m: Msg) -> AnyView {
    match m.body {
        Body::Me(t) => view! { <div class=BUBBLE_ME>{t}</div> }.into_any(),
        Body::MeSheet => view! {
            <div class="gc-card-in flex max-w-[85%] flex-col items-end gap-1.5 self-end">
                {sheet_chip()}
                <div class="rounded-[20px] bg-gc-text px-4 py-2.5 text-gc-bg">{ASK_FILE}</div>
            </div>
        }
        .into_any(),
        Body::Ge(t) => view! { <div class=BUBBLE_GE>{t}</div> }.into_any(),
        Body::Tool(t) => view! {
            <div class="gc-card-in flex items-center gap-1.5 self-start font-mono text-[11.5px] text-gc-text-faint">
                <span class="material-symbols-outlined text-[14px]">"build"</span>
                {t}
            </div>
        }
        .into_any(),
        Body::Apps => view! {
            <div class=CARD>
                <p class="mb-2.5 text-gc-text">"One Buwiz app is running. Which one should I use? It holds real taxpayer data, so I need your OK."</p>
                <div class="flex items-center gap-2.5 rounded-lg border border-gc-border bg-gc-bg px-3 py-2">
                    <span class="material-symbols-outlined text-[18px] text-gc-text-muted">"desktop_windows"</span>
                    <span class="flex-1">
                        <span class="block font-semibold text-gc-text">"Buwiz Forms · desktop"</span>
                        <span class="block text-[11.5px] text-gc-text-muted">"Real taxpayer database · 1 taxpayer"</span>
                    </span>
                </div>
            </div>
        }
        .into_any(),
        Body::Profile => view! {
            <div class=CARD>
                {card_title("Tax profile")}
                <div class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
                    <span class="text-gc-text-muted">"Registered name"</span><span class="text-gc-text">"DELA CRUZ BAKESHOP"</span>
                    <span class="text-gc-text-muted">"TIN"</span><span class="tabular-nums text-gc-text">"123-456-788-00000"</span>
                    <span class="text-gc-text-muted">"RDO"</span><span class="text-gc-text">"047"</span>
                </div>
            </div>
        }
        .into_any(),
        Body::Deadlines => view! {
            <div class=CARD>
                {card_title("Upcoming returns")}
                <div class="flex flex-col divide-y divide-gc-border">
                    {[
                        ("1601-C", "October 2026", "Nov 10", "17 days"),
                        ("0619-E", "October 2026", "Nov 10", "17 days"),
                        ("2551Q", "Q4 2026", "Jan 25, 2027", "93 days"),
                    ]
                        .into_iter()
                        .map(|(form, period, due, left)| view! {
                            <div class="flex items-center gap-3 py-1.5">
                                <span class="w-14 font-semibold text-gc-text">{form}</span>
                                <span class="flex-1 text-gc-text-muted">{period}</span>
                                <span class="text-gc-text">{due}</span>
                                <span class="w-14 text-right text-[11.5px] text-gc-text-faint">{left}</span>
                            </div>
                        })
                        .collect_view()}
                </div>
            </div>
        }
        .into_any(),
        Body::Card => {
            let choice = move |yes: bool| {
                let base = "inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[12.5px] font-semibold";
                move || {
                    let picked = f.relief.get() == Some(yes);
                    let tone = if picked { "border-gc-accent bg-gc-accent-soft text-gc-accent" } else { "border-gc-border bg-gc-bg text-gc-text" };
                    let ring = if !yes && f.lit.get() == Lit::No { " ring-2 ring-gc-accent/70 ring-offset-1" } else { "" };
                    format!("{base} {tone}{ring}")
                }
            };
            view! {
                <div class=CARD>
                    {card_title("1601-C needs you")}
                    <p class="mb-1 text-[11.5px] text-gc-text-faint">"Special tax rate"</p>
                    <p class="mb-3 text-gc-text">"Are there payees availing of tax relief under Special Law or International Tax Treaty?"</p>
                    <div class="flex items-center gap-2">
                        <div class=choice(true)>"Yes"</div>
                        <div node_ref=f.no_btn class=choice(false)>"No"</div>
                        <div node_ref=f.card_send class=f.lit_class(Lit::CardSend, "ml-auto inline-flex items-center gap-1.5 rounded-lg bg-gc-primary px-3 py-1.5 text-[12.5px] font-semibold text-gc-primary-fg")>"Send"</div>
                    </div>
                </div>
            }
            .into_any()
        }
        Body::Ready => view! {
            <div class=CARD>
                {card_title("1601-C · October 2026 is ready")}
                <div class="grid grid-cols-[1fr_auto] gap-x-4 gap-y-1">
                    <span class="text-gc-text-muted">"Total taxable compensation"</span><span class="text-right tabular-nums text-gc-text">"₱442,500.00"</span>
                    <span class="text-gc-text-muted">"Total taxes withheld"</span><span class="text-right tabular-nums text-gc-text">"₱18,450.00"</span>
                    <span class="text-gc-text-muted">"Validate"</span><span class="text-right font-medium text-gc-accent">"✓ 0 errors"</span>
                </div>
                <p class="mt-2.5 text-gc-text">"I can't submit returns. Look it over, or open it in Buwiz and click Submit yourself."</p>
                <div class="mt-3 flex flex-wrap gap-2">
                    <div node_ref=f.view_btn class=f.lit_class(Lit::View, CHAT_BTN)>
                        <span class="material-symbols-outlined text-[16px]">"visibility"</span>"View the form"
                    </div>
                    <div node_ref=f.open_btn class=f.lit_class(Lit::Open, CHAT_BTN_PRIMARY)>
                        <span class="material-symbols-outlined text-[16px]">"send"</span>"Open in Buwiz to submit"
                    </div>
                </div>
            </div>
        }
        .into_any(),
    }
}

/// The Plugins sheet over the computer pane.
fn plugins_sheet(f: Film) -> impl IntoView {
    view! {
        <div class="flex h-12 shrink-0 items-center gap-2 border-b border-gc-border px-4 font-semibold text-gc-text">
            <span class="material-symbols-outlined text-[18px] text-gc-text-muted">"extension"</span>
            "Plugins"
        </div>
        <div class="flex flex-col gap-2.5 p-3">
            <div class="rounded-xl border border-gc-border bg-gc-bg-card p-3">
                <div class="flex items-center gap-2.5">
                    {buwiz_badge("h-9 w-9 text-[15px]")}
                    <div class="min-w-0 flex-1">
                        <div class="font-semibold text-gc-text">"Buwiz Forms"</div>
                        <div class="text-[11.5px] text-gc-text-muted">"bir-mcp · 16 tools · drafts only"</div>
                    </div>
                </div>
                <p class="mt-2 text-[12px] leading-snug text-gc-text-muted">"Read tax profiles and deadlines. Open, fill and check any of the 43 BIR forms in your Buwiz app."</p>
                <div class="mt-2.5 flex justify-end">
                    <div
                        node_ref=f.install_btn
                        class=move || {
                            let base = match f.plugin.get() {
                                Plugin::Listed => "rounded-lg bg-gc-primary px-3 py-1.5 text-[12px] font-semibold text-gc-primary-fg",
                                Plugin::Installing => "rounded-lg bg-gc-bg-muted px-3 py-1.5 text-[12px] font-semibold text-gc-text-muted",
                                Plugin::Installed => "rounded-lg bg-gc-accent-soft px-3 py-1.5 text-[12px] font-semibold text-gc-accent",
                            };
                            if f.lit.get() == Lit::Install { format!("{base} ring-2 ring-gc-accent/70 ring-offset-1") } else { base.to_string() }
                        }
                    >
                        {move || match f.plugin.get() {
                            Plugin::Listed => "Install",
                            Plugin::Installing => "Installing…",
                            Plugin::Installed => "✓ Installed",
                        }}
                    </div>
                </div>
            </div>
            {["Google Drive", "Gmail"].into_iter().map(|name| view! {
                <div class="flex items-center gap-2.5 rounded-xl border border-gc-border px-3 py-2.5 opacity-60">
                    <span class="grid h-7 w-7 place-items-center rounded-md bg-gc-bg-muted text-[11px] font-bold text-gc-text-muted">{&name[..1]}</span>
                    <span class="flex-1 text-gc-text">{name}</span>
                    <span class="text-[11.5px] text-gc-text-faint">"Installed"</span>
                </div>
            }).collect_view()}
        </div>
    }
}

/// account.buwiz.com's Allow screen, as buwiz-server renders it for an agent client.
fn consent(f: Film, safe_row: NodeRef<html::Div>, allow_btn: NodeRef<html::Div>) -> impl IntoView {
    let radio = move |on: bool| {
        if on {
            "mt-[3px] h-3 w-3 shrink-0 rounded-full border-[3.5px] border-[#2dd4bf] bg-white"
        } else {
            "mt-[3px] h-3 w-3 shrink-0 rounded-full border border-[#666]"
        }
    };
    view! {
        <div class="h-full overflow-hidden bg-[#111] px-8 py-5 text-[12px] text-[#e5e5e5]">
            <div class="mx-auto max-w-[540px]">
                {move || if f.scene.get() == Scene::Connected {
                    view! {
                        <div class="gc-card-in mt-16 text-center">
                            <span class="material-symbols-outlined text-[40px] text-[#2dd4bf]">"check_circle"</span>
                            <h3 class="mt-2 text-[17px] font-bold text-white">"Finance can work on DELA CRUZ BAKESHOP"</h3>
                            <p class="mt-1 text-[#9a9a9a]">"Back to OpenGrok. End it any time under Tax profiles → Connected agents."</p>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="mb-2 flex items-center gap-2 text-[#9a9a9a]">{buwiz_badge("h-4 w-4 text-[9px]")}"Buwiz account"</div>
                        <h3 class="text-[17px] font-bold leading-tight text-white">"Allow Finance to work on tax profiles for you?"</h3>
                        <p class="mt-1 text-[11.5px] text-[#9a9a9a]">"“Finance” named itself; Buwiz cannot confirm who made it. Allow only an app you just started yourself."</p>
                        <div class="mt-3 rounded-lg border border-[#333] px-3 py-2">
                            <div class="mb-1 text-[11px] text-[#c4c4c4]">"Profiles it may reach"</div>
                            <div class="flex items-center gap-2">
                                <span class="grid h-3 w-3 place-items-center rounded-[3px] bg-[#2dd4bf] text-[9px] font-black text-black">"✓"</span>
                                "DELA CRUZ BAKESHOP"<span class="text-[11px] text-[#9a9a9a]">"TIN ···0000"</span>
                            </div>
                        </div>
                        <div class="mt-2.5 rounded-lg border border-[#333] px-3 py-2">
                            <div class="mb-1 text-[11px] text-[#c4c4c4]">"What it may do"</div>
                            <div class="flex items-start gap-2 py-0.5">
                                <span class=move || radio(!f.safe.get())></span>
                                <span><b>"Read-only"</b>" "<span class="text-[11px] text-[#9a9a9a]">"See taxpayer details, tax years, drafts, filing history"</span></span>
                            </div>
                            <div node_ref=safe_row class=move || if f.lit.get() == Lit::Safe { "-mx-1 flex items-start gap-2 rounded px-1 py-0.5 ring-2 ring-[#2dd4bf]/60" } else { "-mx-1 flex items-start gap-2 rounded px-1 py-0.5" }>
                                <span class=move || radio(f.safe.get())></span>
                                <span><b>"Safe"</b>" "<span class="text-[11px] text-[#9a9a9a]">"Read-only, plus save a new draft, update an existing draft, mark a draft reviewed"</span></span>
                            </div>
                            <div class="flex items-start gap-2 py-0.5">
                                <span class=radio(false)></span>
                                <span><b>"Full"</b>" "<span class="text-[11px] text-[#9a9a9a]">"Safe, plus edit taxpayer details, add tax years"</span></span>
                            </div>
                            <p class="mt-1.5 text-[11px] text-[#9a9a9a]">"An agent can never publish to BIR, share profiles, archive or delete them."</p>
                        </div>
                        <div class="mt-3 flex gap-2">
                            <div node_ref=allow_btn class=f.lit_class(Lit::Allow, "rounded-md bg-white px-4 py-1.5 font-semibold text-black")>"Allow"</div>
                            <div class="rounded-md border border-[#444] px-4 py-1.5 font-semibold text-white">"Deny"</div>
                        </div>
                    }.into_any()
                }}
            </div>
        </div>
    }
}

/// The Buwiz Forms desktop app on the 1601-C page.
fn buwiz_app(f: Film, submit_btn: NodeRef<html::Div>, review: bool) -> impl IntoView {
    let source_chip = |source: &'static str| {
        let tone = match source {
            "ai" => "bg-[#ede9fe] text-[#6d28d9]",
            "computed" => "bg-[#f4f4f5] text-[#71717a]",
            _ => "bg-[#e0f2fe] text-[#0369a1]",
        };
        view! { <span class=format!("rounded px-1.5 py-px text-[10px] font-semibold {tone}")>{source}</span> }
    };
    view! {
        <div class="absolute bottom-[68px] left-1/2 top-11 flex w-[min(94%,820px)] -translate-x-1/2 overflow-hidden rounded-[10px] bg-[#fafafa] text-[12px] text-[#18181b] shadow-[0_18px_40px_rgba(0,0,0,.45)] ring-1 ring-black/20">
            <div class="flex w-[150px] shrink-0 flex-col gap-0.5 border-r border-[#e4e4e7] bg-[#f4f4f5] p-2.5">
                <div class="mb-2 flex items-center gap-1.5 px-1 font-bold">{buwiz_badge("h-5 w-5 text-[10px]")}"Buwiz Forms"</div>
                {[("home", "Home", false), ("description", "Forms", true), ("history", "History", false), ("badge", "Tax profiles", false)]
                    .into_iter()
                    .map(|(icon, label, on)| view! {
                        <span class=if on { "flex items-center gap-1.5 rounded-md bg-white px-2 py-1 font-semibold shadow-sm" } else { "flex items-center gap-1.5 rounded-md px-2 py-1 text-[#52525b]" }>
                            <span class="material-symbols-outlined text-[15px]">{icon}</span>{label}
                        </span>
                    })
                    .collect_view()}
            </div>
            <div class="flex min-w-0 flex-1 flex-col">
                <div class="flex items-center gap-2 border-b border-[#e4e4e7] px-3 py-2">
                    <div class="min-w-0 flex-1">
                        <div class="font-bold">"BIR Form 1601-C · October 2026"</div>
                        <div class="text-[10.5px] text-[#71717a]">"DELA CRUZ BAKESHOP · 123-456-788-00000"</div>
                    </div>
                    <span class=move || if f.submitted.get() { "rounded-full bg-[#dcfce7] px-2 py-0.5 text-[10.5px] font-semibold text-[#15803d]" } else { "rounded-full bg-[#fef9c3] px-2 py-0.5 text-[10.5px] font-semibold text-[#a16207]" }>
                        {move || if f.submitted.get() { "Submitted" } else { "Draft" }}
                    </span>
                    <span class="rounded-md border border-[#e4e4e7] bg-white px-2 py-1 font-semibold">"Validate"</span>
                    <span class="rounded-md border border-[#e4e4e7] bg-white px-2 py-1 font-semibold">"Print preview"</span>
                    <div
                        node_ref=submit_btn
                        class=move || {
                            let base = if review && !f.submitted.get() {
                                "rounded-md bg-[#0f766e] px-3 py-1 font-semibold text-white ring-4 ring-[#2dd4bf]/50 animate-pulse"
                            } else {
                                "rounded-md bg-[#0f766e]/50 px-3 py-1 font-semibold text-white"
                            };
                            if f.lit.get() == Lit::Submit { format!("{base} outline outline-2 outline-offset-2 outline-[#0f766e]") } else { base.to_string() }
                        }
                    >"Submit"</div>
                </div>
                {review.then(|| view! {
                    {move || if f.submitted.get() {
                        view! { <div class="gc-card-in border-b border-[#bbf7d0] bg-[#f0fdf4] px-3 py-1.5 text-[11.5px] font-medium text-[#15803d]">"✓ Submitted to BIR. The confirmation lands in History."</div> }.into_any()
                    } else {
                        view! { <div class="border-b border-[#99f6e4] bg-[#f0fdfa] px-3 py-1.5 text-[11.5px] font-medium text-[#0f766e]">"Prepared by the assistant. Review every box, then click Submit yourself."</div> }.into_any()
                    }}
                })}
                <div class="min-h-0 flex-1 overflow-hidden px-3 py-2">
                    {BOXES.iter().enumerate().map(|(i, &(label, value, source))| view! {
                        <div class=move || {
                            let filling = f.filled.get() == i + 1 && i >= PREFILLED && f.scene.get() == Scene::Fill;
                            if filling { "flex items-center gap-2 rounded-md bg-[#f5f3ff] px-2 py-[5px] ring-1 ring-[#c4b5fd]" } else { "flex items-center gap-2 border-b border-[#f4f4f5] px-2 py-[5px]" }
                        }>
                            <span class="flex-1 text-[#3f3f46]">{label}</span>
                            {move || (i < f.filled.get()).then(|| view! {
                                <span class="gc-card-in flex items-center gap-2">
                                    <span class="font-mono tabular-nums">{value}</span>
                                    {source_chip(source)}
                                </span>
                            })}
                        </div>
                    }).collect_view()}
                    <div class="flex items-center gap-2 px-2 py-[5px]">
                        <span class="flex-1 text-[#3f3f46]">"Special tax rate"</span>
                        {move || match f.relief.get() {
                            Some(_) => view! { <span class="flex items-center gap-2"><span class="font-mono">"No"</span>{source_chip("user")}</span> }.into_any(),
                            None if f.filled.get() == BOXES.len() => view! { <span class="rounded bg-[#fef3c7] px-1.5 py-px text-[10px] font-semibold text-[#b45309]">"needs you"</span> }.into_any(),
                            None => ().into_any(),
                        }}
                    </div>
                </div>
                <div class="border-t border-[#e4e4e7] px-3 py-1.5 text-[11px] text-[#71717a]">
                    {move || if f.valid.get() {
                        "✓ Validate: 0 errors"
                    } else if f.filled.get() < BOXES.len() {
                        "Filling from payroll-oct-2026.xlsx…"
                    } else {
                        "1 box needs you"
                    }}
                </div>
            </div>
        </div>
    }
}

/// `bir_form_preview`: the official sheet, filled, opened from a local file.
fn preview_page() -> impl IntoView {
    let rows: &[(&str, &str, &str)] = &[
        ("14", "Total Amount of Compensation", "486,000.00"),
        ("18", "De Minimis Benefits", "12,000.00"),
        ("19", "SSS, GSIS, PHIC, HDMF Mandatory Contributions & Union Dues", "31,500.00"),
        ("21", "Total Non-Taxable Compensation", "43,500.00"),
        ("22", "Total Taxable Compensation", "442,500.00"),
        ("25", "Total Taxes Withheld", "18,450.00"),
    ];
    chrome(
        "1601C-October-2026.html",
        "file://",
        "/Buwiz/previews/1601C-October-2026.html",
        view! {
            <div class="absolute inset-x-0 top-3 mx-auto w-[86%] bg-white px-5 py-4 font-['Arimo',Arial,sans-serif] text-[10px] text-black shadow-[0_2px_10px_rgba(0,0,0,.15)]">
                <div class="flex items-end justify-between border-b-2 border-black pb-1">
                    <span class="text-[9px]">"Republic of the Philippines · Department of Finance · Bureau of Internal Revenue"</span>
                    <span class="text-[15px] font-bold">"1601-C"</span>
                </div>
                <div class="border-b border-black py-1 text-center text-[11px] font-bold">"Monthly Remittance Return of Income Taxes Withheld on Compensation"</div>
                <div class="grid grid-cols-[auto_1fr_auto_auto] border-b border-black">
                    <span class="border-r border-black px-1">"1"</span><span class="px-1">"For the Month (MM/YYYY)"</span><span class="col-span-2 border-l border-black px-2 font-mono">"10 / 2026"</span>
                </div>
                <div class="grid grid-cols-[auto_1fr_auto] border-b border-black">
                    <span class="border-r border-black px-1">"6"</span><span class="px-1">"TIN"</span><span class="border-l border-black px-2 font-mono">"123 456 788 00000"</span>
                </div>
                <div class="grid grid-cols-[auto_1fr_auto] border-b border-black">
                    <span class="border-r border-black px-1">"8"</span><span class="px-1">"Withholding Agent's Name"</span><span class="border-l border-black px-2 font-mono">"DELA CRUZ BAKESHOP"</span>
                </div>
                <div class="mt-1 bg-[#d9d9d9] px-1 font-bold">"Part II – Computation of Tax"</div>
                {rows.iter().map(|&(n, label, value)| view! {
                    <div class="grid grid-cols-[22px_1fr_110px] border-b border-black">
                        <span class="border-r border-black px-1">{n}</span>
                        <span class="px-1">{label}</span>
                        <span class="border-l border-black px-2 text-right font-mono">{value}</span>
                    </div>
                }).collect_view()}
            </div>
        },
    )
}

fn desktop(f: Film, big: bool) -> AnyView {
    // Only the full-window copy carries the refs the cursor aims at.
    let pick = move |r: NodeRef<html::Div>| if big { r } else { NodeRef::new() };
    box_desktop(
        move || match f.scene.get() {
            Scene::Desk => "Finance's computer",
            Scene::Consent | Scene::Connected | Scene::Preview => "Chromium",
            Scene::Fill | Scene::Review => "Buwiz Forms",
        },
        move || match f.scene.get() {
            Scene::Desk => ().into_any(),
            Scene::Consent | Scene::Connected => chrome(
                "Allow Finance · Buwiz",
                "account.buwiz.com",
                "/oauth/authorize",
                consent(f, pick(f.safe_row), pick(f.allow_btn)),
            )
            .into_any(),
            Scene::Fill => buwiz_app(f, NodeRef::new(), false).into_any(),
            Scene::Preview => preview_page().into_any(),
            Scene::Review => buwiz_app(f, pick(f.submit_btn), true).into_any(),
        },
    )
    .into_any()
}

/// The OpenGrok demo: act chips, speed, Replay, a caption, and the agent window.
#[component]
pub fn OpenGrokFilm() -> impl IntoView {
    let win = NodeRef::<html::Div>::new();
    let tl = Timeline::new();
    tl.speed.set(2.);
    let lit = RwSignal::new(Lit::None);
    let icons = PaneIcons {
        monitor: PaneIcon::new(),
        plug: PaneIcon::new().lit(Signal::derive(move || lit.get() == Lit::Plug)),
        wrench: PaneIcon::new(),
    };
    let f = Film {
        tl,
        cur: DemoCursor::new(tl, win, (0.93, 0.95)),
        act: RwSignal::new(None),
        caption: RwSignal::new(""),
        msgs: RwSignal::new(Vec::new()),
        next: StoredValue::new(0),
        draft: RwSignal::new(""),
        lit,
        sheet_open: RwSignal::new(false),
        plugin: RwSignal::new(Plugin::Listed),
        dragging: RwSignal::new(false),
        scene: RwSignal::new(Scene::Desk),
        safe: RwSignal::new(false),
        filled: RwSignal::new(PREFILLED),
        relief: RwSignal::new(None),
        valid: RwSignal::new(false),
        submitted: RwSignal::new(false),
        screen: AgentScreen::new(),
        icons,
        win,
        scroller: NodeRef::new(),
        composer: NodeRef::new(),
        drag_from: NodeRef::new(),
        install_btn: NodeRef::new(),
        safe_row: NodeRef::new(),
        allow_btn: NodeRef::new(),
        no_btn: NodeRef::new(),
        card_send: NodeRef::new(),
        view_btn: NodeRef::new(),
        open_btn: NodeRef::new(),
        submit_btn: NodeRef::new(),
    };

    tl.script(move |s| {
        f.reset();
        s.wait(300);
        act_install(s, f);
        act_sign_in(s, f);
        act_profile(s, f);
        act_deadlines(s, f);
        act_file(s, f);
    });

    // Starts the first time it scrolls into view, and again after a run that ended off screen.
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

    let cur_at = f.cur.at();
    let desktop: Desktop = Rc::new(move |big| desktop(f, big));

    view! {
        <div class="mx-auto w-full max-w-[1080px]">
            <div class="mb-4 flex flex-wrap items-center gap-2">
                <StepChips steps=ACTS current=f.act />
                <div class="ml-auto flex items-center gap-2">
                    <SpeedChips timeline=tl />
                    <ReplayButton timeline=tl />
                </div>
                <p class="min-h-5 basis-full pt-1 text-left text-[13px] text-gc-text-muted" aria-live="polite">{move || f.caption.get()}</p>
            </div>

            <AgentWindow
                win
                screen=f.screen
                who=FINANCE
                rail=FINANCE_RAIL
                icons=f.icons
                scroller=f.scroller
                composer=f.composer
                draft=f.draft
                desktop
                routine=("Monthly returns", "Before each deadline")
                sheet=move || plugins_sheet(f)
                sheet_open=f.sheet_open
                overlay=move || view! {
                    <div node_ref=f.drag_from class="pointer-events-none absolute left-1 top-[42%] h-px w-px"></div>
                    <div class=move || if f.dragging.get() {
                        "pointer-events-none absolute inset-x-3 bottom-3 top-14 z-[16] rounded-2xl border-2 border-dashed border-gc-accent/60 bg-gc-accent/5 md:right-[284px] lg:right-[312px] lg:left-[76px]"
                    } else {
                        "hidden"
                    }></div>
                    {move || f.dragging.get().then(|| {
                        let (x, y) = cur_at.get().unwrap_or_default();
                        view! {
                            <div
                                class="pointer-events-none absolute left-0 top-0 z-[25] shadow-xl"
                                style=format!("transform: translate({:.1}px, {:.1}px) rotate(-4deg)", x - 40., y - 20.)
                            >
                                {sheet_chip()}
                            </div>
                        }
                    })}
                    <DemoPointer cursor=f.cur />
                }
            >
                <For each=move || f.msgs.get() key=|m| m.id children=move |m| message(f, m) />
            </AgentWindow>
        </div>
    }
}
