//! Film 6, Pay: a routine that pays once BIR's confirmation is in. The co-worker opens the bank's
//! online payment site on its computer, picks BIR, fills the form, period, ATC and amount, asks
//! you to log in (saved login, Touch ID) and to approve the amount, then pays and keeps the
//! receipt. The bank site here is a mock.

use std::rc::Rc;

use leptos::{html, prelude::*};

use super::kit::{approve_card, card, line_view, login_card, rows, tool, Chat, Line, Rig, Routine, BTN, LIT_FILM};
use super::player::Script;
use super::shell::{browser, idle_screen, Desktop, ShellWindow};
use super::FilmBar;

const STEPS: &[&str] = &["1 · Pay routine", "2 · Open the bank", "3 · Fill the payment", "4 · Log in", "5 · Approve", "6 · Receipt"];

const LIT_FIELD: u8 = LIT_FILM;
const LIT_SAVED: u8 = LIT_FILM + 1;
const LIT_LOGIN: u8 = LIT_FILM + 2;
const LIT_PAY: u8 = LIT_FILM + 3;

const ACCOUNT: &str = "delacruz.bakeshop";
const HOST: &str = "linkbiz.landbank.demo";

/// Label, value, as the co-worker fills them in.
const FIELDS: &[(&str, &str)] = &[
    ("TIN", "123-456-788-00001"),
    ("RDO", "047 · East Makati"),
    ("Form", "1601-C"),
    ("ATC", "WW010"),
    ("Tax type", "WC · Withholding on compensation"),
    ("Return period", "10/31/2026"),
    ("Amount", "5,598.60"),
];

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Blank,
    Billers,
    Form,
    Login,
    Paying,
    Receipt,
}

#[derive(Clone, Copy)]
enum B {
    L(Line),
    Routine,
    Login,
    Approve,
    Receipt,
}

#[derive(Clone, Copy)]
struct F {
    r: Rig,
    chat: Chat<B>,
    note: RwSignal<&'static str>,
    page: RwSignal<Page>,
    search: RwSignal<&'static str>,
    filled: RwSignal<usize>,
    signed: RwSignal<bool>,
    login: RwSignal<u8>,
    approved: RwSignal<bool>,
    field: NodeRef<html::Div>,
    saved: NodeRef<html::Div>,
    login_btn: NodeRef<html::Div>,
    pay_btn: NodeRef<html::Div>,
}

fn script(s: &mut Script, f: F) {
    let (r, c) = (f.r, f.chat);
    s.then(move || r.act(0, "One more routine: when BIR's confirmation is in, pay the tax due online.")).wait(300);
    r.say(s, "When the confirmation arrives, pay it through LandBank", move || {
        c.push(B::L(Line::Me("When the confirmation arrives, pay it through LandBank")))
    });
    s.wait(300);
    tool(s, r, c, B::L, "routine", "create_routine on BIR confirmation", 900);
    s.then(move || {
        f.note.set("Waiting for the confirmation");
        r.add_routine(Routine { name: "Pay BIR", when: "When the confirmation arrives", note: f.note });
        c.push(B::Routine);
    })
    .wait(1400)
    .then(move || {
        f.note.set("Confirmation found · paying");
        c.push(B::L(Line::Ai("BIR's confirmation is in. Opening LandBank's online payment site.")));
        r.act(1, "The routine fires. The co-worker opens the bank's payment site on its computer.");
    })
    .wait(500);
    tool(s, r, c, B::L, "browser", "open linkbiz.landbank.demo", 600);
    s.then(move || {
        f.page.set(Page::Billers);
        r.open_screen();
    })
    .wait(900)
    .typing("BIR", 160, move |t| f.search.set(t))
    .wait(700)
    .then(move || {
        f.page.set(Page::Form);
        r.act(2, "It picks BIR, then fills the form, period, ATC and amount from the filed return.");
    })
    .wait(600);
    for n in 1..=FIELDS.len() {
        s.then(move || f.filled.set(n)).wait(480);
    }
    s.wait(700)
        .then(move || f.page.set(Page::Login))
        .wait(1100)
        .then(move || {
            r.screen.shrink();
            r.act(3, "The bank wants a login. It asks you in the chat; Touch ID fills it.");
        })
        .wait(600);
    tool(s, r, c, B::L, "browser", "request_user_form", 600);
    s.then(move || c.push(B::Login)).wait(900);
    r.tap(s, f.field, LIT_FIELD, move || f.login.set(1));
    r.tap(s, f.saved, LIT_SAVED, || {});
    r.touch_id(s, ACCOUNT, HOST);
    s.then(move || f.login.set(2)).wait(600);
    r.tap(s, f.login_btn, LIT_LOGIN, move || {
        f.login.set(3);
        f.signed.set(true);
    });
    s.wait(400)
        .then(move || r.act(4, "Before money moves, it shows you the amount and asks."))
        .then(move || c.push(B::Approve))
        .wait(1800);
    r.tap(s, f.pay_btn, LIT_PAY, move || f.approved.set(true));
    s.wait(300)
        .then(move || {
            f.page.set(Page::Paying);
            r.open_screen();
        })
        .wait(1600)
        .then(move || {
            f.page.set(Page::Receipt);
            f.note.set("Paid ₱5,598.60 · done");
            r.act(5, "Paid. The receipt goes back to the chat and onto the return in Buwiz.");
        })
        .wait(2200)
        .then(move || r.screen.shrink())
        .wait(500)
        .then(move || c.push(B::Receipt))
        .wait(300);
    r.end(s, "Filed, confirmed and paid, with your approval at every step that counts.");
}

