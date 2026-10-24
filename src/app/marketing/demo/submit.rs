//! Film 4, Submit: say "submit", the co-worker validates the draft again, finds it may not
//! publish to BIR and asks for that permission, then asks you to approve the submission itself.
//! Approved, the return joins the filing queue.

use std::rc::Rc;

use leptos::{html, prelude::*};

use super::kit::{approve_card, card, check_row, line_view, rows, tool, Chat, Line, Rig, LIT_FILM};
use super::player::Script;
use super::shell::{idle_screen, Desktop, ShellWindow};
use super::FilmBar;

const STEPS: &[&str] = &["1 · Ask to submit", "2 · Validate again", "3 · Permission", "4 · Approve", "5 · Queued"];

const LIT_ALLOW: u8 = LIT_FILM;
const LIT_SUBMIT: u8 = LIT_FILM + 1;

#[derive(Clone, Copy)]
enum B {
    L(Line),
    Permission,
    Confirm,
    Queued,
}

#[derive(Clone, Copy)]
struct F {
    r: Rig,
    chat: Chat<B>,
    allowed: RwSignal<bool>,
    submitted: RwSignal<bool>,
    /// 0 queued, 1 sending, 2 sent.
    stage: RwSignal<u8>,
    allow_btn: NodeRef<html::Div>,
    submit_btn: NodeRef<html::Div>,
}

fn script(s: &mut Script, f: F) {
    let (r, c) = (f.r, f.chat);
    s.then(move || r.act(0, "When the reviewed form looks right, just say “submit”.")).wait(300);
    r.say(s, "Submit the Makati branch's.", move || c.push(B::L(Line::Me("Submit the Makati branch's."))));
    s.wait(300)
        .then(move || r.act(1, "It checks the form against every BIR rule again before anything leaves."));
    tool(s, r, c, B::L, "buwiz", "validate_form 1601-C · 0 errors", 1100);
    tool(s, r, c, B::L, "buwiz", "check_permission publish", 800);
    s.then(move || r.act(2, "Publishing was left off, so it asks. You can allow it for this return only."))
        .then(move || c.push(B::L(Line::Ai("Publishing to BIR is off for me. Allow it for this return?"))))
        .wait(500)
        .then(move || c.push(B::Permission))
        .wait(1600);
    r.tap(s, f.allow_btn, LIT_ALLOW, move || f.allowed.set(true));
    s.wait(400)
        .then(move || r.act(3, "One last look: what is filed, for which period, and the tax due. Nothing goes until you approve."))
        .then(move || c.push(B::Confirm))
        .wait(2000);
    r.tap(s, f.submit_btn, LIT_SUBMIT, move || f.submitted.set(true));
    s.wait(300).then(move || r.act(4, "Approved. The return joins the filing queue and Buwiz sends it to BIR."));
    tool(s, r, c, B::L, "buwiz", "submit_filing", 900);
    s.then(move || c.push(B::Queued))
        .wait(1300)
        .then(move || f.stage.set(1))
        .wait(1800)
        .then(move || f.stage.set(2))
        .wait(600)
        .then(move || c.push(B::L(Line::Ai("Sent. BIR emails a confirmation when it is received; want me to watch for it?"))))
        .wait(300);
    r.end(s, "Validated twice, permission asked, your approval given, then queued.");
}

fn message(f: F, id: u32, b: B) -> AnyView {
    let r = f.r;
    match b {
        B::L(l) => line_view(r, id, l),
        B::Permission => approve_card(
            r,
            "shield_person",
            "Allow Buwiz to publish to BIR?",
            view! {
                {check_row(true, "Publish to BIR")}
                <div class="gs-d mt-1 text-[11.5px]">"Only for 1601-C · October 2026, branch 00001. Each submission still asks you."</div>
            },
            "Allow once",
            f.allow_btn,
            LIT_ALLOW,
            Signal::derive(move || f.allowed.get().then_some("Allowed for this return")),
        )
        .into_any(),
        B::Confirm => approve_card(
            r,
            "send",
            "Submit this return to BIR?",
            rows(&[
                ("Form", "1601-C"),
                ("Period", "October 2026"),
                ("Taxpayer", "JUAN DELA CRUZ"),
                ("Branch", "00001 · Dela Cruz Bakeshop, Makati"),
                ("TIN", "123-456-788-00001"),
                ("Tax due", "₱5,598.60"),
            ]),
            "Submit",
            f.submit_btn,
            LIT_SUBMIT,
            Signal::derive(move || f.submitted.get().then_some("Approved by you")),
        )
        .into_any(),
        B::Queued => card(
            "schedule_send",
            "Filing queue",
            "Mini app",
            view! {
                {[("Queued", 0u8), ("Sending to BIR", 1), ("Sent, awaiting confirmation", 2)].into_iter().map(|(label, at)| view! {
                    <div class="flex items-center gap-2 py-0.5">
                        {move || {
                            let st = f.stage.get();
                            if st > at || (st == 2 && at == 2) {
                                view! { <span class="material-symbols-outlined text-[16px] text-[#14C370]">"check_circle"</span> }.into_any()
                            } else if st == at {
                                view! { <span class="mx-[2px] h-3 w-3 animate-spin rounded-full border-[1.5px] border-[#14C370] border-t-transparent"></span> }.into_any()
                            } else {
                                view! { <span class="material-symbols-outlined gs-f text-[16px]">"radio_button_unchecked"</span> }.into_any()
                            }
                        }}
                        <span class={move || if f.stage.get() >= at { "" } else { "gs-f" }}>{label}</span>
                    </div>
                }).collect_view()}
                <div class="gs-d mt-1.5 text-[11.5px]">"1601-C · October 2026 · ₱5,598.60"</div>
            },
        )
        .into_any(),
    }
}

#[component]
pub fn SubmitFilm() -> impl IntoView {
    let r = Rig::new();
    let f = F {
        r,
        chat: Chat::new(r),
        allowed: RwSignal::new(false),
        submitted: RwSignal::new(false),
        stage: RwSignal::new(0),
        allow_btn: NodeRef::new(),
        submit_btn: NodeRef::new(),
    };
    r.tl.script(move |s| {
        r.reset(&["Buwiz"], Vec::new());
        f.chat.clear();
        f.allowed.set(false);
        f.submitted.set(false);
        f.stage.set(0);
        s.wait(300);
        script(s, f);
    });
    r.autoplay();
    let desktop: Desktop = Rc::new(|_| idle_screen().into_any());
    view! {
        <FilmBar rig=r steps=STEPS />
        <ShellWindow rig=r desktop chats=&["October returns", "Tax profiles"]>
            <For each=move || f.chat.list() key=|m| m.0 children=move |(id, b)| message(f, id, b) />
        </ShellWindow>
    }
}
