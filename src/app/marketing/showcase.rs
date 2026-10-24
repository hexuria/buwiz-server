use super::downloads::{DownloadButtons, RELEASES_URL};
use super::{themed_screenshot, AccountActions};
use leptos::prelude::*;

#[derive(Clone, PartialEq, Debug)]
struct Feature {
    id: &'static str,
    title: &'static str,
    badge: &'static str,
    description: &'static str,
    detail: &'static str,
    icon: &'static str,
    dark_src: &'static str,
    light_src: &'static str,
}

struct Step {
    index: &'static str,
    title: &'static str,
    body: &'static str,
}

struct FaqItem {
    id: &'static str,
    question: &'static str,
    answer: &'static str,
}

const SALES_MAILTO: &str = "mailto:support@buwiz.com?subject=Buwiz%20Multi-Profile%20Plan";

const SECTION: &str = "py-20 px-4 sm:px-6 md:px-10 max-w-[1400px] mx-auto border-t border-gc-border";
const SECTION_TITLE: &str = "text-3xl md:text-4xl font-black tracking-tighter text-gc-text mb-3";
const SECTION_LEAD: &str = "text-base text-gc-text-muted leading-relaxed";
const CARD: &str = "rounded-xl border border-gc-border bg-gc-bg-elevated";
const CHECK_ROW: &str = "flex items-start gap-2.5 text-xs md:text-sm text-gc-text";
const CHECK_ICON: &str =
    "material-symbols-outlined text-gc-accent text-[18px] leading-tight shrink-0";

fn features() -> Vec<Feature> {
    vec![
        Feature {
            id: "preview",
            title: "Live Print Preview",
            badge: "Live",
            description: "Fill the on-screen form on the left and watch the BIR-format paper return update on the right as you type.",
            detail: "Print preview follows your open draft and redraws only the pages you changed, so you can check every comb box and schedule row before you print or file.",
            icon: "splitscreen",
            dark_src: "/assets/images/shell_side_by_side_dark.webp",
            light_src: "/assets/images/shell_side_by_side_light.webp",
        },
        Feature {
            id: "rules",
            title: "43 BIR Forms, Official Rules",
            badge: "43 Forms",
            description: "Every form is checked with rules built from the official eBIRForms package: 2,007 validations and 623 calculations across 9,592 fields.",
            detail: "Checks run as you type without freezing the window. Each message points at its numbered item, and the 'On this form' sidebar jumps to the next part that needs attention.",
            icon: "verified",
            dark_src: "/assets/images/shell_form_2550q_dark.webp",
            light_src: "/assets/images/shell_form_2550q_light.webp",
        },
        Feature {
            id: "templates",
            title: "Templates & Guided Mode",
            badge: "Tab",
            description: "Keep one template per form for each branch, then press Tab in Guided Mode to stop only at the fields that change each period.",
            detail: "Templates keep setup answers such as ATCs, tax elections, and categories, never last period's amounts. Turn any filed return into a template with 'Use as template'.",
            icon: "keyboard_tab",
            dark_src: "/assets/images/shell_templates_dark.webp",
            light_src: "/assets/images/shell_templates_light.webp",
        },
        Feature {
            id: "branches",
            title: "Taxpayers & Branches",
            badge: "Branches",
            description: "Keep your head office (00000) and branches under one 9-digit TIN. Branches inherit address and RDO until you change them.",
            detail: "Switch taxpayer or branch from the title bar. Each tax year has its own form checklist: copy last year's forms in one click or start from a Non-VAT, VAT, Employer, or Professional starter pack.",
            icon: "account_tree",
            dark_src: "/assets/images/shell_panel_profile_dark.webp",
            light_src: "/assets/images/shell_panel_profile_light.webp",
        },
        Feature {
            id: "deadlines",
            title: "Deadlines Calendar",
            badge: "30 Days",
            description: "See every return due in the next 30 days across your taxpayers and branches, overdue items first.",
            detail: "Home lists overdue and upcoming deadlines with one-click Open, plus recently filed returns you can reuse as templates.",
            icon: "event_upcoming",
            dark_src: "/assets/images/shell_calendar_dark.webp",
            light_src: "/assets/images/shell_calendar_light.webp",
        },
        Feature {
            id: "search",
            title: "Search Everything",
            badge: "Cmd+K",
            description: "Jump to any taxpayer, branch, recent return, or any of the 43 forms from the keyboard.",
            detail: "Press Cmd+K on macOS or Ctrl+K on Windows and Linux, then search by TIN, name, or form code such as 1601C, 2550Q, or 1701.",
            icon: "search",
            dark_src: "/assets/images/shell_palette_dark.webp",
            light_src: "/assets/images/shell_palette_light.webp",
        },
    ]
}

