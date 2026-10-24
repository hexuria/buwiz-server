//! Film 3, Prepare returns: attach the accountant's payroll workbook for three branches under
//! one TIN, the co-worker opens it, fills one October 1601-C per branch, checks them against
//! BIR's rules, shows branch 00001's form as a PDF, and marks them reviewed when you say so.

use std::rc::Rc;

use leptos::{html, prelude::*};

use super::kit::{card, line_view, rows, tool, Chat, Line, Rig, BTN, BTN_PRIMARY, LIT_FILM, LIT_PLUS};
use super::player::Script;
use super::shell::{doc_frame, idle_screen, viewer, Desktop, ShellWindow};
use super::FilmBar;

const STEPS: &[&str] = &["1 · Accountant's sheet", "2 · Process it", "3 · Fill the form", "4 · View the form", "5 · Reviewed"];

const LIT_VIEW: u8 = LIT_FILM;

const SHEET: &str = "payroll-oct-2026.xlsx";
const PDF: &str = "1601-C · October 2026 · 00001.pdf";

type Row = (&'static str, &'static str, &'static str, &'static str, &'static str, &'static str);

/// Per branch: (branch, its employees, its subtotal). A row is employee, gross, de minimis,
/// mandatory contributions, taxable, withheld (TRAIN's 2023 monthly table).
const PAYROLL: &[(&str, &[Row], Row)] = &[
    (
        "00001 · Dela Cruz Bakeshop, Makati",
        &[
            ("Santos, Ana", "42,000.00", "2,000.00", "3,400.00", "36,600.00", "2,528.40"),
            ("Reyes, Ben", "36,000.00", "2,000.00", "2,900.00", "31,100.00", "1,540.05"),
            ("Cruz, Carla", "32,000.00", "2,000.00", "2,600.00", "27,400.00", "985.05"),
            ("Bautista, Dan", "28,000.00", "2,000.00", "2,400.00", "23,600.00", "415.05"),
            ("Garcia, Ella", "26,000.00", "2,000.00", "2,300.00", "21,700.00", "130.05"),
            ("Mendoza, Fe", "22,000.00", "2,000.00", "2,000.00", "18,000.00", "0.00"),
        ],
        ("Subtotal", "186,000.00", "12,000.00", "15,600.00", "158,400.00", "5,598.60"),
    ),
    (
        "00002 · Dela Cruz Bakeshop, Quezon City",
        &[
            ("Lim, Gina", "30,000.00", "2,000.00", "2,500.00", "25,500.00", "700.05"),
            ("Torres, Hans", "26,000.00", "2,000.00", "2,300.00", "21,700.00", "130.05"),
            ("Villanueva, Ivy", "24,000.00", "2,000.00", "2,100.00", "19,900.00", "0.00"),
        ],
        ("Subtotal", "80,000.00", "6,000.00", "6,900.00", "67,100.00", "830.10"),
    ),
    (
        "00003 · JDC Apartments, Pasig",
        &[("Ramos, Jose", "15,000.00", "0.00", "1,200.00", "13,800.00", "0.00")],
        ("Subtotal", "15,000.00", "0.00", "1,200.00", "13,800.00", "0.00"),
    ),
];
const TOTAL: Row = ("All branches", "281,000.00", "18,000.00", "23,700.00", "239,300.00", "6,428.70");

/// One 1601-C per branch: branch, place, compensation, tax withheld.
const RETURNS: &[(&str, &str, &str, &str)] = &[
    ("00001", "Makati bakeshop", "₱186,000.00", "₱5,598.60"),
    ("00002", "QC bakeshop", "₱80,000.00", "₱830.10"),
    ("00003", "Pasig apartments", "₱15,000.00", "₱0.00"),
];

#[derive(Clone, Copy, PartialEq)]
enum Open {
    Nothing,
    Sheet,
    Form,
}

#[derive(Clone, Copy)]
enum B {
    L(Line),
    Draft,
}

#[derive(Clone, Copy)]
struct F {
    r: Rig,
    chat: Chat<B>,
    open: RwSignal<Open>,
    reviewed: RwSignal<bool>,
    view_btn: NodeRef<html::Div>,
}

