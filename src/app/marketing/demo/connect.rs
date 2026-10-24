//! Film 1, Connect: install the Buwiz plugin from the chat, "Log in with Buwiz", a login card
//! filled from Passwords with Touch ID, then the permission prompt (buwiz-server's own
//! permission labels; Publish to BIR is left off).

use std::rc::Rc;

use leptos::{html, prelude::*};

use super::kit::{
    approve_card, card, check_row, line_view, login_card, tool, Chat, Line, Rig, LIT_FILM,
};
use super::player::Script;
use super::shell::{idle_screen, Desktop, ShellWindow};
use super::FilmBar;

const STEPS: &[&str] = &["1 · Install the plugin", "2 · Log in with Buwiz", "3 · Touch ID", "4 · Allow access"];
const ACCOUNT: &str = "juan@delacruz.ph";

const LIT_FIELD: u8 = LIT_FILM;
const LIT_SAVED: u8 = LIT_FILM + 1;
const LIT_LOGIN: u8 = LIT_FILM + 2;
const LIT_ALLOW: u8 = LIT_FILM + 3;

#[derive(Clone, Copy)]
enum B {
    L(Line),
    Installed,
    Login,
    Allow,
}

#[derive(Clone, Copy)]
struct F {
    r: Rig,
    chat: Chat<B>,
    login: RwSignal<u8>,
    allowed: RwSignal<bool>,
    field: NodeRef<html::Div>,
    saved: NodeRef<html::Div>,
    login_btn: NodeRef<html::Div>,
    allow_btn: NodeRef<html::Div>,
}

fn script(s: &mut Script, f: F) {
    let (r, c) = (f.r, f.chat);
    s.then(move || r.act(0, "Ask for the plugin in plain words. The co-worker installs it with a tool call."))
        .wait(400);
    r.say(s, "Install the Buwiz plugin", move || {
        c.push(B::L(Line::Me("Install the Buwiz plugin")));
    });
    s.wait(300);
    tool(s, r, c, B::L, "plugins", "install_plugin buwiz", 1400);
    s.then(move || {
        r.connect(&["Buwiz"]);
        c.push(B::Installed);
    })
    .wait(1600)
    .then(move || r.act(1, "Say “Log in with Buwiz”. The co-worker asks for your login on a form card."))
    .wait(300);
    r.say(s, "Log in with Buwiz", move || {
        c.push(B::L(Line::Me("Log in with Buwiz")));
    });
    s.wait(400);
    tool(s, r, c, B::L, "buwiz", "request_user_form", 700);
    s.then(move || c.push(B::Login)).wait(900);
    r.tap(s, f.field, LIT_FIELD, move || f.login.set(1));
    s.wait(400)
        .then(move || r.act(2, "Pick the saved login. Touch ID fills the password, so it never lands in the chat."));
    r.tap(s, f.saved, LIT_SAVED, || {});
    r.touch_id(s, ACCOUNT, "account.buwiz.com");
    s.then(move || f.login.set(2)).wait(700);
    r.tap(s, f.login_btn, LIT_LOGIN, move || f.login.set(3));
    s.wait(600)
        .then(move || r.act(3, "Buwiz asks what the co-worker may do. You choose; publishing to BIR stays off."))
        .wait(300)
        .then(move || {
            c.push(B::Allow);
        })
        .wait(1800);
    r.tap(s, f.allow_btn, LIT_ALLOW, move || f.allowed.set(true));
    s.wait(500)
        .then(move || {
            c.push(B::L(Line::Ai("Connected to Buwiz. I can see 3 tax profiles on your account. What should we work on?")));
        })
        .wait(400);
    r.end(s, "Installed, logged in with Touch ID, and allowed. Change or end it any time in Buwiz.");
}

fn message(f: F, id: u32, b: B) -> AnyView {
    let r = f.r;
    match b {
        B::L(l) => line_view(r, id, l),
        B::Installed => card(
            "extension",
            "Buwiz installed",
            "Plugin",
            view! {
                <p class="gs-d">"Tax profiles, deadlines, BIR forms and filings, as tools your co-worker can use."</p>
            },
        )
        .into_any(),
        B::Login => login_card(
            r,
            "buwiz",
            "Log in to Buwiz",
            "account.buwiz.com",
            ACCOUNT,
            f.login,
            (f.field, f.saved, f.login_btn),
            (LIT_FIELD, LIT_SAVED, LIT_LOGIN),
        )
        .into_any(),
        B::Allow => approve_card(
            r,
            "shield_person",
            "Allow the Buwiz agent on your account?",
            view! {
                <div class="gs-d mb-1 text-[11px]">"Profiles"</div>
                <div class="mb-2">"JUAN DELA CRUZ · 123-456-788 · all 4 branches"</div>
                <div class="gs-d mb-1 text-[11px]">"What it may do"</div>
                <div class="grid grid-cols-1 sm:grid-cols-2 sm:gap-x-3">
                    {check_row(true, "See taxpayer details")}
                    {check_row(true, "See tax years and their forms")}
                    {check_row(true, "Add tax years, choose forms")}
                    {check_row(true, "See drafts")}
                    {check_row(true, "Save a new draft")}
                    {check_row(true, "Update an existing draft")}
                    {check_row(true, "Mark a draft reviewed")}
                    {check_row(true, "See filing history and BIR confirmations")}
                    {check_row(false, "Edit taxpayer details")}
                    {check_row(false, "Publish to BIR")}
                </div>
            },
            "Allow",
            f.allow_btn,
            LIT_ALLOW,
            Signal::derive(move || f.allowed.get().then_some("Allowed. Publishing to BIR still asks you first.")),
        )
        .into_any(),
    }
}

#[component]
pub fn ConnectFilm() -> impl IntoView {
    let r = Rig::new();
    let f = F {
        r,
        chat: Chat::new(r),
        login: RwSignal::new(0),
        allowed: RwSignal::new(false),
        field: NodeRef::new(),
        saved: NodeRef::new(),
        login_btn: NodeRef::new(),
        allow_btn: NodeRef::new(),
    };
    r.tl.script(move |s| {
        r.reset(&[], Vec::new());
        f.chat.clear();
        f.login.set(0);
        f.allowed.set(false);
        s.wait(300);
        script(s, f);
    });
    r.autoplay();
    let desktop: Desktop = Rc::new(|_| idle_screen().into_any());
    view! {
        <FilmBar rig=r steps=STEPS />
        <ShellWindow rig=r desktop chats=&["Buwiz setup", "Weekly bookkeeping"]>
            <For each=move || f.chat.list() key=|m| m.0 children=move |(id, b)| message(f, id, b) />
        </ShellWindow>
    }
}
