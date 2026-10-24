//! Film 3, Prepare a return: attach the accountant's payroll workbook, the co-worker opens it,
//! fills October's 1601-C from it, checks it against BIR's rules, shows the form as a PDF, and
//! marks it reviewed when you say so.

use std::rc::Rc;

use leptos::{html, prelude::*};

use super::kit::{card, line_view, rows, tool, Chat, Line, Rig, BTN, BTN_PRIMARY, LIT_FILM, LIT_PLUS};
use super::player::Script;
use super::shell::{idle_screen, viewer, Desktop, ShellWindow};
use super::FilmBar;

const STEPS: &[&str] = &["1 · Accountant's sheet", "2 · Process it", "3 · Fill the form", "4 · View the form", "5 · Reviewed"];

const LIT_VIEW: u8 = LIT_FILM;

const SHEET: &str = "payroll-oct-2026.xlsx";
const PDF: &str = "1601-C · October 2026.pdf";

/// Employee, gross, de minimis, mandatory contributions, taxable, withheld.
const PAYROLL: &[(&str, &str, &str, &str, &str, &str)] = &[
    ("Santos, Ana", "42,000.00", "2,000.00", "3,400.00", "36,600.00", "2,528.40"),
    ("Reyes, Ben", "36,000.00", "2,000.00", "2,900.00", "31,100.00", "1,540.05"),
    ("Cruz, Carla", "32,000.00", "2,000.00", "2,600.00", "27,400.00", "985.05"),
    ("Bautista, Dan", "28,000.00", "2,000.00", "2,400.00", "23,600.00", "415.05"),
    ("Garcia, Ella", "26,000.00", "2,000.00", "2,300.00", "21,700.00", "130.05"),
    ("Mendoza, Fe", "22,000.00", "2,000.00", "2,000.00", "18,000.00", "0.00"),
];
const TOTAL: (&str, &str, &str, &str, &str) = ("186,000.00", "12,000.00", "15,600.00", "158,400.00", "5,598.60");

/// Item, label, amount.
const ITEMS: &[(&str, &str, &str)] = &[
    ("14", "Total amount of compensation", "186,000.00"),
    ("18", "De minimis benefits", "12,000.00"),
    ("19", "Mandatory contributions and union dues", "15,600.00"),
    ("21", "Total non-taxable compensation", "27,600.00"),
    ("22", "Total taxable compensation", "158,400.00"),
    ("24", "Net taxable compensation", "158,400.00"),
    ("25", "Total taxes withheld", "5,598.60"),
    ("27", "Taxes withheld for remittance", "5,598.60"),
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
    r.tap(s, r.plus_btn, LIT_PLUS, move || c.push(B::L(Line::File(SHEET, "Sheet1 · 6 employees · 24 KB"))));
    r.say(s, "From our accountant. Make October's 1601-C from this.", move || {
        c.push(B::L(Line::Me("From our accountant. Make October's 1601-C from this.")))
    });
    s.wait(300)
        .then(move || r.act(1, "The co-worker opens the workbook on its computer and reads the payroll."));
    tool(s, r, c, B::L, "buwiz", "read_workbook", 500);
    s.then(move || {
        f.open.set(Open::Sheet);
        r.screen.expand();
    })
    .wait(3000)
    .then(move || r.screen.shrink())
    .wait(700)
    .then(move || r.act(2, "It fills the form from the totals, then runs every BIR rule for 1601-C."));
    tool(s, r, c, B::L, "buwiz", "save_form_draft 1601-C · October 2026", 1000);
    tool(s, r, c, B::L, "buwiz", "validate_form · 0 errors", 900);
    s.then(move || c.push(B::Draft))
        .wait(1500)
        .then(move || r.act(3, "Open the form as BIR prints it. Spreadsheets, PDFs and pages all open right here."));
    r.tap(s, f.view_btn, LIT_VIEW, move || {
        f.open.set(Open::Form);
        r.screen.expand();
    });
    s.wait(3400)
        .then(move || r.screen.shrink())
        .wait(700)
        .then(move || r.act(4, "Happy with it? Say so, and the draft is marked reviewed."));
    r.say(s, "Looks right. Mark it reviewed.", move || c.push(B::L(Line::Me("Looks right. Mark it reviewed."))));
    s.wait(300);
    tool(s, r, c, B::L, "buwiz", "mark_draft_reviewed", 700);
    s.then(move || {
        f.reviewed.set(true);
        c.push(B::L(Line::Ai("Marked reviewed. Say “submit” when you want it filed.")));
    })
    .wait(300);
    r.end(s, "From the accountant's workbook to a reviewed 1601-C, checked against BIR's rules.");
}