const ACCOUNT_STEPS: [Step; 3] = [
    Step {
        index: "01",
        title: "Create your account",
        body: "Sign up free with your email, here or from the desktop app the first time it opens.",
    },
    Step {
        index: "02",
        title: "Add your TIN",
        body: "Each TIN and branch has exactly one holder. If another account already holds yours, claim it with your BIR Form 2303; claims are reviewed before ownership moves.",
    },
    Step {
        index: "03",
        title: "Sign in on your desktop",
        body: "Approve the app once in your browser. It stays signed in and keeps working offline until you sign out.",
    },
];

const FAQS: [FaqItem; 5] = [
    FaqItem {
        id: "account",
        question: "Do I need a Buwiz account?",
        answer: "Yes. The desktop app asks you to sign in or create a free account the first time it opens. Your account is how Buwiz knows which TINs are yours and keeps each TIN with a single holder.",
    },
    FaqItem {
        id: "offline",
        question: "Does Buwiz work offline?",
        answer: "Yes, after that first sign-in. Your taxpayers, templates, and returns are stored encrypted on your computer, and the app stays signed in until you sign out. You need a connection to file a return, check for updates, and add or claim a TIN.",
    },
    FaqItem {
        id: "agent",
        question: "Can an AI agent file a return for me?",
        answer: "No. bir-mcp, the agent server built into Buwiz, can open, fill, and check drafts, but it has no submit, queue, or payment tool. It only runs after you sign it in with your Buwiz account, and filing always takes your own click in the app.",
    },
    FaqItem {
        id: "pricing",
        question: "What is free, and when do I pay?",
        answer: "Buwiz is free for one tax profile (one 9-digit TIN), including all 43 forms, print preview, templates, Guided Mode, and AI agent drafting. A paid plan is needed only when you file for several TINs or businesses.",
    },
    FaqItem {
        id: "claims",
        question: "How does claiming a TIN work?",
        answer: "If another Buwiz account already holds your TIN, upload your BIR Form 2303 from the Tax profiles page. A reviewer checks it, and the TIN moves to you only if the claim is approved.",
    },
];

