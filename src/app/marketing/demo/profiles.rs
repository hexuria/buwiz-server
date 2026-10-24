//! Film 2, Tax profiles: generative UI over the Buwiz plugin. A table of every profile, the one
//! you pick, its tax year and forms, and what falls due next.

use std::rc::Rc;

use leptos::{html, prelude::*};

use super::kit::{card, line_view, rows, tool, Chat, Line, Rig, BTN, LIT_FILM};
use super::player::Script;
use super::shell::{idle_screen, Desktop, ShellWindow};
use super::FilmBar;

const STEPS: &[&str] = &["1 · All profiles", "2 · Set the profile", "3 · Tax year", "4 · Forms", "5 · Due dates"];

const LIT_ROW: u8 = LIT_FILM;

const PROFILES: &[(&str, &str, &str, &str)] = &[
    ("DELA CRUZ BAKESHOP", "123-456-788-00000", "047", "Sole proprietor"),
    ("JUAN DELA CRUZ", "123-456-789-00000", "047", "Professional"),
    ("DCB FOODS CORP", "456-123-789-00000", "050", "Corporation"),
];

const FORMS: &[(&str, &str, &str)] = &[
    ("1601-C", "Monthly", "Tax withheld on compensation"),
    ("0619-E", "Monthly", "Expanded withholding remittance"),
    ("2551Q", "Quarterly", "Percentage tax"),
    ("1604-C", "Yearly", "Annual compensation withholding"),
];

const DUES: &[(&str, &str, &str, &str)] = &[
    ("1601-C", "October 2026", "Nov 10", "In 17 days"),
    ("0619-E", "October 2026", "Nov 10", "In 17 days"),
    ("2551Q", "Q4 2026", "Jan 25, 2027", "In 93 days"),
    ("1604-C", "2026", "Feb 1, 2027", "In 100 days"),
];

#[derive(Clone, Copy)]
enum B {
    L(Line),
    Table,
    Profile,
    Year,
    Forms,
    Dues,
}

#[derive(Clone, Copy)]
struct F {
    r: Rig,
    chat: Chat<B>,
    picked: RwSignal<bool>,
    forms_on: RwSignal<usize>,
    row: NodeRef<html::Div>,
}

fn script(s: &mut Script, f: F) {
    let (r, c) = (f.r, f.chat);
    s.then(move || r.act(0, "Ask in plain words. The co-worker answers with a live table, not a wall of text."))
        .wait(300);
    r.say(s, "Show my tax profiles", move || c.push(B::L(Line::Me("Show my tax profiles"))));
    s.wait(300);
    tool(s, r, c, B::L, "buwiz", "list_tax_profiles", 1000);
    s.then(move || c.push(B::Table))
        .wait(1500)
        .then(move || r.act(1, "Point at a row to work on it. Its details open as a card."));
    r.tap(s, f.row, LIT_ROW, move || f.picked.set(true));
    s.then(move || c.push(B::L(Line::Me("Use DELA CRUZ BAKESHOP"))))
        .wait(300);
    tool(s, r, c, B::L, "buwiz", "get_tax_profile", 900);
    s.then(move || c.push(B::Profile))
        .wait(1800)
        .then(move || r.act(2, "Set the tax year you are filing for."));
    r.say(s, "Set the tax year to 2026", move || c.push(B::L(Line::Me("Set the tax year to 2026"))));
    s.wait(300);
    tool(s, r, c, B::L, "buwiz", "add_tax_year 2026", 900);
    s.then(move || c.push(B::Year))
        .wait(1300)
        .then(move || r.act(3, "Choose the forms the business files. Buwiz knows each one's schedule."));
    r.say(s, "We file 1601-C, 0619-E, 2551Q and 1604-C", move || {
        c.push(B::L(Line::Me("We file 1601-C, 0619-E, 2551Q and 1604-C")))
    });
    s.wait(300);
    tool(s, r, c, B::L, "buwiz", "choose_forms", 800);
    s.then(move || c.push(B::Forms)).wait(400);
    for n in 1..=FORMS.len() {
        s.then(move || f.forms_on.set(n)).wait(350);
    }
    s.wait(900).then(move || r.act(4, "Ask what is due. Deadlines come from each form's schedule."));
    r.say(s, "What's due next?", move || c.push(B::L(Line::Me("What's due next?"))));
    s.wait(300);
    tool(s, r, c, B::L, "buwiz", "list_tax_years", 800);
    s.then(move || c.push(B::Dues))
        .wait(900)
        .then(move || c.push(B::L(Line::Ai("Two returns are due Nov 10. Send me October's payroll and I'll prepare the 1601-C."))))
        .wait(300);
    r.end(s, "Profiles, year, forms and deadlines, each as a card you can keep as a Mini App.");
}

