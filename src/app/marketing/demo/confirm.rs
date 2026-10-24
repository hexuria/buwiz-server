//! Film 5, Confirmation email: install Gmail, set a routine that checks the inbox every minute,
//! and when BIR's confirmation arrives the co-worker brings it back as a PDF to print or keep.

use std::rc::Rc;

use leptos::{html, prelude::*};

use super::kit::{card, line_view, tool, Chat, Line, Rig, Routine, BTN, BTN_PRIMARY, LIT_FILM};
use super::player::Script;
use super::shell::{doc_frame, idle_screen, viewer, Desktop, ShellWindow};
use super::FilmBar;

const STEPS: &[&str] = &["1 · Install Gmail", "2 · Set a routine", "3 · Every minute", "4 · Email arrives", "5 · Confirmation PDF"];

const LIT_OPEN: u8 = LIT_FILM;
const LIT_DOWNLOAD: u8 = LIT_FILM + 1;

const PDF: &str = "BIR confirmation · 1601-C October 2026.pdf";

#[derive(Clone, Copy)]
enum B {
    L(Line),
    Installed,
    Routine,
    Email,
}

#[derive(Clone, Copy)]
struct F {
    r: Rig,
    chat: Chat<B>,
    note: RwSignal<&'static str>,
    open: RwSignal<bool>,
    saved: RwSignal<bool>,
    open_btn: NodeRef<html::Div>,
    download_btn: NodeRef<html::Div>,
}

fn script(s: &mut Script, f: F) {
    let (r, c) = (f.r, f.chat);
    s.then(move || r.act(0, "The co-worker needs your inbox for this. Ask it to install Gmail.")).wait(300);
    r.say(s, "Install Gmail", move || c.push(B::L(Line::Me("Install Gmail"))));
    s.wait(300);
    tool(s, r, c, B::L, "plugins", "install_plugin gmail", 1300);
    s.then(move || {
        r.connect(&["Buwiz", "Gmail"]);
        c.push(B::Installed);
    })
    .wait(1300)
    .then(move || r.act(1, "Now ask for a routine: it runs on a schedule, even when you are not chatting."));
    r.say(s, "Check my email every minute for BIR's confirmation", move || {
        c.push(B::L(Line::Me("Check my email every minute for BIR's confirmation")))
    });
    s.wait(300);
    tool(s, r, c, B::L, "routine", "create_routine every 1m", 900);
    s.then(move || {
        f.note.set("Starting…");
        r.add_routine(Routine { name: "BIR confirmation", when: "Every minute · Gmail", note: f.note });
        c.push(B::Routine);
    })
    .wait(1200)
    .then(move || r.act(2, "Each run searches the inbox. Nothing yet, so it stays quiet."));
    for note in ["10:41 · nothing yet", "10:42 · nothing yet", "10:43 · nothing yet"] {
        s.then(move || f.note.set("Checking Gmail…"))
            .wait(700)
            .then(move || f.note.set(note))
            .wait(900);
    }
    s.then(move || f.note.set("Checking Gmail…"))
        .wait(700)
        .then(move || {
            f.note.set("10:44 · found BIR's confirmation");
            r.act(3, "BIR's email lands. The routine picks it up and tells you in the chat.");
        });
    tool(s, r, c, B::L, "gmail", "search “Tax Return Receipt Confirmation”", 800);
    s.then(move || c.push(B::Email))
        .wait(1600)
        .then(move || r.act(4, "The confirmation opens as a PDF, ready to print or save."));
    r.tap(s, f.open_btn, LIT_OPEN, move || {
        f.open.set(true);
        r.open_screen();
    });
    s.wait(1800);
    r.tap(s, f.download_btn, LIT_DOWNLOAD, move || f.saved.set(true));
    s.wait(1400)
        .then(move || r.screen.shrink())
        .wait(600)
        .then(move || c.push(B::L(Line::Ai("Saved to Downloads and to the return in Buwiz. Want me to pay the ₱5,598.60 next?"))))
        .wait(300);
    r.end(s, "A routine watched the inbox and brought back BIR's confirmation as a PDF.");
}