fn script(s: &mut Script, f: F) {
    let (r, c) = (f.r, f.chat);
    s.then(move || r.act(0, "Attach the workbook your accountant sent, and say what it is for."))
        .wait(300);
    r.tap(s, r.plus_btn, LIT_PLUS, move || c.push(B::L(Line::File(SHEET, "3 branches · 10 employees · 31 KB"))));
    r.say(s, "From our accountant. Make October's 1601-C for each branch.", move || {
        c.push(B::L(Line::Me("From our accountant. Make October's 1601-C for each branch.")))
    });
    s.wait(300)
        .then(move || r.act(1, "The co-worker opens the workbook and reads each branch's payroll."));
    tool(s, r, c, B::L, "buwiz", "read_workbook", 500);
    s.then(move || {
        f.open.set(Open::Sheet);
        r.open_screen();
    })
    .wait(3000)
    .then(move || r.screen.shrink())
    .wait(700)
    .then(move || r.act(2, "It fills one 1601-C per branch from that branch's totals, then runs every BIR rule."));
    tool(s, r, c, B::L, "buwiz", "save_form_draft 1601-C × 3 · October 2026", 1100);
    tool(s, r, c, B::L, "buwiz", "validate_form × 3 · 0 errors", 900);
    s.then(move || c.push(B::Draft))
        .wait(1500)
        .then(move || r.act(3, "Open the form as BIR prints it. Spreadsheets, PDFs and pages all open right here."));
    r.tap(s, f.view_btn, LIT_VIEW, move || {
        f.open.set(Open::Form);
        r.open_screen();
    });
    s.wait(3400)
        .then(move || r.screen.shrink())
        .wait(700)
        .then(move || r.act(4, "Happy with it? Say so, and the draft is marked reviewed."));
    r.say(s, "Looks right. Mark them reviewed.", move || c.push(B::L(Line::Me("Looks right. Mark them reviewed."))));
    s.wait(300);
    tool(s, r, c, B::L, "buwiz", "mark_draft_reviewed × 3", 700);
    s.then(move || {
        f.reviewed.set(true);
        c.push(B::L(Line::Ai("All three are reviewed. Say “submit” when you want them filed.")));
    })
    .wait(300);
    r.end(s, "One workbook, three branches: three reviewed 1601-Cs, checked against BIR's rules.");
}

fn message(f: F, id: u32, b: B) -> AnyView {
    match b {
        B::L(l) => line_view(f.r, id, l),
        B::Draft => card(
            "description",
            "1601-C · October 2026 · 3 branches",
            "Mini app",
            view! {
                <div class="grid grid-cols-[auto_1fr_auto] gap-x-3 sm:grid-cols-[auto_1fr_auto_auto]">
                    <span class="gs-f pb-1 text-[11px]">"Branch"</span>
                    <span class="gs-f pb-1 text-[11px]"></span>
                    <span class="gs-f hidden pb-1 text-right text-[11px] sm:block">"Compensation"</span>
                    <span class="gs-f pb-1 text-right text-[11px]">"Tax withheld"</span>
                    {RETURNS.iter().map(|&(code, place, comp, tax)| view! {
                        <span class="gs-line border-t py-1 font-mono text-[11.5px]">{code}</span>
                        <span class="gs-line truncate border-t py-1">{place}</span>
                        <span class="gs-line gs-d hidden border-t py-1 text-right tabular-nums sm:block">{comp}</span>
                        <span class="gs-line border-t py-1 text-right font-medium tabular-nums">{tax}</span>
                    }).collect_view()}
                    <span class="gs-line border-t pt-1 font-semibold">"Total"</span>
                    <span class="gs-line border-t pt-1"></span>
                    <span class="gs-line gs-d hidden border-t pt-1 text-right tabular-nums sm:block">"₱281,000.00"</span>
                    <span class="gs-line border-t pt-1 text-right font-semibold tabular-nums">"₱6,428.70"</span>
                </div>
                <div class="mt-2 flex items-center gap-1.5 text-[11.5px] text-[#14C370]">
                    <span class="material-symbols-outlined text-[15px]">"verified"</span>
                    "All three pass every 1601-C rule"
                </div>
                <div class="mt-2.5 flex items-center gap-2">
                    <div node_ref=f.view_btn class=f.r.lit_class(LIT_VIEW, BTN_PRIMARY)>
                        <span class="material-symbols-outlined text-[14px]">"picture_as_pdf"</span>"View form"
                    </div>
                    <span class=BTN>"Open in Buwiz"</span>
                    <span class=move || if f.reviewed.get() {
                        "ml-auto rounded-full bg-[#14C370]/15 px-2 py-0.5 text-[10.5px] font-semibold text-[#14C370]"
                    } else {
                        "gs-hover gs-d ml-auto rounded-full px-2 py-0.5 text-[10.5px]"
                    }>{move || if f.reviewed.get() { "Reviewed" } else { "Draft" }}</span>
                </div>
            },
        )
        .into_any(),
    }
}

