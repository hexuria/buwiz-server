//! Film 2, Tax profiles: generative UI over the Buwiz plugin. One mixed-income earner's profile
//! with every branch under the TIN, the branch you pick, the tax year and forms, and what falls
//! due next.

use std::rc::Rc;

use leptos::{html, prelude::*};

use super::kit::{card, line_view, rows, tool, Chat, Line, Rig, BTN, LIT_FILM};
use super::player::Script;
use super::shell::{idle_screen, Desktop, ShellWindow};
use super::FilmBar;

const STEPS: &[&str] = &["1 · Your profile", "2 · Pick a branch", "3 · Tax year", "4 · Forms", "5 · Due dates"];

const LIT_ROW: u8 = LIT_FILM;

/// The branch the film picks: its row in `BRANCHES`.
const PICK: usize = 1;

/// Code, trade name, where (RDO), what it earns from.
const BRANCHES: &[(&str, &str, &str, &str)] = &[
    ("00000", "Head office", "Makati · 047", "Salary + freelance"),
    ("00001", "Dela Cruz Bakeshop", "Makati · 047", "Bakery, 6 staff"),
    ("00002", "Dela Cruz Bakeshop", "Quezon City · 038", "Bakery, 3 staff"),
    ("00003", "JDC Apartments", "Pasig · 043", "Rentals, 1 staff"),
];

/// Form, how often, what it is, which branches file it.
const FORMS: &[(&str, &str, &str, &str)] = &[
    ("1701Q", "Quarterly", "Income tax, all income", "00000"),
    ("2551Q", "Quarterly", "Percentage tax", "All"),
    ("1601-C", "Monthly", "Tax withheld on salaries", "00001–03"),
    ("1604-C", "Yearly", "Annual withholding report", "00001–03"),
];