fn message(f: F, id: u32, b: B) -> AnyView {
    let r = f.r;
    match b {
        B::L(l) => line_view(r, id, l),
        B::Routine => card(
            "schedule",
            "Routine: Pay BIR",
            "Routine",
            view! {
                <p class="gs-d">"When BIR confirms 1601-C · October 2026, pay ₱5,598.60 through LandBank. Ask me before paying."</p>
                <div class="gs-f mt-1.5 text-[11.5px]">{move || f.note.get()}</div>
            },
        )
        .into_any(),
        B::Login => login_card(
            r,
            "browser",
            "Log in to LandBank",
            HOST,
            ACCOUNT,
            f.login,
            (f.field, f.saved, f.login_btn),
            (LIT_FIELD, LIT_SAVED, LIT_LOGIN),
        )
        .into_any(),
        B::Approve => approve_card(
            r,
            "payments",
            "Pay ₱5,598.60 to BIR?",
            rows(&[
                ("Biller", "Bureau of Internal Revenue"),
                ("Form", "1601-C · October 2026"),
                ("ATC", "WW010"),
                ("From", "LandBank · Savings ••• 4821"),
            ]),
            "Pay ₱5,598.60",
            f.pay_btn,
            LIT_PAY,
            Signal::derive(move || f.approved.get().then_some("Approved by you")),
        )
        .into_any(),
        B::Receipt => card(
            "receipt_long",
            "Payment receipt",
            "PDF",
            view! {
                {rows(&[
                    ("Paid", "₱5,598.60"),
                    ("For", "1601-C · October 2026"),
                    ("Reference", "LBP-2611-0387-4412"),
                    ("Date", "Nov 3, 2026 · 10:47"),
                ])}
                <div class="mt-2.5 flex gap-2">
                    <span class=BTN>"Download PDF"</span>
                    <span class=BTN>"Print"</span>
                </div>
            },
        )
        .into_any(),
    }
}