fn message(f: F, id: u32, b: B) -> AnyView {
    let r = f.r;
    match b {
        B::L(l) => line_view(r, id, l),
        B::Table => card(
            "table_view",
            "Tax profiles",
            "Mini app",
            view! {
                <div class="grid grid-cols-[1fr_auto] gap-x-3 sm:grid-cols-[1fr_auto_auto_auto]">
                    <span class="gs-f pb-1 text-[11px]">"Name"</span>
                    <span class="gs-f pb-1 text-[11px]">"TIN"</span>
                    <span class="gs-f hidden pb-1 text-[11px] sm:block">"RDO"</span>
                    <span class="gs-f hidden pb-1 text-[11px] sm:block">"Type"</span>
                </div>
                {PROFILES.iter().enumerate().map(|(i, &(name, tin, rdo, kind))| {
                    let base = if i == 0 {
                        "gs-line -mx-1.5 grid grid-cols-[1fr_auto] gap-x-3 rounded-lg border-t px-1.5 py-1.5 sm:grid-cols-[1fr_auto_auto_auto]"
                    } else {
                        "gs-line -mx-1.5 grid grid-cols-[1fr_auto] gap-x-3 border-t px-1.5 py-1.5 sm:grid-cols-[1fr_auto_auto_auto]"
                    };
                    let cls = move || {
                        let mut c = r.lit_class(if i == 0 { LIT_ROW } else { 0 }, base)();
                        if i == 0 && f.picked.get() { c.push_str(" gs-hover"); }
                        c
                    };
                    let node = if i == 0 { f.row } else { NodeRef::<html::Div>::new() };
                    view! {
                        <div node_ref=node class=cls>
                            <span class="truncate font-medium">{name}</span>
                            <span class="font-mono text-[11.5px]">{tin}</span>
                            <span class="hidden sm:block">{rdo}</span>
                            <span class="gs-d hidden sm:block">{kind}</span>
                        </div>
                    }
                }).collect_view()}
            },
        )
        .into_any(),
        B::Profile => card(
            "storefront",
            "DELA CRUZ BAKESHOP",
            "Active profile",
            view! {
                {rows(&[
                    ("TIN", "123-456-788-00000"),
                    ("RDO", "047 · East Makati"),
                    ("Registered", "Sole proprietor"),
                    ("Address", "12 Rizal St., Makati City"),
                    ("Business", "Bakery and pastry shop"),
                    ("Employees", "6"),
                ])}
                <div class="mt-2.5 flex gap-2">
                    <span class=BTN>"View profile"</span>
                    <span class=BTN>"Edit in Buwiz"</span>
                </div>
            },
        )
        .into_any(),
        B::Year => card(
            "calendar_month",
            "Tax year 2026",
            "",
            view! { <p class="gs-d">"Added for DELA CRUZ BAKESHOP. Calendar year, January to December 2026."</p> },
        )
        .into_any(),
        B::Forms => card(
            "description",
            "Forms for 2026",
            "Mini app",
            view! {
                {FORMS.iter().enumerate().map(|(i, &(code, every, what))| view! {
                    <div class="flex items-center gap-2 py-1">
                        <span class={move || if f.forms_on.get() > i { "material-symbols-outlined text-[16px] text-[#14C370]" } else { "material-symbols-outlined gs-f text-[16px]" }}>
                            {move || if f.forms_on.get() > i { "check_box" } else { "check_box_outline_blank" }}
                        </span>
                        <span class="w-14 font-mono font-semibold">{code}</span>
                        <span class="gs-d flex-1 truncate">{what}</span>
                        <span class="gs-hover gs-d rounded-full px-2 py-0.5 text-[10.5px]">{every}</span>
                    </div>
                }).collect_view()}
            },
        )
        .into_any(),
        B::Dues => card(
            "event_upcoming",
            "Upcoming due dates",
            "Mini app",
            view! {
                {DUES.iter().enumerate().map(|(i, &(code, period, due, left))| view! {
                    <div class=if i == 0 { "flex items-center gap-2 py-1" } else { "gs-line flex items-center gap-2 border-t py-1" }>
                        <span class="w-14 font-mono font-semibold">{code}</span>
                        <span class="gs-d flex-1 truncate">{period}</span>
                        <span class="tabular-nums">{due}</span>
                        <span class=if i < 2 { "w-[78px] rounded-full bg-[#f59e0b]/15 px-2 py-0.5 text-center text-[10.5px] font-semibold text-[#d97706]" } else { "gs-hover gs-d w-[78px] rounded-full px-2 py-0.5 text-center text-[10.5px]" }>{left}</span>
                    </div>
                }).collect_view()}
            },
        )
        .into_any(),
    }
}

#[component]
pub fn ProfilesFilm() -> impl IntoView {
    let r = Rig::new();
    let f = F { r, chat: Chat::new(r), picked: RwSignal::new(false), forms_on: RwSignal::new(0), row: NodeRef::new() };
    r.tl.script(move |s| {
        r.reset(&["Buwiz"], Vec::new());
        f.chat.clear();
        f.picked.set(false);
        f.forms_on.set(0);
        s.wait(300);
        script(s, f);
    });
    r.autoplay();
    let desktop: Desktop = Rc::new(|_| idle_screen().into_any());
    view! {
        <FilmBar rig=r steps=STEPS />
        <ShellWindow rig=r desktop chats=&["Tax profiles", "Buwiz setup"]>
            <For each=move || f.chat.list() key=|m| m.0 children=move |(id, b)| message(f, id, b) />
        </ShellWindow>
    }
}