/// Form, period, branches, due, left.
const DUES: &[(&str, &str, &str, &str, &str)] = &[
    ("1601-C", "Oct 2026", "×3", "Nov 10", "In 17 days"),
    ("1701Q", "Q3 2026", "", "Nov 16", "In 23 days"),
    ("2551Q", "Q4 2026", "×4", "Jan 25, 2027", "In 93 days"),
    ("1604-C", "2026", "×3", "Feb 1, 2027", "In 100 days"),
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
    s.then(move || r.act(0, "Ask in plain words. Your TIN and every branch under it come back as a live table."))
        .wait(300);
    r.say(s, "Show my tax profile", move || c.push(B::L(Line::Me("Show my tax profile"))));
    s.wait(300);
    tool(s, r, c, B::L, "buwiz", "list_tax_profiles", 1000);
    s.then(move || c.push(B::Table))
        .wait(1700)
        .then(move || r.act(1, "One TIN, four branches: a job, two bakeshops and an apartment. Point at one to work on it."));
    r.tap(s, f.row, LIT_ROW, move || f.picked.set(true));
    s.then(move || c.push(B::L(Line::Me("Open the Makati bakeshop"))))
        .wait(300);
    tool(s, r, c, B::L, "buwiz", "get_branch 00001", 900);
    s.then(move || c.push(B::Profile))
        .wait(1800)
        .then(move || r.act(2, "Set the tax year once. It covers every branch."));
    r.say(s, "Set the tax year to 2026", move || c.push(B::L(Line::Me("Set the tax year to 2026"))));
    s.wait(300);
    tool(s, r, c, B::L, "buwiz", "add_tax_year 2026", 900);
    s.then(move || c.push(B::Year))
        .wait(1300)
        .then(move || r.act(3, "Say what you file. Buwiz knows which branch files each form, and when."));
    r.say(s, "We file 1701Q, 2551Q, 1601-C and 1604-C", move || {
        c.push(B::L(Line::Me("We file 1701Q, 2551Q, 1601-C and 1604-C")))
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
        .then(move || c.push(B::L(Line::Ai("Three 1601-Cs are due Nov 10, one for each branch with staff. Send me October's payroll and I'll prepare them."))))
        .wait(300);
    r.end(s, "One TIN, every branch: year, forms and deadlines, each as a card you can keep as a Mini App.");
}

fn message(f: F, id: u32, b: B) -> AnyView {
    let r = f.r;
    match b {
        B::L(l) => line_view(r, id, l),
        B::Table => card(
            "account_tree",
            "JUAN DELA CRUZ",
            "Mini app",
            view! {
                <div class="gs-d -mt-0.5 mb-2 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[11.5px]">
                    <span class="font-mono">"TIN 123-456-788"</span>
                    <span>"·"</span><span>"Mixed income earner"</span>
                    <span>"·"</span><span>"4 branches"</span>
                </div>
                <div class="grid grid-cols-[auto_1fr_auto] gap-x-3 sm:grid-cols-[auto_1fr_auto_auto]">
                    <span class="gs-f pb-1 text-[11px]">"Branch"</span>
                    <span class="gs-f pb-1 text-[11px]">"Name"</span>
                    <span class="gs-f pb-1 text-[11px]">"RDO"</span>
                    <span class="gs-f hidden pb-1 text-[11px] sm:block">"Income"</span>
                </div>
                {BRANCHES.iter().enumerate().map(|(i, &(code, name, place, kind))| {
                    let base = if i == PICK {
                        "gs-line -mx-1.5 grid grid-cols-[auto_1fr_auto] gap-x-3 rounded-lg border-t px-1.5 py-1.5 sm:grid-cols-[auto_1fr_auto_auto]"
                    } else {
                        "gs-line -mx-1.5 grid grid-cols-[auto_1fr_auto] gap-x-3 border-t px-1.5 py-1.5 sm:grid-cols-[auto_1fr_auto_auto]"
                    };
                    let cls = move || {
                        let mut c = r.lit_class(if i == PICK { LIT_ROW } else { 0 }, base)();
                        if i == PICK && f.picked.get() { c.push_str(" gs-hover"); }
                        c
                    };
                    let node = if i == PICK { f.row } else { NodeRef::<html::Div>::new() };
                    view! {
                        <div node_ref=node class=cls>
                            <span class="font-mono text-[11.5px]">{code}</span>
                            <span class="truncate font-medium">{name}</span>
                            <span class="gs-d text-[11.5px]">{place}</span>
                            <span class="gs-d hidden text-[11.5px] sm:block">{kind}</span>
                        </div>
                    }
                }).collect_view()}
            },
        )
        .into_any(),
        B::Profile => card(
            "storefront",
            "Dela Cruz Bakeshop · 00001",
            "Branch",
            view! {
                {rows(&[
                    ("TIN", "123-456-788-00001"),
                    ("Registered to", "JUAN DELA CRUZ"),
                    ("RDO", "047 · East Makati"),
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
            view! { <p class="gs-d">"Added for JUAN DELA CRUZ and all 4 branches. Calendar year, January to December 2026."</p> },
        )
        .into_any(),
        B::Forms => card(
            "description",
            "Forms for 2026",
            "Mini app",
            view! {
                {FORMS.iter().enumerate().map(|(i, &(code, every, what, branches))| view! {
                    <div class="flex items-center gap-2 py-1">
                        <span class={move || if f.forms_on.get() > i { "material-symbols-outlined text-[16px] text-[#14C370]" } else { "material-symbols-outlined gs-f text-[16px]" }}>
                            {move || if f.forms_on.get() > i { "check_box" } else { "check_box_outline_blank" }}
                        </span>
                        <span class="w-14 font-mono font-semibold">{code}</span>
                        <span class="gs-d min-w-0 flex-1 truncate">{what}</span>
                        <span class="gs-f hidden font-mono text-[11px] sm:inline">{branches}</span>
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
                {DUES.iter().enumerate().map(|(i, &(code, period, many, due, left))| view! {
                    <div class=if i == 0 { "flex items-center gap-2 py-1" } else { "gs-line flex items-center gap-2 border-t py-1" }>
                        <span class="w-14 font-mono font-semibold">{code}</span>
                        <span class="gs-d min-w-0 flex-1 truncate">{period}<span class="gs-f ml-1.5 text-[11px]">{many}</span></span>
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
        <ShellWindow rig=r desktop chats=&["My branches", "Buwiz setup"]>
            <For each=move || f.chat.list() key=|m| m.0 children=move |(id, b)| message(f, id, b) />
        </ShellWindow>
    }
}