fn site(f: F) -> impl IntoView {
    const FIELD: &str = "flex h-9 items-center rounded border border-[#cbd5e1] px-3 text-[13px]";
    view! {
        <div class="flex h-full flex-col bg-[#f1f5f2]">
            <div class="flex h-12 shrink-0 items-center gap-3 bg-[#0b6b3a] px-5 text-white">
                <span class="grid h-7 w-7 place-items-center rounded-full bg-[#f5c518] text-[12px] font-black text-[#0b6b3a]">"LB"</span>
                <span class="text-[15px] font-bold">"Link.BizPortal"</span>
                <span class="ml-auto rounded bg-white/15 px-2 py-0.5 text-[11px]">"Demo site"</span>
            </div>
            <div class="min-h-0 flex-1 overflow-hidden p-5">
                {move || match f.page.get() {
                    Page::Blank => ().into_any(),
                    Page::Billers => view! {
                        <div class="mx-auto max-w-[560px]">
                            <div class="mb-2 text-[15px] font-bold">"Choose a biller"</div>
                            <div class=format!("{FIELD} bg-white")>
                                <span class="material-symbols-outlined mr-2 text-[18px] text-[#64748b]">"search"</span>
                                {move || f.search.get()}<span class="gc-caret"></span>
                            </div>
                            <div class="mt-3 flex flex-col gap-2">
                                {move || if f.search.get().len() == 3 {
                                    view! { <div class="rounded border-2 border-[#0b6b3a] bg-white px-3 py-2.5 text-[13px] font-semibold">"Bureau of Internal Revenue (eTax)"</div> }.into_any()
                                } else {
                                    ["Bureau of Internal Revenue (eTax)", "Social Security System", "Pag-IBIG Fund", "PhilHealth"].into_iter().map(|b| view! {
                                        <div class="rounded border border-[#e2e8f0] bg-white px-3 py-2.5 text-[13px]">{b}</div>
                                    }).collect_view().into_any()
                                }}
                            </div>
                        </div>
                    }.into_any(),
                    Page::Form => view! {
                        <div class="mx-auto max-w-[620px] rounded-lg bg-white p-5 shadow-sm">
                            <div class="mb-3 text-[15px] font-bold">"Bureau of Internal Revenue · Tax payment"</div>
                            <div class="grid grid-cols-2 gap-x-4 gap-y-2.5">
                                {FIELDS.iter().enumerate().map(|(i, &(label, value))| view! {
                                    <div>
                                        <div class="mb-0.5 text-[11px] font-semibold text-[#475569]">{label}</div>
                                        <div class=move || if f.filled.get() == i + 1 { format!("{FIELD} ring-2 ring-[#0b6b3a]/40") } else { FIELD.to_string() }>
                                            {move || (f.filled.get() > i).then_some(value)}
                                        </div>
                                    </div>
                                }).collect_view()}
                            </div>
                            <div class="mt-4 flex justify-end">
                                <span class="rounded bg-[#0b6b3a] px-4 py-2 text-[13px] font-semibold text-white">"Proceed to payment"</span>
                            </div>
                        </div>
                    }.into_any(),
                    Page::Login | Page::Paying => view! {
                        <div class="mx-auto max-w-[380px] rounded-lg bg-white p-5 shadow-sm">
                            <div class="text-[15px] font-bold">"Pay with your LandBank account"</div>
                            <div class="mb-3 text-[12px] text-[#64748b]">"BIR · 1601-C · PHP 5,598.60"</div>
                            <div class="mb-0.5 text-[11px] font-semibold text-[#475569]">"User ID"</div>
                            <div class=FIELD>{move || f.signed.get().then_some(ACCOUNT)}</div>
                            <div class="mb-0.5 mt-2.5 text-[11px] font-semibold text-[#475569]">"Password"</div>
                            <div class=FIELD>{move || f.signed.get().then_some("••••••••••")}</div>
                            <div class="mt-4 rounded bg-[#0b6b3a] py-2 text-center text-[13px] font-semibold text-white">
                                {move || if f.page.get() == Page::Paying { "Paying…" } else { "Log in and pay" }}
                            </div>
                        </div>
                    }.into_any(),
                    Page::Receipt => view! {
                        <div class="mx-auto max-w-[420px] rounded-lg bg-white p-6 text-center shadow-sm">
                            <span class="material-symbols-outlined text-[44px] text-[#0b6b3a]">"check_circle"</span>
                            <div class="text-[16px] font-bold">"Payment successful"</div>
                            <div class="mt-3 grid grid-cols-2 gap-y-1 text-left text-[12.5px]">
                                <span class="text-[#64748b]">"Amount"</span><span class="text-right font-semibold">"PHP 5,598.60"</span>
                                <span class="text-[#64748b]">"Biller"</span><span class="text-right">"BIR (eTax)"</span>
                                <span class="text-[#64748b]">"Reference"</span><span class="text-right font-mono">"LBP-2611-0387-4412"</span>
                            </div>
                        </div>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}

fn desktop(f: F) -> AnyView {
    if f.page.get() == Page::Blank {
        return idle_screen().into_any();
    }
    browser("Link.BizPortal", HOST, "/bir/pay", site(f)).into_any()
}

#[component]
pub fn PayFilm() -> impl IntoView {
    let r = Rig::new();
    let f = F {
        r,
        chat: Chat::new(r),
        note: RwSignal::new(""),
        page: RwSignal::new(Page::Blank),
        search: RwSignal::new(""),
        filled: RwSignal::new(0),
        signed: RwSignal::new(false),
        login: RwSignal::new(0),
        approved: RwSignal::new(false),
        field: NodeRef::new(),
        saved: NodeRef::new(),
        login_btn: NodeRef::new(),
        pay_btn: NodeRef::new(),
    };
    let watch = RwSignal::new("10:44 · found BIR's confirmation");
    r.tl.script(move |s| {
        r.reset(
            &["Buwiz", "Gmail"],
            vec![Routine { name: "BIR confirmation", when: "Every minute · Gmail", note: watch }],
        );
        f.chat.clear();
        f.note.set("");
        f.page.set(Page::Blank);
        f.search.set("");
        f.filled.set(0);
        f.signed.set(false);
        f.login.set(0);
        f.approved.set(false);
        s.wait(300);
        script(s, f);
    });
    r.autoplay();
    let desktop: Desktop = Rc::new(move |_| view! { {move || desktop(f)} }.into_any());
    view! {
        <FilmBar rig=r steps=STEPS />
        <ShellWindow rig=r desktop chats=&["October returns", "Tax profiles"]>
            <For each=move || f.chat.list() key=|m| m.0 children=move |(id, b)| message(f, id, b) />
        </ShellWindow>
    }
}