fn message(f: F, id: u32, b: B) -> AnyView {
    match b {
        B::L(l) => line_view(f.r, id, l),
        B::Installed => card(
            "extension",
            "Gmail installed",
            "Plugin",
            view! { <p class="gs-d">"Reads and searches the inbox you connected. It cannot send mail unless you allow it."</p> },
        )
        .into_any(),
        B::Routine => card(
            "schedule",
            "Routine: BIR confirmation",
            "Routine",
            view! {
                <p class="gs-d">"Every minute, search Gmail for BIR's confirmation of 1601-C · October 2026. Stop once found."</p>
                <div class="gs-f mt-1.5 text-[11.5px]">{move || f.note.get()}</div>
            },
        )
        .into_any(),
        B::Email => card(
            "mail",
            "Tax Return Receipt Confirmation",
            "Gmail",
            view! {
                <div class="gs-d text-[11.5px]">"ebirforms-noreply@bir.gov.ph · 10:44"</div>
                <p class="mt-1">"This confirms receipt of your submission with the following details subject to validation by BIR:"</p>
                <div class="gs-d mt-1 font-mono text-[11px] leading-relaxed">
                    "File name: 123456788001-1601Cv2018-102026.xml"<br />
                    "Date received by BIR: 3 November 2026"<br />
                    "Time received by BIR: 10:43 AM"
                </div>
                <div class="mt-2.5 flex gap-2">
                    <div node_ref=f.open_btn class=f.r.lit_class(LIT_OPEN, BTN_PRIMARY)>
                        <span class="material-symbols-outlined text-[14px]">"picture_as_pdf"</span>"Open PDF"
                    </div>
                    <span class=BTN>"Open email"</span>
                </div>
            },
        )
        .into_any(),
    }
}

/// The return as Buwiz prints it with BIR's confirmation email as its last page.
fn letter() -> impl IntoView {
    doc_frame("/assets/demo/1601c-october-2026-confirmation.html#page-receipt")
}

fn desktop(f: F) -> AnyView {
    if !f.open.get() {
        return idle_screen().into_any();
    }
    viewer(
        "pdf",
        PDF,
        view! {
            <span class="rounded-full border border-white/20 px-2.5 py-0.5 text-[11px]">"Print"</span>
            <div node_ref=f.download_btn class=f.r.lit_class(LIT_DOWNLOAD, "rounded-full border border-white/20 px-2.5 py-0.5 text-[11px]")>
                {move || if f.saved.get() { "Saved" } else { "Download" }}
            </div>
        },
        letter(),
    )
    .into_any()
}

#[component]
pub fn ConfirmFilm() -> impl IntoView {
    let r = Rig::new();
    let f = F {
        r,
        chat: Chat::new(r),
        note: RwSignal::new(""),
        open: RwSignal::new(false),
        saved: RwSignal::new(false),
        open_btn: NodeRef::new(),
        download_btn: NodeRef::new(),
    };
    r.tl.script(move |s| {
        r.reset(&["Buwiz"], Vec::new());
        f.chat.clear();
        f.note.set("");
        f.open.set(false);
        f.saved.set(false);
        s.wait(300);
        script(s, f);
    });
    r.autoplay();
    let desktop: Desktop = Rc::new(move |big| {
        if big {
            view! { {move || desktop(f)} }.into_any()
        } else {
            view! { {move || if f.open.get() { letter().into_any() } else { idle_screen().into_any() }} }.into_any()
        }
    });
    view! {
        <FilmBar rig=r steps=STEPS />
        <ShellWindow rig=r desktop chats=&["October returns", "Tax profiles"]>
            <For each=move || f.chat.list() key=|m| m.0 children=move |(id, b)| message(f, id, b) />
        </ShellWindow>
    }
}
