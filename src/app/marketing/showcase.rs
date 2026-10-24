use super::downloads::DownloadButton;
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
        question: "Can an AI Co-worker file and pay for me?",
        answer: "Yes, with your approval. Your co-worker can prepare a return, submit it and pay it online, but it only acts on the profiles and actions you allowed on your Buwiz account, and it stops to ask you before every submission and every payment.",
    },
    FaqItem {
        id: "pricing",
        question: "What is free, and when do I pay?",
        answer: "Buwiz is free for one tax profile (one 9-digit TIN), including all 43 forms, print preview, templates, Guided Mode, and the AI Co-worker. A paid plan is needed only when you file for several TINs or businesses.",
    },
    FaqItem {
        id: "claims",
        question: "How does claiming a TIN work?",
        answer: "If another Buwiz account already holds your TIN, upload your BIR Form 2303 from the Tax profiles page. A reviewer checks it, and the TIN moves to you only if the claim is approved.",
    },
];

/// The six AI Co-worker demos, in filing order: (eyebrow, title, lead).
const DEMOS: [(&str, &str, &str); 6] = [
    (
        "CONNECT",
        "Install Buwiz from the chat. Log in with Touch ID.",
        "Ask your co-worker to install the Buwiz plugin, then say “Log in with Buwiz”. A login card appears in the chat and your saved password fills it with Touch ID. You choose what the co-worker may do on your account.",
    ),
    (
        "TAX PROFILES",
        "One TIN, every branch, as live cards.",
        "A salary, two bakeshops and an apartment: a mixed-income earner keeps them all under one TIN. Ask in plain words and get your branches, the tax year, which branch files which form and what falls due next. Keep any of them as a Mini App.",
    ),
    (
        "PREPARE",
        "One spreadsheet for every branch. A finished form for each.",
        "Attach the payroll workbook your accountant sent for all your branches. The co-worker reads it, fills one 1601-C per branch, checks each against every official rule and opens them as BIR prints them, for you to review.",
    ),
    (
        "SUBMIT",
        "Say “submit”. Approve it. It is queued.",
        "The co-worker checks the form again, asks for permission to publish if you never gave it, and shows you exactly what will be filed. Nothing leaves until you approve.",
    ),
    (
        "CONFIRMATION",
        "A routine watches your inbox for BIR's confirmation.",
        "Install Gmail and ask for a routine. Every minute it looks for BIR's confirmation email; when it arrives, the confirmation comes back as a PDF to print or keep.",
    ),
    (
        "PAY",
        "Paid online through LandBank, with your OK.",
        "A second routine waits for the confirmation, then opens the bank's payment site on the co-worker's computer and fills in BIR, the form, the period, the ATC and the amount. You log in with Touch ID and approve the amount.",
    ),
];

#[cfg(feature = "hydrate")]
fn agent_demo(n: usize) -> AnyView {
    use super::demo::*;
    match n {
        0 => view! { <ConnectFilm /> }.into_any(),
        1 => view! { <ProfilesFilm /> }.into_any(),
        2 => view! { <PrepareFilm /> }.into_any(),
        3 => view! { <SubmitFilm /> }.into_any(),
        4 => view! { <ConfirmFilm /> }.into_any(),
        _ => view! { <PayFilm /> }.into_any(),
    }
}

/// Holds a demo's height on the server build; the page renders client-side.
#[cfg(not(feature = "hydrate"))]
fn agent_demo(_n: usize) -> AnyView {
    view! { <div class="h-[720px]"></div> }.into_any()
}