fn message(f: F, id: u32, b: B) -> AnyView {
    match b {
        B::L(l) => line_view(f.r, id, l),
        B::Draft => card(
            "description",
            "1601-C · October 2026",
            "Mini app",
            view! {
                {rows(&[
                    ("Total compensation", "₱186,000.00"),
                    ("Non-taxable", "₱27,600.00"),
                    ("Taxable compensation", "₱158,400.00"),
                    ("Tax withheld", "₱5,598.60"),
                ])}
                <div class="mt-2 flex items-center gap-1.5 text-[11.5px] text-[#14C370]">
                    <span class="material-symbols-outlined text-[15px]">"verified"</span>
                    "Passes all 1601-C rules"
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
    const TD: &str = "border border-[#e4e4e7] px-2 py-1";
    view! {
        <div class="absolute inset-0 overflow-hidden bg-white p-4 text-[12.5px] text-[#18181b]">
            <div class="mb-2 font-semibold">"DELA CRUZ BAKESHOP · Payroll, October 2026"</div>
            <table class="w-full border-collapse tabular-nums">
                <tr>
                    <th class=TH>"Employee"</th><th class=TH>"Gross pay"</th><th class=TH>"De minimis"</th>
                    <th class=TH>"SSS / PhilHealth / Pag-IBIG"</th><th class=TH>"Taxable"</th><th class=TH>"Tax withheld"</th>
                </tr>
                {PAYROLL.iter().map(|&(n, g, d, m, t, w)| view! {
                    <tr>
                        <td class=TD>{n}</td><td class=format!("{TD} text-right")>{g}</td><td class=format!("{TD} text-right")>{d}</td>
                        <td class=format!("{TD} text-right")>{m}</td><td class=format!("{TD} text-right")>{t}</td><td class=format!("{TD} text-right")>{w}</td>
                    </tr>
                }).collect_view()}
                <tr class="bg-[#dcfce7] font-semibold">
                    <td class=TD>"Total"</td><td class=format!("{TD} text-right")>{TOTAL.0}</td><td class=format!("{TD} text-right")>{TOTAL.1}</td>
                    <td class=format!("{TD} text-right")>{TOTAL.2}</td><td class=format!("{TD} text-right")>{TOTAL.3}</td><td class=format!("{TD} text-right")>{TOTAL.4}</td>
                </tr>
            </table>
            <div class="mt-3 flex gap-1 text-[11px] text-[#71717a]">
                <span class="rounded-t border border-b-0 border-[#d4d4d8] bg-white px-2 py-0.5 font-semibold text-[#1d6f42]">"Sheet1"</span>
                <span class="px-2 py-0.5">"Notes"</span>
            </div>
        </div>
    }
}

/// A plain rendition of the filled form's first page.
pub fn form_page() -> impl IntoView {
    view! {
        <div class="absolute inset-0 overflow-hidden bg-[#525659] p-4">
            <div class="mx-auto h-full max-w-[560px] overflow-hidden bg-white p-5 text-[11px] text-black shadow-xl">
                <div class="flex items-start justify-between border-b-2 border-black pb-2">
                    <div>
                        <div class="text-[9px]">"Republic of the Philippines · Bureau of Internal Revenue"</div>
                        <div class="text-[13px] font-black">"BIR Form No. 1601-C"</div>
                        <div class="text-[10px] font-semibold">"Monthly Remittance Return of Income Taxes Withheld on Compensation"</div>
                    </div>
                    <div class="text-right text-[10px]">"For the month"<div class="font-mono text-[12px] font-bold">"10 / 2026"</div></div>
                </div>
                <div class="grid grid-cols-2 gap-x-4 border-b border-black py-1.5 text-[10px]">
                    <div>"TIN "<span class="font-mono font-bold">"123 456 788 00000"</span></div>
                    <div>"RDO "<span class="font-mono font-bold">"047"</span>"   ATC "<span class="font-mono font-bold">"WW010"</span></div>
                    <div class="col-span-2">"Withholding agent "<span class="font-bold">"DELA CRUZ BAKESHOP"</span></div>
                </div>
                <div class="mt-1.5 bg-black px-1.5 py-0.5 text-[10px] font-bold text-white">"Part II · Computation of Tax"</div>
                {ITEMS.iter().map(|&(n, label, amt)| view! {
                    <div class="flex items-center gap-2 border-b border-[#d4d4d4] py-[3px]">
                        <span class="w-5 font-bold">{n}</span>
                        <span class="flex-1">{label}</span>
                        <span class="w-[110px] border border-black px-1 text-right font-mono">{amt}</span>
                    </div>
                }).collect_view()}
            </div>
        </div>
    }
}

fn desktop(f: F) -> AnyView {
    match f.open.get() {
        Open::Nothing => idle_screen().into_any(),
        Open::Sheet => viewer("xlsx", SHEET, view! { <span class="text-[11px] text-white/50">"Read only"</span> }, sheet()).into_any(),
        Open::Form => viewer(
            "pdf",
            PDF,
            view! {
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