/// The whole page body is one island: the hero tabs, feature picker, and FAQ
/// are client state and it needs no server data.
#[island]
pub fn Showcase() -> impl IntoView {
    let features = features();
    let (hero_tab, set_hero_tab) = signal(0usize);
    let (active, set_active) = signal(features[0].clone());
    let (open_faq, set_open_faq) = signal(Some("account"));

    view! {
        <div class="pb-24">
            <section id="hero" class="pt-14 md:pt-20 pb-16 px-4 sm:px-6 md:px-10 max-w-[1400px] mx-auto">
                <div class="text-center max-w-3xl mx-auto">
                    <span class="inline-block text-[11px] font-bold uppercase tracking-[0.18em] text-gc-accent mb-5 animate-rise">
                        "THE MODERN ALTERNATIVE TO EBIRFORMS"
                    </span>
                    <h1 class="text-4xl sm:text-5xl md:text-6xl lg:text-7xl font-black tracking-tighter text-gc-text leading-[1.02] mb-6 animate-rise animate-rise-d1">
                        "Philippine tax filing. Finally modern."
                    </h1>
                    <p class="text-base md:text-lg text-gc-text-muted leading-relaxed max-w-[58ch] mx-auto mb-9 animate-rise animate-rise-d2">
                        "Buwiz Forms is a fast, free alternative to eBIRForms for Windows, macOS, and Linux. Fill all 43 BIR forms with the official rules checking as you type."
                    </p>
                    <div id="download" class="flex flex-wrap items-center justify-center gap-3 animate-rise animate-rise-d3">
                        <DownloadButtons />
                    </div>
                    <p class="mt-4 text-[11px] text-gc-text-faint animate-rise animate-rise-d3">
                        "Always the latest release — Windows Setup.exe · universal macOS DMG · Linux .deb. "
                        <a href=RELEASES_URL target="_blank" rel="noopener noreferrer" class="underline underline-offset-2 hover:text-gc-text-muted transition-colors">
                            "All packages & release notes"
                        </a>
                    </p>
                </div>

                <div class="max-w-5xl mx-auto mt-12 md:mt-16 animate-rise animate-rise-d3">
                    <div class="rounded-xl border border-gc-border bg-gc-bg-elevated shadow-2xl overflow-hidden">
                        <div class="px-4 py-2.5 bg-gc-bg-card border-b border-gc-border flex flex-wrap items-center justify-between gap-3">
                            <div class="flex items-center gap-2">
                                <span class="text-xs font-semibold text-gc-text">"Buwiz Forms"</span>
                                <span class="text-[11px] text-gc-text-faint tabular-nums">"123-456-788-00000"</span>
                            </div>
                            <div class="inline-flex items-center gap-1 p-0.5 rounded-lg bg-gc-bg-hover border border-gc-border max-w-full overflow-x-auto" role="tablist" aria-label="App preview">
                                {[(0usize, "Live Print Preview"), (1usize, "Templates"), (2usize, "Home & Deadlines")]
                                    .into_iter()
                                    .map(|(idx, label)| {
                                        let selected = move || hero_tab.get() == idx;
                                        view! {
                                            <button
                                                type="button"
                                                role="tab"
                                                aria-selected=move || selected().to_string()
                                                on:click=move |_| set_hero_tab.set(idx)
                                                class=move || format!(
                                                    "px-2.5 py-1 rounded-md text-[11px] font-semibold transition-colors cursor-pointer whitespace-nowrap {}",
                                                    if selected() { "bg-gc-bg-elevated text-gc-text shadow-xs" } else { "text-gc-text-muted hover:text-gc-text" }
                                                )
                                            >
                                                {label}
                                            </button>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        </div>
                        <div class="relative aspect-[1440/1129] w-full bg-gc-bg overflow-hidden">
                            {themed_screenshot(
                                move |dark| match hero_tab.get() {
                                    0 => if dark { "/assets/images/shell_side_by_side_dark.webp" } else { "/assets/images/shell_side_by_side_light.webp" },
                                    1 => if dark { "/assets/images/shell_templates_dark.webp" } else { "/assets/images/shell_templates_light.webp" },
                                    _ => if dark { "/assets/images/shell_home_dark.webp" } else { "/assets/images/shell_home_light.webp" },
                                },
                                || "Buwiz Forms desktop app showing a BIR form editor beside its live print preview",
                                true,
                            )}
                        </div>
                    </div>
                </div>

                <div class="mt-10 md:mt-12 flex flex-wrap items-center justify-center gap-x-7 gap-y-3">
                    {[
                        ("devices", "Windows · macOS · Linux"),
                        ("description", "All 43 BIR forms"),
                        ("cloud_off", "Works offline after sign-in"),
                        ("smart_toy", "AI drafts, you file"),
                        ("money_off", "Free for one TIN"),
                    ]
                        .into_iter()
                        .map(|(icon, label)| view! {
                            <span class="inline-flex items-center gap-2 text-xs font-medium text-gc-text-muted">
                                <span class="material-symbols-outlined text-[15px] text-gc-accent leading-none">{icon}</span>
                                {label}
                            </span>
                        })
                        .collect_view()}
                </div>
            </section>

            <section id="features" class=SECTION>
                <div class="mb-12 max-w-[65ch]">
                    <h2 class=SECTION_TITLE>"Everything you file, in one calm window."</h2>
                    <p class=SECTION_LEAD>
                        "Built around how business owners, bookkeepers, and CPAs actually file: pick the taxpayer, open the form, fill only what changed."
                    </p>
                </div>

                <div class="grid grid-cols-1 lg:grid-cols-12 gap-8 items-start">
                    <div class="lg:col-span-5 flex flex-col gap-2.5">
                        {features
                            .into_iter()
                            .map(|feature| {
                                let id = feature.id;
                                let pick = feature.clone();
                                let shot = feature.clone();
                                let is_selected = move || active.read().id == id;
                                view! {
                                    <div
                                        on:click=move |_| set_active.set(pick.clone())
                                        class=move || format!(
                                            "w-full text-left p-4 rounded-xl border transition-all cursor-pointer {}",
                                            if is_selected() {
                                                "bg-gc-bg-elevated border-gc-accent ring-1 ring-gc-accent/25 shadow-sm"
                                            } else {
                                                "bg-gc-bg-card/60 border-gc-border hover:bg-gc-bg-hover hover:border-gc-border-strong"
                                            }
                                        )
                                    >
                                        <div class="flex items-start gap-3.5">
                                            <div class=move || format!(
                                                "w-9 h-9 rounded-lg flex items-center justify-center shrink-0 mt-0.5 transition-colors {}",
                                                if is_selected() { "bg-gc-accent-soft text-gc-accent" } else { "bg-gc-bg-hover text-gc-text-muted" }
                                            )>
                                                <span class="material-symbols-outlined text-[18px]">{feature.icon}</span>
                                            </div>
                                            <div class="flex-1 min-w-0">
                                                <div class="flex items-center justify-between gap-2 mb-1">
                                                    <h3 class="text-sm font-bold tracking-tight text-gc-text truncate">{feature.title}</h3>
                                                    <span class=move || format!(
                                                        "text-[10px] font-semibold px-2 py-0.5 rounded-full shrink-0 {}",
                                                        if is_selected() { "bg-gc-accent-soft text-gc-accent" } else { "bg-gc-bg-hover text-gc-text-faint" }
                                                    )>
                                                        {feature.badge}
                                                    </span>
                                                </div>
                                                <p class="text-xs text-gc-text-muted leading-relaxed">{feature.description}</p>
                                            </div>
                                        </div>

                                        <div class=move || if is_selected() { "block lg:hidden mt-4 pt-3 border-t border-gc-border" } else { "hidden" }>
                                            <div class="rounded-lg overflow-hidden border border-gc-border bg-gc-bg aspect-[1440/1129]">
                                                {
                                                    let title = shot.title;
                                                    let pair = (shot.dark_src, shot.light_src);
                                                    themed_screenshot(move |dark| if dark { pair.0 } else { pair.1 }, move || title, false)
                                                }
                                            </div>
                                            <p class="text-xs text-gc-text-muted leading-relaxed mt-3">{shot.detail}</p>
                                        </div>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>

                    <div class="hidden lg:block lg:col-span-7 sticky top-20">
                        <div class="rounded-xl border border-gc-border bg-gc-bg-elevated shadow-xl overflow-hidden">
                            <div class="px-5 py-3 bg-gc-bg-card border-b border-gc-border flex items-center justify-between">
                                <div class="flex items-center gap-2.5">
                                    <span class="material-symbols-outlined text-[16px] text-gc-accent">{move || active.read().icon}</span>
                                    <span class="text-xs font-bold text-gc-text">{move || active.read().title}</span>
                                </div>
                                <span class="text-[11px] font-semibold px-2.5 py-0.5 rounded-full bg-gc-accent-soft text-gc-accent">
                                    {move || active.read().badge}
                                </span>
                            </div>
                            <div class="relative aspect-[1440/1129] w-full bg-gc-bg overflow-hidden">
                                {themed_screenshot(
                                    move |dark| { let cur = active.read(); if dark { cur.dark_src } else { cur.light_src } },
                                    move || active.read().title,
                                    false,
                                )}
                            </div>
                            <div class="p-5 bg-gc-bg-card border-t border-gc-border">
                                <p class="text-xs md:text-sm text-gc-text-muted leading-relaxed">{move || active.read().detail}</p>
                            </div>
                        </div>
                    </div>
                </div>
            </section>

            <section id="account" class=SECTION>
                <div class="mb-12 max-w-[65ch]">
                    <h2 class=SECTION_TITLE>"One Buwiz account. Your TIN stays yours."</h2>
                    <p class=SECTION_LEAD>
                        "The same account signs you in on the web and in the desktop app, and it is how Buwiz keeps every TIN with its rightful holder."
                    </p>
                </div>
                <ol class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-8">
                    {ACCOUNT_STEPS
                        .iter()
                        .map(|step| view! {
                            <li class=format!("{CARD} p-6")>
                                <span class="block text-xs font-bold tabular-nums text-gc-accent mb-3">{step.index}</span>
                                <h3 class="text-lg font-bold tracking-tight text-gc-text mb-2">{step.title}</h3>
                                <p class="text-sm text-gc-text-muted leading-relaxed">{step.body}</p>
                            </li>
                        })
                        .collect_view()}
                </ol>
                <div class="flex flex-wrap gap-3">
                    <AccountActions />
                </div>
            </section>

            <section id="agent" class=SECTION>
                <div class=format!("{CARD} p-7 md:p-10 grid grid-cols-1 lg:grid-cols-2 gap-8 items-center")>
                    <div>
                        <span class="inline-block text-[11px] font-bold uppercase tracking-[0.18em] text-gc-accent mb-4">"AI AGENT"</span>
                        <h2 class=SECTION_TITLE>"Let an AI agent prepare the draft."</h2>
                        <p class=SECTION_LEAD>
                            "Buwiz ships with bir-mcp, an MCP server your AI agent can use to open, fill, and check any of the 43 forms. You review the result in the app and file it yourself."
                        </p>
                    </div>
                    <div class="grid grid-cols-1 gap-3">
                        {[
                            ("edit_note", "Drafts only", "There is no submit, queue, or payment tool. Filing always takes your click."),
                            ("key", "Runs on your sign-in", "The agent works only after you approve it with your Buwiz account."),
                            ("link_off", "Revoke anytime", "Disconnect an agent from your account settings and its access ends."),
                        ]
                            .into_iter()
                            .map(|(icon, title, body)| view! {
                                <div class="flex items-start gap-3.5 p-4 rounded-lg bg-gc-bg border border-gc-border">
                                    <div class="w-9 h-9 rounded-lg bg-gc-accent-soft text-gc-accent flex items-center justify-center shrink-0">
                                        <span class="material-symbols-outlined text-[18px]">{icon}</span>
                                    </div>
                                    <div>
                                        <div class="text-sm font-bold text-gc-text mb-0.5">{title}</div>
                                        <p class="text-xs text-gc-text-muted leading-relaxed">{body}</p>
                                    </div>
                                </div>
                            })
                            .collect_view()}
                    </div>
                </div>
            </section>

            <section id="pricing" class=SECTION>
                <div class="mb-12 max-w-[65ch]">
                    <h2 class=SECTION_TITLE>"Free for one TIN. Pay only when you file for more."</h2>
                    <p class=SECTION_LEAD>
                        "Download and file at no cost. Upgrade when you manage several tax profiles or businesses."
                    </p>
                </div>

                <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                    <div class=format!("{CARD} p-7 md:p-9 flex flex-col justify-between")>
                        <div>
                            <div class="flex items-center justify-between gap-3 mb-4">
                                <h3 class="text-xl font-bold text-gc-text">"Personal"</h3>
                                <span class="text-[11px] font-semibold px-3 py-1 rounded-full bg-gc-accent-soft text-gc-accent">"Free Forever"</span>
                            </div>
                            <div class="flex items-baseline gap-2 mb-3">
                                <span class="text-4xl md:text-5xl font-black tracking-tight text-gc-text tabular-nums">"₱0"</span>
                                <span class="text-xs font-semibold text-gc-text-muted">"/ forever for 1 tax profile"</span>
                            </div>
                            <p class="text-sm text-gc-text-muted leading-relaxed mb-6">
                                "For individuals, freelancers, professionals, or a single business with one 9-digit TIN."
                            </p>
                            <div class="space-y-3 pt-6 border-t border-gc-border mb-8">
                                {[
                                    "1 tax profile (one 9-digit TIN)",
                                    "All 43 BIR forms with official rule checks",
                                    "Live BIR-format print preview",
                                    "Templates & Guided Mode",
                                    "AI agent drafting with bir-mcp",
                                    "Encrypted on your computer, works offline after sign-in",
                                ]
                                    .into_iter()
                                    .map(|item| view! {
                                        <div class=CHECK_ROW>
                                            <span class=CHECK_ICON>"check_circle"</span>
                                            <span>{item}</span>
                                        </div>
                                    })
                                    .collect_view()}
                            </div>
                        </div>
                        <div class="flex flex-wrap gap-3">
                            <DownloadButtons />
                        </div>
                    </div>

                    <div class="rounded-xl border border-gc-accent/40 bg-gc-bg-elevated p-7 md:p-9 flex flex-col justify-between ring-1 ring-gc-accent/20">
                        <div>
                            <div class="flex items-center justify-between gap-3 mb-4">
                                <h3 class="text-xl font-bold text-gc-text">"Multiple Tax Profiles"</h3>
                                <span class="text-[11px] font-semibold px-3 py-1 rounded-full bg-gc-accent text-white">"Paid Plan"</span>
                            </div>
                            <div class="flex items-baseline gap-2 mb-3">
                                <span class="text-4xl md:text-5xl font-black tracking-tight text-gc-text">"Pro"</span>
                                <span class="text-xs font-semibold text-gc-text-muted">"/ several TINs & businesses"</span>
                            </div>
                            <p class="text-sm text-gc-text-muted leading-relaxed mb-6">
                                "For owners running more than one business and teams filing for several TINs."
                            </p>
                            <div class="space-y-3 pt-6 border-t border-gc-border mb-8">
                                {[
                                    "Everything in Personal, plus:",
                                    "Multiple tax profiles & business TINs",
                                    "Branches under each TIN with head-office inheritance",
                                    "Bulk copy templates and form setups across branches & TINs",
                                ]
                                    .into_iter()
                                    .map(|item| view! {
                                        <div class=CHECK_ROW>
                                            <span class=CHECK_ICON>"check_circle"</span>
                                            <span>{item}</span>
                                        </div>
                                    })
                                    .collect_view()}
                            </div>
                        </div>
                        <div class="flex flex-wrap gap-3">
                            <a
                                href=SALES_MAILTO
                                class="h-11 px-6 rounded-lg bg-gc-accent text-white font-bold text-sm inline-flex items-center justify-center gap-2 hover:bg-gc-accent-hover hover:-translate-y-[1px] active:scale-[0.98] transition-all whitespace-nowrap"
                            >
                                <span class="material-symbols-outlined text-[18px] leading-none">"mail"</span>
                                <span>"Contact Sales"</span>
                            </a>
                        </div>
                    </div>
                </div>
            </section>

            <section id="faq" class=SECTION>
                <div class="max-w-4xl mx-auto">
                    <h2 class="text-3xl md:text-4xl font-black tracking-tighter text-gc-text mb-8 text-center">"Questions"</h2>
                    <div class="rounded-xl border border-gc-border bg-gc-bg-elevated divide-y divide-gc-border overflow-hidden">
                        {FAQS
                            .iter()
                            .map(|faq| {
                                let id = faq.id;
                                let is_open = move || open_faq.get() == Some(id);
                                view! {
                                    <div class="p-5">
                                        <button
                                            type="button"
                                            aria-expanded=move || is_open().to_string()
                                            on:click=move |_| set_open_faq.update(|cur| {
                                                *cur = if *cur == Some(id) { None } else { Some(id) };
                                            })
                                            class="w-full flex items-center justify-between gap-4 text-left cursor-pointer"
                                        >
                                            <span class="text-sm md:text-base font-bold text-gc-text">{faq.question}</span>
                                            <span class="material-symbols-outlined text-gc-text-muted text-[20px] shrink-0">
                                                {move || if is_open() { "expand_less" } else { "expand_more" }}
                                            </span>
                                        </button>
                                        <div class=move || if is_open() { "mt-3 text-xs md:text-sm text-gc-text-muted leading-relaxed" } else { "hidden" }>
                                            {faq.answer}
                                        </div>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>
                </div>
            </section>
        </div>
    }
}