/// The hero's picture: one month of a mixed-income earner's taxes, from the accountant's
/// payroll to a paid return, under the deadline that starts it.
fn hero_journey() -> impl IntoView {
    const STEP: &str = "relative flex flex-col rounded-xl border border-gc-border bg-gc-bg-card p-4 text-left";
    const NUM: &str = "grid h-6 w-6 shrink-0 place-items-center rounded-full bg-gc-accent-soft text-[11px] font-bold text-gc-accent";
    const ROW: &str = "flex items-center justify-between gap-2 border-t border-gc-border py-1.5 text-[12px]";
    view! {
        <div class="relative">
            <div class="pointer-events-none absolute -inset-x-6 -inset-y-8 -z-10 rounded-[40px] bg-[radial-gradient(ellipse_at_center,var(--color-gc-accent-soft),transparent_70%)] opacity-80"></div>
            <div class="rounded-2xl border border-gc-border bg-gc-bg-elevated p-3 shadow-2xl sm:p-4">
                <div class="mb-3 flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl border border-[#f59e0b]/30 bg-[#f59e0b]/10 px-3.5 py-2.5 text-left">
                    <span class="material-symbols-outlined text-[20px] text-[#d97706]">"event_upcoming"</span>
                    <div class="min-w-0 flex-1">
                        <div class="text-[13px] font-bold text-gc-text">"1601-C for October is due Nov 10"</div>
                        <div class="text-[11.5px] text-gc-text-muted">"JUAN DELA CRUZ · TIN 123-456-788 · 3 branches with staff"</div>
                    </div>
                    <span class="rounded-full bg-[#f59e0b]/15 px-2.5 py-0.5 text-[11px] font-semibold text-[#d97706]">"17 days left"</span>
                </div>
                <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
                    <div class=STEP>
                        <div class="mb-2 flex items-center gap-2"><span class=NUM>"1"</span><span class="text-[13px] font-bold text-gc-text">"Prepare"</span></div>
                        <p class="mb-2 text-[12px] leading-snug text-gc-text-muted">"Send the accountant's payroll. One return per branch, filled for you."</p>
                        <div class="mt-auto">
                            <div class=ROW><span class="font-mono text-gc-text-muted">"00001 Makati"</span><span class="tabular-nums text-gc-text">"₱5,598.60"</span></div>
                            <div class=ROW><span class="font-mono text-gc-text-muted">"00002 QC"</span><span class="tabular-nums text-gc-text">"₱830.10"</span></div>
                            <div class=ROW><span class="font-mono text-gc-text-muted">"00003 Pasig"</span><span class="tabular-nums text-gc-text">"₱0.00"</span></div>
                        </div>
                    </div>
                    <div class=STEP>
                        <div class="mb-2 flex items-center gap-2"><span class=NUM>"2"</span><span class="text-[13px] font-bold text-gc-text">"Check"</span></div>
                        <p class="mb-2 text-[12px] leading-snug text-gc-text-muted">"Every field runs through BIR's own eBIRForms rules before anything is filed."</p>
                        <div class="mt-auto rounded-lg bg-gc-accent-soft px-3 py-2.5 text-gc-accent">
                            <div class="flex items-center gap-1.5 text-[13px] font-bold">
                                <span class="material-symbols-outlined text-[18px]">"verified"</span>"0 errors"
                            </div>
                            <div class="mt-0.5 text-[11px] opacity-80">"2,007 validations · 623 calculations"</div>
                        </div>
                    </div>
                    <div class=STEP>
                        <div class="mb-2 flex items-center gap-2"><span class=NUM>"3"</span><span class="text-[13px] font-bold text-gc-text">"File"</span></div>
                        <p class="mb-2 text-[12px] leading-snug text-gc-text-muted">"Approve it once. Buwiz files it and keeps BIR's confirmation with the return."</p>
                        <div class="mt-auto rounded-lg border border-gc-border bg-gc-bg-elevated px-3 py-2 text-[11px]">
                            <div class="flex items-center gap-1.5 font-semibold text-gc-text">
                                <span class="material-symbols-outlined text-[15px] text-gc-accent">"mark_email_read"</span>"Received by BIR"
                            </div>
                            <div class="mt-1 truncate font-mono text-gc-text-muted">"123456788001-1601Cv2018-102026.xml"</div>
                            <div class="text-gc-text-faint">"3 November 2026 · 10:43 AM"</div>
                        </div>
                    </div>
                    <div class=STEP>
                        <div class="mb-2 flex items-center gap-2"><span class=NUM>"4"</span><span class="text-[13px] font-bold text-gc-text">"Pay"</span></div>
                        <p class="mb-2 text-[12px] leading-snug text-gc-text-muted">"Pay online with the form, period and amount already filled in. You confirm."</p>
                        <div class="mt-auto rounded-lg bg-gc-text px-3 py-2.5 text-gc-bg">
                            <div class="text-[11px] opacity-70">"Paid to BIR · LandBank"</div>
                            <div class="text-[18px] font-black tabular-nums tracking-tight">"₱5,598.60"</div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

/// Keeps the gallery's dots and chips on the slide the rail has scrolled to, and scrolls it to
/// a slide when one is picked.
#[cfg(feature = "hydrate")]
mod rail {
    use leptos::{html, prelude::*};

    fn step() -> f64 {
        document()
            .get_element_by_id("feat-card-0")
            .map(|e| f64::from(e.client_width()) + 2. + 12.)
            .filter(|w| *w > 14.)
            .unwrap_or(1.)
    }

    pub fn current(rail: NodeRef<html::Div>, count: usize) -> Option<usize> {
        let el = rail.get_untracked()?;
        Some(((el.scroll_left() as f64 / step()).round().max(0.) as usize).min(count - 1))
    }

    pub fn go(rail: NodeRef<html::Div>, i: usize) {
        if let Some(el) = rail.get_untracked() {
            el.set_scroll_left((i as f64 * step()).round() as i32);
        }
    }
}

#[cfg(not(feature = "hydrate"))]
mod rail {
    use leptos::{html, prelude::*};

    pub fn current(_rail: NodeRef<html::Div>, _count: usize) -> Option<usize> {
        None
    }

    pub fn go(_rail: NodeRef<html::Div>, _i: usize) {}
}

/// The whole page body is one island: the hero tabs, feature picker, and FAQ
/// are client state and it needs no server data.
#[island]
pub fn Showcase() -> impl IntoView {
    let features = features();
    let (active, set_active) = signal(features[0].clone());
    let slide = RwSignal::new(0usize);
    let rail = NodeRef::<leptos::html::Div>::new();
    let count = features.len();
    let (open_faq, set_open_faq) = signal(Some("account"));

    view! {
        <div class="pb-24">
            <section id="hero" class="pt-14 md:pt-20 pb-16 px-4 sm:px-6 md:px-10 max-w-[1400px] mx-auto">
                <div class="text-center max-w-3xl mx-auto">
                    <span class="inline-flex items-center gap-1.5 rounded-full border border-gc-accent/30 bg-gc-accent-soft px-3 py-1 text-[11px] font-bold uppercase tracking-[0.16em] text-gc-accent mb-6 animate-rise">
                        <span class="material-symbols-outlined text-[15px] leading-none">"volunteer_activism"</span>
                        "Ease of Paying Taxes"
                    </span>
                    <h1 class="text-4xl sm:text-5xl md:text-6xl lg:text-7xl font-black tracking-tighter text-gc-text leading-[1.02] mb-6 animate-rise animate-rise-d1">
                        "Filing and paying taxes, "<span class="text-gc-accent">"finally easy."</span>
                    </h1>
                    <p class="text-base md:text-lg text-gc-text-muted leading-relaxed max-w-[60ch] mx-auto mb-9 animate-rise animate-rise-d2">
                        "Buwiz turns the records you already keep into finished BIR returns, checks each one against the official rules, reminds you before every deadline, then helps you file and pay online. One TIN, every branch under it, all 43 forms."
                    </p>
                    <div id="download" class="flex flex-wrap items-center justify-center gap-3 animate-rise animate-rise-d3">
                        <DownloadButton />
                        <a href="#agent" class="h-11 px-5 rounded-lg border border-gc-border-strong bg-gc-bg-elevated text-gc-text font-semibold text-sm inline-flex items-center justify-center gap-2 hover:bg-gc-bg-hover transition-colors whitespace-nowrap">
                            <span class="material-symbols-outlined text-[18px] leading-none">"play_circle"</span>
                            "Watch it file and pay"
                        </a>
                    </div>
                    <p class="mt-4 text-xs text-gc-text-faint animate-rise animate-rise-d3">"Free for one TIN · Windows, macOS and Linux"</p>
                </div>

                <div class="max-w-5xl mx-auto mt-12 md:mt-16 animate-rise animate-rise-d3">
                    {hero_journey()}
                </div>

                <div class="mt-10 md:mt-12 flex flex-wrap items-center justify-center gap-x-7 gap-y-3">
                    {[
                        ("devices", "Windows · macOS · Linux"),
                        ("description", "All 43 BIR forms"),
                        ("cloud_off", "Works offline after sign-in"),
                        ("account_tree", "Every branch under one TIN"),
                        ("approval", "You approve every filing and payment"),
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
                <div class="mb-8 max-w-[65ch] lg:mb-12">
                    <span class="mb-4 inline-flex items-center gap-1.5 text-[11px] font-bold uppercase tracking-[0.18em] text-gc-accent">
                        <span class="material-symbols-outlined text-[15px] leading-none">"photo_library"</span>
                        {format!("Tour the app · {count} screens")}
                    </span>
                    <h2 class=SECTION_TITLE>"Everything you file, in one calm window."</h2>
                    <p class=SECTION_LEAD>
                        "Built around how business owners, bookkeepers, and CPAs actually file: pick the taxpayer, open the form, fill only what changed."
                    </p>
                </div>

                <div class="lg:hidden">
                    <div class="no-scrollbar -mx-4 flex gap-2 overflow-x-auto px-4 pb-1 sm:-mx-6 sm:px-6 md:-mx-10 md:px-10" role="tablist" aria-label="App screens">
                        {features.iter().enumerate().map(|(i, feature)| {
                            let on = move || slide.get() == i;
                            view! {
                                <button
                                    type="button"
                                    role="tab"
                                    aria-selected=move || on().to_string()
                                    on:click=move |_| { slide.set(i); rail::go(rail, i); }
                                    class=move || format!(
                                        "inline-flex shrink-0 items-center gap-1.5 whitespace-nowrap rounded-full border px-3 py-1.5 text-xs font-semibold transition-colors cursor-pointer {}",
                                        if on() { "border-gc-accent bg-gc-accent-soft text-gc-accent" } else { "border-gc-border bg-gc-bg-card text-gc-text-muted" }
                                    )
                                >
                                    <span class="material-symbols-outlined text-[16px] leading-none">{feature.icon}</span>
                                    {feature.title}
                                </button>
                            }
                        }).collect_view()}
                    </div>

                    <div
                        node_ref=rail
                        on:scroll=move |_| if let Some(i) = rail::current(rail, count) { if slide.get_untracked() != i { slide.set(i) } }
                        class="no-scrollbar -mx-4 mt-4 flex snap-x snap-mandatory gap-3 overflow-x-auto scroll-smooth px-[8%] pb-2 sm:-mx-6 sm:px-[15%] md:-mx-10 md:px-[20%]"
                    >
                        {features.iter().enumerate().map(|(i, feature)| {
                            let (title, pair) = (feature.title, (feature.dark_src, feature.light_src));
                            view! {
                                <article
                                    id=format!("feat-card-{i}")
                                    class=move || format!(
                                        "w-[84%] shrink-0 snap-center overflow-hidden rounded-2xl border bg-gc-bg-elevated shadow-lg transition-[opacity,transform] duration-300 sm:w-[70%] md:w-[60%] {}",
                                        if slide.get() == i { "border-gc-accent/50 opacity-100" } else { "border-gc-border opacity-60 scale-[0.97]" }
                                    )
                                >
                                    <div class="relative aspect-[1440/1129] w-full overflow-hidden border-b border-gc-border bg-gc-bg">
                                        {themed_screenshot(move |dark| if dark { pair.0 } else { pair.1 }, move || title, false)}
                                        <span class="absolute left-2.5 top-2.5 rounded-full bg-black/65 px-2 py-0.5 text-[10.5px] font-semibold tabular-nums text-white">
                                            {format!("{} / {count}", i + 1)}
                                        </span>
                                    </div>
                                    <div class="p-4">
                                        <div class="mb-1.5 flex items-center gap-2">
                                            <span class="material-symbols-outlined text-[18px] text-gc-accent">{feature.icon}</span>
                                            <h3 class="min-w-0 flex-1 truncate text-sm font-bold tracking-tight text-gc-text">{feature.title}</h3>
                                            <span class="shrink-0 rounded-full bg-gc-accent-soft px-2 py-0.5 text-[10px] font-semibold text-gc-accent">{feature.badge}</span>
                                        </div>
                                        <p class="text-xs leading-relaxed text-gc-text-muted">{feature.description}</p>
                                        <p class="mt-2 text-xs leading-relaxed text-gc-text-faint">{feature.detail}</p>
                                    </div>
                                </article>
                            }
                        }).collect_view()}
                    </div>

                    <div class="mt-3 flex items-center justify-between gap-3">
                        <div class="flex items-center gap-1.5">
                            {(0..count).map(|i| view! {
                                <button
                                    type="button"
                                    aria-label=format!("Screen {}", i + 1)
                                    on:click=move |_| { slide.set(i); rail::go(rail, i); }
                                    class=move || if slide.get() == i {
                                        "h-2 w-6 rounded-full bg-gc-accent transition-all cursor-pointer"
                                    } else {
                                        "h-2 w-2 rounded-full bg-gc-border-strong transition-all cursor-pointer"
                                    }
                                ></button>
                            }).collect_view()}
                        </div>
                        <div class="flex items-center gap-2">
                            <span class="mr-1 hidden text-xs text-gc-text-faint sm:inline">"Swipe to browse"</span>
                            <button
                                type="button"
                                aria-label="Previous screen"
                                on:click=move |_| { let i = slide.get_untracked().saturating_sub(1); slide.set(i); rail::go(rail, i); }
                                class="grid h-9 w-9 place-items-center rounded-full border border-gc-border bg-gc-bg-elevated text-gc-text disabled:opacity-40 cursor-pointer"
                                disabled=move || slide.get() == 0
                            >
                                <span class="material-symbols-outlined text-[20px]">"chevron_left"</span>
                            </button>
                            <button
                                type="button"
                                aria-label="Next screen"
                                on:click=move |_| { let i = (slide.get_untracked() + 1).min(count - 1); slide.set(i); rail::go(rail, i); }
                                class="grid h-9 w-9 place-items-center rounded-full bg-gc-accent text-white disabled:opacity-40 cursor-pointer"
                                disabled=move || slide.get() == count - 1
                            >
                                <span class="material-symbols-outlined text-[20px]">"chevron_right"</span>
                            </button>
                        </div>
                    </div>
                </div>

                <div class="hidden lg:grid lg:grid-cols-12 gap-8 items-start">
                    <div class="lg:col-span-5 flex flex-col gap-2.5">
                        {features
                            .into_iter()
                            .map(|feature| {
                                let id = feature.id;
                                let pick = feature.clone();
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
                <div class="mb-12 max-w-[68ch]">
                    <span class="inline-block text-[11px] font-bold uppercase tracking-[0.18em] text-gc-accent mb-4">"AI CO-WORKER"</span>
                    <h2 class=SECTION_TITLE>"AI Native Tax Filing"</h2>
                    <p class=SECTION_LEAD>
                        "Give the paperwork to an AI Co-worker. It installs Buwiz from the chat, reads your tax profiles, turns your accountant's spreadsheet into a finished BIR form, files it when you approve, watches for BIR's confirmation and pays online. You stay in charge: every step that files or pays waits for your OK."
                    </p>
                </div>
                <div class="flex flex-col gap-20">
                    {DEMOS.iter().enumerate().map(|(i, &(eyebrow, title, lead))| view! {
                        <div>
                            <div class="mb-6 max-w-[68ch]">
                                <span class="inline-block text-[11px] font-bold uppercase tracking-[0.18em] text-gc-accent mb-3">
                                    {format!("{:02} · {eyebrow}", i + 1)}
                                </span>
                                <h3 class="text-2xl md:text-3xl font-black tracking-tight text-gc-text mb-2">{title}</h3>
                                <p class=SECTION_LEAD>{lead}</p>
                            </div>
                            {agent_demo(i)}
                        </div>
                    }).collect_view()}
                </div>
                <div class="mt-12 grid grid-cols-1 md:grid-cols-3 gap-3">
                    {[
                        ("approval", "Asks before it acts", "Filing and paying always wait for your approval, every time."),
                        ("key", "You choose what it may do", "Approve the co-worker on your Buwiz account: which profiles, and which actions."),
                        ("link_off", "End it anytime", "Remove the co-worker under Tax profiles, Connected agents, and its access stops."),
                    ]
                        .into_iter()
                        .map(|(icon, title, body)| view! {
                            <div class="flex items-start gap-3.5 p-4 rounded-lg bg-gc-bg-elevated border border-gc-border">
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
                                    "AI Co-worker: prepare, file and pay with your approval",
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
                            <DownloadButton />
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