fn sheet() -> impl IntoView {
    const TH: &str = "border border-[#d4d4d8] bg-[#f4f4f5] px-2 py-1 text-left font-semibold";
    const TD: &str = "border border-[#e4e4e7] px-2 py-[3px]";
    let tr = |cls: &'static str, (n, g, d, m, t, w): Row| view! {
        <tr class=cls>
            <td class=TD>{n}</td><td class=format!("{TD} text-right")>{g}</td><td class=format!("{TD} text-right")>{d}</td>
            <td class=format!("{TD} text-right")>{m}</td><td class=format!("{TD} text-right")>{t}</td><td class=format!("{TD} text-right")>{w}</td>
        </tr>
    };
    view! {
        <div class="absolute inset-0 overflow-hidden bg-white p-4 text-[12px] text-[#18181b]">
            <div class="mb-2 font-semibold">"JUAN DELA CRUZ · TIN 123-456-788 · Payroll, October 2026"</div>
            <table class="w-full border-collapse tabular-nums">
                <tr>
                    <th class=TH>"Employee"</th><th class=TH>"Gross pay"</th><th class=TH>"De minimis"</th>
                    <th class=TH>"SSS / PhilHealth / Pag-IBIG"</th><th class=TH>"Taxable"</th><th class=TH>"Tax withheld"</th>
                </tr>
                {PAYROLL.iter().map(|&(branch, people, sub)| view! {
                    <tr><td colspan="6" class="border border-[#d4d4d8] bg-[#eef2ff] px-2 py-1 text-[11.5px] font-semibold text-[#3730a3]">{branch}</td></tr>
                    {people.iter().map(|&p| tr("", p)).collect_view()}
                    {tr("bg-[#f4f4f5] font-semibold", sub)}
                }).collect_view()}
                {tr("bg-[#dcfce7] font-semibold", TOTAL)}
            </table>
            <div class="mt-3 flex gap-1 text-[11px] text-[#71717a]">
                <span class="rounded-t border border-b-0 border-[#d4d4d8] bg-white px-2 py-0.5 font-semibold text-[#1d6f42]">"October"</span>
                <span class="px-2 py-0.5">"Employees"</span>
                <span class="px-2 py-0.5">"Notes"</span>
            </div>
        </div>
    }
}

/// The filled 1601-C exactly as Buwiz's print preview renders it.
pub fn form_page() -> impl IntoView {
    doc_frame("/assets/demo/1601c-october-2026.html")
}

fn desktop(f: F) -> AnyView {
    match f.open.get() {
        Open::Nothing => idle_screen().into_any(),
        Open::Sheet => viewer("xlsx", SHEET, view! { <span class="text-[11px] text-white/50">"Read only"</span> }, sheet()).into_any(),
        Open::Form => viewer(
            "pdf",
            PDF,
            view! {
                <span class="mr-2 flex items-center gap-1 font-mono text-[11px]">
                    <span class="rounded-full bg-white px-2 py-0.5 font-semibold text-[#1c1c1e]">"00001"</span>
                    <span class="rounded-full px-2 py-0.5 text-white/55">"00002"</span>
                    <span class="rounded-full px-2 py-0.5 text-white/55">"00003"</span>
                </span>
                <span class="rounded-full border border-white/20 px-2.5 py-0.5 text-[11px]">"Print"</span>
                <span class="rounded-full border border-white/20 px-2.5 py-0.5 text-[11px]">"Download"</span>
            },
            form_page(),
        )
        .into_any(),
    }
}

#[component]
pub fn PrepareFilm() -> impl IntoView {
    let r = Rig::new();
    let f = F { r, chat: Chat::new(r), open: RwSignal::new(Open::Nothing), reviewed: RwSignal::new(false), view_btn: NodeRef::new() };
    r.tl.script(move |s| {
        r.reset(&["Buwiz"], Vec::new());
        f.chat.clear();
        f.open.set(Open::Nothing);
        f.reviewed.set(false);
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
