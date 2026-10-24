use super::downloads::{DownloadButtons, RELEASES_URL};
use super::{themed_screenshot, AccountActions};
use leptos::prelude::*;
use leptos_meta::*;

#[derive(Clone, PartialEq, Debug)]
struct WorkspaceScreen {
    id: &'static str,
    title: &'static str,
    badge: &'static str,
    description: &'static str,
    detail: &'static str,
    icon: &'static str,
    dark_src: &'static str,
    light_src: &'static str,
}

#[derive(Clone, PartialEq, Debug)]
struct McpStep {
    id: &'static str,
    title: &'static str,
    tool_name: &'static str,
    summary: &'static str,
    request_json: &'static str,
    response_json: &'static str,
}

#[derive(Clone, PartialEq, Debug)]
struct FaqItem {
    id: &'static str,
    question: &'static str,
    answer: &'static str,
}

const FIRM_SALES_MAILTO: &str =
    "mailto:support@goldcoders.dev?subject=Buwiz%20Multi-Profile%20%26%20Firm%20Plan";

/// The whole page body is one island: its four pickers are client state and
/// it needs no server data.
#[island]
pub fn Showcase() -> impl IntoView {

    let (hero_tab, set_hero_tab) = signal(0usize);

    let screens = vec![
        WorkspaceScreen {
            id: "side_by_side",
            title: "Real-Time Side-by-Side Form Preview",
            badge: "Live Sync",
            description: "Type into the structured on-screen form on the left and watch the official BIR paper sheet update live on the right.",
            detail: "Every edit streams in memory to the in-app Print Preview window within 120ms. Blitz re-renders only dirty sheets so your scroll position stays put, while a background WebKit worker prepares a vector A4 PDF for one-click printing.",
            icon: "splitscreen",
            dark_src: "/assets/images/shell_side_by_side_dark.webp",
            light_src: "/assets/images/shell_side_by_side_light.webp",
        },
        WorkspaceScreen {
            id: "templates_guided",
            title: "Form Templates & Guided Mode (Tab)",
            badge: "Keyboard Flow",
            description: "Keep one template per form per branch across tax years, then press Tab in Guided Mode to jump straight through marked fields.",
            detail: "Create a template from any filed return with 'Use as template' or from the Templates tab. Templates keep setup answers (ATCs, tax elections, categories) and never carry over stale amounts. When Guided Mode is on, pressing Tab stops only at marked fields, empty required fields, and Save draft.",
            icon: "keyboard_tab",
            dark_src: "/assets/images/shell_templates_dark.webp",
            light_src: "/assets/images/shell_templates_light.webp",
        },
        WorkspaceScreen {
            id: "validation_parity",
            title: "100% BIR Validation Parity",
            badge: "43 Forms",
            description: "All 43 BIR forms, 9,592 fields, 2,007 validations, and 623 calculations extracted from the official rule corpus.",
            detail: "Validation and computation run asynchronously off the UI thread 300ms after your last keystroke. Every error message binds directly to its numbered form item in a reserved status line so layout never shifts while you type.",
            icon: "verified",
            dark_src: "/assets/images/shell_form_2550q_dark.webp",
            light_src: "/assets/images/shell_form_2550q_light.webp",
        },
        WorkspaceScreen {
            id: "taxpayer_branches",
            title: "Taxpayer & Multi-Branch Hierarchy",
            badge: "Branches",
            description: "Group Head Office (00000) and branch codes under a single 9-digit TIN with automatic address and RDO inheritance.",
            detail: "The centered title-bar Context Pill opens the Tax Profile drawer to switch branches or taxpayers in place. Branches inherit Head Office address, ZIP, phone, email, and RDO by default until you flip 'Same as head office' off for a specific field.",
            icon: "account_tree",
            dark_src: "/assets/images/shell_panel_profile_dark.webp",
            light_src: "/assets/images/shell_panel_profile_light.webp",
        },
        WorkspaceScreen {
            id: "starter_packs",
            title: "Per-Branch Tax Years & Starter Packs",
            badge: "Setup",
            description: "Add tax years from 2010 to next year, copy prior-year form sets in one click, or apply curated starter packs.",
            detail: "Choose Non-VAT business, VAT business, Employer, or Professional starter packs to populate the exact returns a branch files. Read mode hides the rest of the catalog so your workspace only shows active obligations.",
            icon: "inventory_2",
            dark_src: "/assets/images/shell_forms_edit_dark.webp",
            light_src: "/assets/images/shell_forms_edit_light.webp",
        },
        WorkspaceScreen {
            id: "palette_library",
            title: "Command Palette & 43-Form Library",
            badge: "Cmd+K",
            description: "Jump between taxpayers, branches, recent returns, and all 43 BIR forms from the keyboard.",
            detail: "Press Cmd+K on macOS or Ctrl+K on Windows to search by TIN, taxpayer name, form code (such as 1601C, 2550Q, 0619E, 1701, or 1702RT), or app screen without leaving the keyboard.",
            icon: "terminal",
            dark_src: "/assets/images/shell_palette_dark.webp",
            light_src: "/assets/images/shell_palette_light.webp",
        },
        WorkspaceScreen {
            id: "calendar_deadlines",
            title: "Home Feed & 30-Day BIR Calendar",
            badge: "Deadlines",
            description: "See every return due in the next 30 days across all taxpayers and branches you manage.",
            detail: "Home surfaces overdue and upcoming deadlines with one-click Open actions, plus recently filed returns ready to be turned into reusable templates.",
            icon: "event_upcoming",
            dark_src: "/assets/images/shell_calendar_dark.webp",
            light_src: "/assets/images/shell_calendar_light.webp",
        },
        WorkspaceScreen {
            id: "security_settings",
            title: "Per-Taxpayer PINs, TOTP & Inbox Sync",
            badge: "Security",
            description: "Lock individual taxpayer accounts with a PIN or TOTP authenticator, with solo or help-desk admin recovery.",
            detail: "First launch adapts to solo business owners or multi-client help desks (admin TIN 000-000-000). Background tasks auto-retry queued submissions and watch your mailbox for official BIR confirmation receipts.",
            icon: "lock",
            dark_src: "/assets/images/shell_settings_dark.webp",
            light_src: "/assets/images/shell_settings_light.webp",
        },
    ];

    let mcp_steps = vec![
        McpStep {
            id: "open",
            title: "1. Connect & Open Draft from Excel, ERP, or Buwiz Books",
            tool_name: "bir_form_open",
            summary: "Your AI agent reads a payroll Excel sheet, ERP export, or Buwiz Books ledger, connects to your running Buwiz app, and opens the target return prefilled from the taxpayer profile.",
            request_json: r#"{
  "tool": "bir_form_open",
  "arguments": {
    "tin": "123-456-788-00000",
    "form_code": "0619E",
    "year": 2026,
    "period": 8
  }
}"#,
            response_json: r#"{
  "form_code": "0619E",
  "status": "Draft",
  "editable": true,
  "profile_prefilled": [
    "frm0619E:txtTIN1",
    "frm0619E:txtTaxpayerName",
    "frm0619E:txtRDOCode"
  ]
}"#,
        },
        McpStep {
            id: "fill",
            title: "2. Map Spreadsheet & Ledger Figures to Official Keys",
            tool_name: "bir_form_fill",
            summary: "The agent inspects fillable box definitions via bir_form_fields and writes derived values with source attribution. Boxes typed directly by the user are never overwritten.",
            request_json: r#"{
  "tool": "bir_form_fill",
  "arguments": {
    "source": "document",
    "fields": {
      "frm0619E:optWithheld": "Y",
      "frm0619E:optCategory": "P",
      "frm0619E:txtTax14": "48500.00"
    }
  }
}"#,
            response_json: r#"{
  "updated_boxes": 3,
  "computed": {
    "frm0619E:txtTax16": "48500.00",
    "frm0619E:txtTax18": "48500.00"
  },
  "kept_user_boxes": []
}"#,
        },
        McpStep {
            id: "validate",
            title: "3. Verify Against Official BIR Rules",
            tool_name: "bir_form_validate",
            summary: "Before presenting the return, the agent runs the exact same BIR rule engine used by the desktop Check button and resolves any missing required fields via bir_form_needs_you.",
            request_json: r#"{
  "tool": "bir_form_validate",
  "arguments": {}
}"#,
            response_json: r#"{
  "ok": true,
  "error_count": 0,
  "field_errors": [],
  "needs_you": []
}"#,
        },
        McpStep {
            id: "review",
            title: "4. Live Official Sheet Preview & Human Approval",
            tool_name: "bir_form_review",
            summary: "Draft-only by construction: bir-mcp has no submit, queue, or payment tool. It renders the official BIR paper preview and highlights the Submit button in the app for your own click.",
            request_json: r#"{
  "tool": "bir_form_review",
  "arguments": {}
}"#,
            response_json: r#"{
  "ready_for_user_submit": true,
  "preview_window": "focused",
  "note": "User must click Queue / Submit in Buwiz."
}"#,
        },
    ];

    let faqs = vec![
        FaqItem {
            id: "parity",
            question: "What does 100% validation parity with BIR mean?",
            answer: "We extracted the complete rule corpus from the official eBIRForms package across all 43 forms: 9,592 field definitions, 2,007 validation rules, and 623 formula calculations. Buwiz runs these exact rules and serializes compliant XML payloads in native Rust without legacy Java or 32-bit Windows dependencies.",
        },
        FaqItem {
            id: "side_by_side",
            question: "How does the real-time side-by-side form preview work?",
            answer: "When you open any return and click Print preview, Buwiz opens the official A4 BIR form sheet beside your on-screen form editor. As you type or change dropdowns, changes sync within 120ms to the Blitz renderer, repainting only modified sheets while generating a print-ready vector PDF in the background.",
        },
        FaqItem {
            id: "templates_guided",
            question: "How do Form Templates and Guided Mode (Tab) speed up monthly filing?",
            answer: "Each branch keeps one template per form that persists across tax years. A template saves your setup answers (ATCs, withholding category, tax regime elections) and never keeps prior-period peso amounts. You can also mark specific fields as Tab stops: when Guided Mode is switched on, pressing Tab steps directly through your marked fields and empty required boxes straight to Save draft.",
        },
        FaqItem {
            id: "mcp_safety",
            question: "Can an AI agent submit a tax return without my permission via the MCP server?",
            answer: "No. The bir-mcp server is draft-only by construction and enforces a unit-tested boundary that excludes submit, queue, file, and payment operations. An AI agent can ingest Excel sheets, ERP exports, or Buwiz Books data, fill the draft, and validate it, but bir_form_review only brings the validated return to the foreground for your physical click.",
        },
        FaqItem {
            id: "pricing_model",
            question: "When is Buwiz free, and when do I need a paid plan?",
            answer: "Buwiz eBIRForms is 100% free for personal use or a single tax profile (one 9-digit TIN). You get all 43 forms, live side-by-side preview, templates, Guided Mode, and the local MCP server at zero cost. A paid plan is only required when managing multiple tax profiles or multiple businesses, or when operating as an accounting firm managing shared client profiles.",
        },
        FaqItem {
            id: "egov_firms",
            question: "How do Buwiz accounts, TIN claims, and firm sharing work?",
            answer: "Create a free Buwiz account and verify your email, then add your tax profile or claim a TIN another workspace already holds by uploading your BIR Form 2303. Each TIN and branch has exactly one holder, and every claim is reviewed before ownership moves. The desktop app signs in to the same account, and you can grant an accounting firm or bookkeeper access to manage your forms and filings while you keep ownership. eGovPH identity verification is planned as a faster way to prove TIN ownership.",
        },
    ];

    let (active_screen, set_active_screen) = signal(screens[0].clone());
    let (active_mcp, set_active_mcp) = signal(mcp_steps[0].clone());
    let (open_faq, set_open_faq) = signal(Some("parity"));

    view! {

        <div class="pb-24">
            // ================================================================
            // SECTION 1: Centered Launch Hero — headline + large real screenshot
            // ================================================================
            <section id="hero" class="pt-14 md:pt-20 pb-16 px-4 sm:px-6 md:px-10 max-w-[1400px] mx-auto">
                <div class="text-center max-w-3xl mx-auto">
                    <span class="inline-block text-[11px] font-bold uppercase tracking-[0.18em] text-gc-accent mb-5 animate-rise">
                        "THE MODERN ALTERNATIVE TO EBIRFORMS"
                    </span>

                    <h1 class="text-4xl sm:text-5xl md:text-6xl lg:text-7xl font-black tracking-tighter text-gc-text leading-[1.02] mb-6 animate-rise animate-rise-d1">
                        "Philippine tax filing. Finally modern."
                    </h1>

                    <p class="text-base md:text-lg text-gc-text-muted leading-relaxed max-w-[58ch] mx-auto mb-9 animate-rise animate-rise-d2">
                        "Meet Buwiz Forms. A faster, free, AI-ready alternative to eBIRForms. Native on Windows, macOS, and Linux, with BIR-matched form validation."
                    </p>

                    <div id="download" class="flex flex-wrap items-center justify-center gap-3 animate-rise animate-rise-d3">
                        <DownloadButtons />
                        <a
                            href="#workspace"
                            class="h-11 px-6 rounded-lg bg-gc-bg-elevated border border-gc-border-strong text-gc-text font-semibold text-sm inline-flex items-center justify-center gap-2 hover:bg-gc-bg-hover hover:-translate-y-[1px] active:scale-[0.98] transition-all whitespace-nowrap"
                        >
                            <span>"See It in Action"</span>
                            <span class="material-symbols-outlined text-[18px] leading-none">"arrow_downward"</span>
                        </a>
                    </div>
                    <p class="mt-4 text-[11px] text-gc-text-faint animate-rise animate-rise-d3">
                        "Always the latest release — Windows Setup.exe · universal macOS DMG · Linux .deb. "
                        <a
                            href=RELEASES_URL
                            target="_blank"
                            rel="noopener noreferrer"
                            class="underline underline-offset-2 hover:text-gc-text-muted transition-colors"
                        >
                            "All packages & release notes"
                        </a>
                    </p>
                    <p class="mt-2 text-[11px] text-gc-text-faint animate-rise animate-rise-d3">
                        "Have a Buwiz account? "
                        <a href="/login" class="underline underline-offset-2 hover:text-gc-text-muted transition-colors">"Sign in"</a>
                        " to manage your tax profiles on the web."
                    </p>
                </div>

                // Large real bir-shell screenshot with functional view switcher
                <div class="max-w-5xl mx-auto mt-12 md:mt-16 animate-rise animate-rise-d3">
                    <div class="rounded-xl border border-gc-border bg-gc-bg-elevated shadow-2xl overflow-hidden">
                        <div class="px-4 py-2.5 bg-gc-bg-card border-b border-gc-border flex flex-wrap items-center justify-between gap-3">
                            <div class="flex items-center gap-2">
                                <span class="text-xs font-semibold text-gc-text">"bir-shell"</span>
                                <span class="text-[11px] text-gc-text-faint tabular-nums">"123-456-788-00000"</span>
                            </div>

                            <div class="inline-flex items-center gap-1 p-0.5 rounded-lg bg-gc-bg-hover border border-gc-border max-w-full overflow-x-auto" role="tablist" aria-label="Hero Preview Mode">
                                {[
                                    (0usize, "Side-by-Side Live Form"),
                                    (1usize, "Guided Form & Templates"),
                                    (2usize, "Home & Deadlines"),
                                ]
                                .into_iter()
                                .map(|(idx, label)| {
                                    let active = move || hero_tab.get() == idx;
                                    view! {
                                        <button
                                            type="button"
                                            role="tab"
                                            aria-selected=move || active().to_string()
                                            on:click=move |_| set_hero_tab.set(idx)
                                            class=move || format!(
                                                "px-2.5 py-1 rounded-md text-[11px] font-semibold transition-colors cursor-pointer whitespace-nowrap {}",
                                                if active() {
                                                    "bg-gc-bg-elevated text-gc-text shadow-xs"
                                                } else {
                                                    "text-gc-text-muted hover:text-gc-text"
                                                }
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
                                || "Buwiz Forms desktop app showing a BIR form editor beside its live official-sheet preview",
                                true,
                            )}
                        </div>
                    </div>
                </div>

                // Proof strip — verified product facts only
                <div class="mt-10 md:mt-12 flex flex-wrap items-center justify-center gap-x-7 gap-y-3">
                    {[
                        ("devices", "Windows · macOS · Linux"),
                        ("description", "43-form BIR library"),
                        ("terminal", "MCP server for AI agents"),
                        ("lock", "Local-first encrypted storage"),
                        ("money_off", "Free for one tax profile"),
                    ]
                    .into_iter()
                    .map(|(icon, label)| {
                        view! {
                            <span class="inline-flex items-center gap-2 text-xs font-medium text-gc-text-muted">
                                <span class="material-symbols-outlined text-[15px] text-gc-accent leading-none">{icon}</span>
                                {label}
                            </span>
                        }
                    })
                    .collect_view()}
                </div>
            </section>

            // ================================================================
            // SECTION 2: Interactive bir-shell Workspace Explorer
            // Layout Family: Sticky Master-Detail Interactive Explorer
            // ================================================================
            <section id="workspace" class="py-20 px-4 sm:px-6 md:px-10 max-w-[1400px] mx-auto border-t border-gc-border">
                <div class="mb-12 max-w-[65ch]">
                    <h2 class="text-3xl md:text-4xl font-black tracking-tighter text-gc-text mb-3">
                        "A calm, keyboard-first tax shell."
                    </h2>
                    <p class="text-base text-gc-text-muted leading-relaxed">
                        "Redesigned around how bookkeepers, business owners, and CPAs actually work: one context pill, one unified form renderer, and zero visual clutter."
                    </p>
                </div>

                <div class="grid grid-cols-1 lg:grid-cols-12 gap-8 items-start">
                    // Left Rail: 8 Interactive Capabilities
                    <div class="lg:col-span-5 flex flex-col gap-2.5">
                        {screens
                            .clone()
                            .into_iter()
                            .map(|screen| {
                                let s_check = screen.clone();
                                let s_click = screen.clone();
                                let s_mobile = screen.clone();
                                let is_selected = move || active_screen.read().id == s_check.id;

                                view! {
                                    <div
                                        on:click=move |_| set_active_screen.set(s_click.clone())
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
                                                if is_selected() {
                                                    "bg-gc-accent-soft text-gc-accent"
                                                } else {
                                                    "bg-gc-bg-hover text-gc-text-muted"
                                                }
                                            )>
                                                <span class="material-symbols-outlined text-[18px]">{screen.icon}</span>
                                            </div>
                                            <div class="flex-1 min-w-0">
                                                <div class="flex items-center justify-between gap-2 mb-1">
                                                    <h3 class="text-sm font-bold tracking-tight text-gc-text truncate">
                                                        {screen.title}
                                                    </h3>
                                                    <span class=move || format!(
                                                        "text-[10px] font-semibold px-2 py-0.5 rounded-full shrink-0 {}",
                                                        if is_selected() {
                                                            "bg-gc-accent-soft text-gc-accent"
                                                        } else {
                                                            "bg-gc-bg-hover text-gc-text-faint"
                                                        }
                                                    )>
                                                        {screen.badge}
                                                    </span>
                                                </div>
                                                <p class="text-xs text-gc-text-muted leading-relaxed">
                                                    {screen.description}
                                                </p>
                                            </div>
                                        </div>

                                        // Mobile fallback: inline screenshot when selected
                                        <div class=move || if is_selected() { "block lg:hidden mt-4 pt-3 border-t border-gc-border" } else { "hidden" }>
                                            <div class="rounded-lg overflow-hidden border border-gc-border bg-gc-bg aspect-[1440/1129]">
                                                {
                                                    let shot = s_mobile.clone();
                                                    let title = s_mobile.title;
                                                    themed_screenshot(move |dark| if dark { shot.dark_src } else { shot.light_src }, move || title, false)
                                                }
                                            </div>
                                            <p class="text-xs text-gc-text-muted leading-relaxed mt-3">
                                                {s_mobile.detail}
                                            </p>
                                        </div>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>

                    // Right Stage: Sticky Desktop Screenshot & Deep Technical Note
                    <div class="hidden lg:block lg:col-span-7 sticky top-20">
                        <div class="rounded-xl border border-gc-border bg-gc-bg-elevated shadow-xl overflow-hidden">
                            <div class="px-5 py-3 bg-gc-bg-card border-b border-gc-border flex items-center justify-between">
                                <div class="flex items-center gap-2.5">
                                    <span class="material-symbols-outlined text-[16px] text-gc-accent">
                                        {move || active_screen.read().icon}
                                    </span>
                                    <span class="text-xs font-bold text-gc-text">
                                        {move || active_screen.read().title}
                                    </span>
                                </div>
                                <span class="text-[11px] font-semibold px-2.5 py-0.5 rounded-full bg-gc-accent-soft text-gc-accent">
                                    {move || active_screen.read().badge}
                                </span>
                            </div>

                            <div class="relative aspect-[1440/1129] w-full bg-gc-bg overflow-hidden">
                                {themed_screenshot(
                                    move |dark| { let cur = active_screen.read(); if dark { cur.dark_src } else { cur.light_src } },
                                    move || active_screen.read().title,
                                    false,
                                )}
                            </div>

                            <div class="p-5 bg-gc-bg-card border-t border-gc-border">
                                <p class="text-xs md:text-sm text-gc-text-muted leading-relaxed">
                                    {move || active_screen.read().detail}
                                </p>
                            </div>
                        </div>
                    </div>
                </div>
            </section>

            // ================================================================
            // SECTION 3: Asymmetric 4-Cell Bento Grid (Validation, Live Sync, Templates, Branches)
            // Layout Family: Asymmetric 7/5 + 5/7 Bento Grid with Visual & Surface Diversity
            // ================================================================
            <section id="validation" class="py-20 px-4 sm:px-6 md:px-10 max-w-[1400px] mx-auto border-t border-gc-border">
                <div class="mb-12 max-w-[65ch]">
                    <h2 class="text-3xl md:text-4xl font-black tracking-tighter text-gc-text mb-3">
                        "Every official rule. Zero legacy lag."
                    </h2>
                    <p class="text-base text-gc-text-muted leading-relaxed">
                        "We extracted the complete BIR rule corpus into a type-safe Rust engine paired with an instant A4 paper renderer."
                    </p>
                </div>

                <div class="grid grid-cols-1 lg:grid-cols-12 gap-6">
                    // Cell 1 (7 cols): Side-by-Side Live Print Sync with real visual
                    <div class="lg:col-span-7 rounded-xl border border-gc-border bg-gc-bg-elevated p-6 md:p-8 flex flex-col justify-between gap-6">
                        <div>
                            <div class="flex items-center justify-between gap-3 mb-3">
                                <span class="text-xs font-bold text-gc-accent">"Real-Time Side-by-Side Forms"</span>
                                <span class="text-[11px] font-semibold px-2.5 py-0.5 rounded-full bg-gc-accent-soft text-gc-accent tabular-nums">"120ms Live Sync"</span>
                            </div>
                            <h3 class="text-xl md:text-2xl font-bold tracking-tight text-gc-text mb-2">
                                "Edit structured fields on the left. Watch the official BIR form update on the right."
                            </h3>
                            <p class="text-sm text-gc-text-muted leading-relaxed max-w-[62ch]">
                                "No more exporting blind PDFs to check if an ATC or schedule row landed in the right comb box. The in-app Print Preview window mirrors your open draft in real time, re-rendering only changed pages while keeping a vector A4 PDF ready to print or share."
                            </p>
                        </div>

                        <div class="rounded-lg border border-gc-border overflow-hidden bg-gc-bg aspect-[1440/1129]">
                            {themed_screenshot(|dark| if dark { "/assets/images/shell_side_by_side_dark.webp" } else { "/assets/images/shell_side_by_side_light.webp" }, || "Side-by-side view of Buwiz on-screen form editor and live official BIR Form 0619-E sheet", false)}
                        </div>
                    </div>

                    // Cell 2 (5 cols, Tinted Accent Surface): 100% BIR Validation Parity
                    <div class="lg:col-span-5 rounded-xl border border-gc-accent/30 bg-gc-accent-soft p-6 md:p-8 flex flex-col justify-between gap-6">
                        <div>
                            <div class="flex items-center justify-between gap-3 mb-3">
                                <span class="text-xs font-bold text-gc-accent">"100% Validation Parity"</span>
                                <span class="text-[11px] font-semibold px-2.5 py-0.5 rounded-full bg-gc-bg-elevated text-gc-text tabular-nums">"300ms Async Check"</span>
                            </div>
                            <h3 class="text-xl md:text-2xl font-bold tracking-tight text-gc-text mb-2">
                                "Exact BIR rules, without the 20-second UI freeze."
                            </h3>
                            <p class="text-sm text-gc-text-muted leading-relaxed">
                                "Every control binds by its official rules-corpus key. Opening and validating run off the main thread, so even multi-schedule returns like 2550Q and 1701 validate smoothly as you type."
                            </p>
                        </div>

                        <div class="grid grid-cols-2 gap-3">
                            <div class="rounded-lg bg-gc-bg-elevated border border-gc-border p-4">
                                <div class="text-2xl md:text-3xl font-black tracking-tight text-gc-text tabular-nums">"43"</div>
                                <div class="text-xs font-semibold text-gc-text-muted mt-1">"Official BIR forms"</div>
                            </div>
                            <div class="rounded-lg bg-gc-bg-elevated border border-gc-border p-4">
                                <div class="text-2xl md:text-3xl font-black tracking-tight text-gc-text tabular-nums">"9,592"</div>
                                <div class="text-xs font-semibold text-gc-text-muted mt-1">"Bound form fields"</div>
                            </div>
                            <div class="rounded-lg bg-gc-bg-elevated border border-gc-border p-4">
                                <div class="text-2xl md:text-3xl font-black tracking-tight text-gc-text tabular-nums">"2,007"</div>
                                <div class="text-xs font-semibold text-gc-text-muted mt-1">"Validation checks"</div>
                            </div>
                            <div class="rounded-lg bg-gc-bg-elevated border border-gc-border p-4">
                                <div class="text-2xl md:text-3xl font-black tracking-tight text-gc-text tabular-nums">"623"</div>
                                <div class="text-xs font-semibold text-gc-text-muted mt-1">"Auto-calculations"</div>
                            </div>
                        </div>

                        <div class="rounded-lg bg-gc-bg-elevated border border-gc-border p-4 space-y-2">
                            <div class="text-xs font-bold text-gc-text">"Part Navigator Attention Badges"</div>
                            <p class="text-xs text-gc-text-muted leading-relaxed">
                                "The sticky 'On this form' sidebar counts items per Part and highlights validation errors in red, with a one-click jump to the next Part that needs attention."
                            </p>
                        </div>
                    </div>

                    // Cell 3 (5 cols): Form Templates & Guided Mode (Tab)
                    <div class="lg:col-span-5 rounded-xl border border-gc-border bg-gc-bg-elevated p-6 md:p-8 flex flex-col justify-between gap-6">
                        <div>
                            <div class="flex items-center justify-between gap-3 mb-3">
                                <span class="text-xs font-bold text-gc-accent">"Templates & Guided Mode"</span>
                                <span class="text-[11px] font-semibold px-2.5 py-0.5 rounded-full bg-gc-accent-soft text-gc-accent">"Tab-Stop Filing"</span>
                            </div>
                            <h3 class="text-xl md:text-2xl font-bold tracking-tight text-gc-text mb-2">
                                "Keep setup answers. Press Tab to fill only what changes."
                            </h3>
                            <p class="text-sm text-gc-text-muted leading-relaxed">
                                "Each branch stores one template per form across tax years. Click any field in template mode to cycle Keep value, Mark for Guided Mode, or Off. Amounts can be marked as stops but are never carried over."
                            </p>
                        </div>

                        <div class="rounded-lg border border-gc-border overflow-hidden bg-gc-bg aspect-[1440/1129]">
                            {themed_screenshot(|dark| if dark { "/assets/images/shell_templates_dark.webp" } else { "/assets/images/shell_templates_light.webp" }, || "Templates tab in Buwiz bir-shell showing the saved template list for a branch", false)}
                        </div>

                        <div class="space-y-2.5">
                            <div class="rounded-lg border border-gc-border bg-gc-bg p-3.5 flex items-center justify-between gap-3">
                                <div class="flex items-center gap-3">
                                    <span class="px-2 py-0.5 rounded bg-gc-bg-hover border border-gc-border text-[11px] font-mono font-semibold text-gc-text">"Auto"</span>
                                    <span class="text-xs font-semibold text-gc-text">"TIN, RDO, Address, Period & Totals"</span>
                                </div>
                                <span class="text-[11px] text-gc-text-faint">"Locked from profile"</span>
                            </div>
                            <div class="rounded-lg border border-gc-border bg-gc-bg p-3.5 flex items-center justify-between gap-3">
                                <div class="flex items-center gap-3">
                                    <span class="px-2 py-0.5 rounded bg-gc-accent-soft text-[11px] font-mono font-semibold text-gc-accent">"Kept"</span>
                                    <span class="text-xs font-semibold text-gc-text">"ATC, Category, Tax Regime Elections"</span>
                                </div>
                                <span class="text-[11px] text-gc-accent font-medium">"Prefilled by template"</span>
                            </div>
                            <div class="rounded-lg border border-gc-accent/40 bg-gc-accent-soft p-3.5 flex items-center justify-between gap-3">
                                <div class="flex items-center gap-3">
                                    <span class="px-2 py-0.5 rounded bg-gc-bg-elevated border border-gc-border text-[11px] font-mono font-semibold text-gc-text">"Tab ⇥"</span>
                                    <span class="text-xs font-semibold text-gc-text">"Marked Amounts -> Empty Required -> Save"</span>
                                </div>
                                <span class="text-[11px] text-gc-text font-semibold">"Guided Mode stops"</span>
                            </div>
                        </div>
                    </div>

                    // Cell 4 (7 cols): Taxpayer -> Branch -> Yearly Forms with real screenshot
                    <div class="lg:col-span-7 rounded-xl border border-gc-border bg-gc-bg-elevated p-6 md:p-8 flex flex-col justify-between gap-6">
                        <div>
                            <div class="flex items-center justify-between gap-3 mb-3">
                                <span class="text-xs font-bold text-gc-accent">"Tax Profile Architecture"</span>
                                <span class="text-[11px] font-semibold px-2.5 py-0.5 rounded-full bg-gc-accent-soft text-gc-accent">"Head Office Inheritance"</span>
                            </div>
                            <h3 class="text-xl md:text-2xl font-bold tracking-tight text-gc-text mb-2">
                                "Taxpayer -> Branch -> Yearly Forms -> One Template per Form."
                            </h3>
                            <p class="text-sm text-gc-text-muted leading-relaxed max-w-[62ch]">
                                "No more uploading or waiting on manual COR reviews to unlock your forms. Set your 9-digit TIN and Head Office once, add branches that inherit details automatically, and pick yearly forms with Non-VAT, VAT, Employer, or Professional starter packs."
                            </p>
                        </div>

                        <div class="rounded-lg border border-gc-border overflow-hidden bg-gc-bg aspect-[1440/1129]">
                            {themed_screenshot(|dark| if dark { "/assets/images/shell_profile_dark.webp" } else { "/assets/images/shell_profile_light.webp" }, || "Taxpayer Profile screen in bir-shell showing shared Taxpayer details, Branch code 00000, and inherited address", false)}
                        </div>
                    </div>
                </div>
            </section>

            // ================================================================
            // SECTION 4: MCP Server - Agentic Filing from Excel, ERPs & Buwiz Books
            // Layout Family: 2-Column Architecture + Interactive JSON Tool Inspector
            // (Eyebrow #2 of 2 on page)
            // ================================================================
            <section id="mcp" class="py-20 px-4 sm:px-6 md:px-10 max-w-[1400px] mx-auto border-t border-gc-border">
                <div class="grid grid-cols-1 lg:grid-cols-12 gap-10 lg:gap-12 items-start">
                    // Left Column (5 cols): Narrative + Data Sources + Safety Boundary
                    <div class="lg:col-span-5 flex flex-col gap-6">
                        <div>
                            <span class="text-[11px] font-bold uppercase tracking-[0.16em] text-gc-accent block mb-3">
                                "MODEL CONTEXT PROTOCOL SERVER"
                            </span>
                            <h2 class="text-3xl md:text-4xl font-black tracking-tighter text-gc-text mb-3 leading-tight">
                                "Let your AI agent prepare the return."
                            </h2>
                            <p class="text-base text-gc-text-muted leading-relaxed">
                                "Buwiz ships with bir-mcp, a local MCP server for all 43 BIR forms. Point your agent at an Excel workbook, your ERP, or Buwiz Books, and let it fill, compute, and validate the draft for you."
                            </p>
                        </div>

                        <div class="space-y-3">
                            <div class="p-4 rounded-xl border border-gc-border bg-gc-bg-elevated flex items-start gap-3.5">
                                <span class="material-symbols-outlined text-gc-accent text-[20px] mt-0.5">"table_view"</span>
                                <div>
                                    <h3 class="text-sm font-bold text-gc-text mb-1">"Excel Spreadsheets & CSV Schedules"</h3>
                                    <p class="text-xs text-gc-text-muted leading-relaxed">
                                        "Hand your agent a monthly payroll sheet, withholding schedule, or quarterly sales summary. It maps every column to the exact official BIR box key."
                                    </p>
                                </div>
                            </div>

                            <div class="p-4 rounded-xl border border-gc-border bg-gc-bg-elevated flex items-start gap-3.5">
                                <span class="material-symbols-outlined text-gc-accent text-[20px] mt-0.5">"menu_book"</span>
                                <div>
                                    <h3 class="text-sm font-bold text-gc-text mb-1">"Buwiz Books & ERP Connectors"</h3>
                                    <p class="text-xs text-gc-text-muted leading-relaxed">
                                        "Connect directly to Buwiz Books, Taxman, QuickBooks, Xero, or Odoo so ledger balances flow into draft returns without manual re-encoding."
                                    </p>
                                </div>
                            </div>

                            <div class="p-4 rounded-xl border border-gc-accent/30 bg-gc-accent-soft flex items-start gap-3.5">
                                <span class="material-symbols-outlined text-gc-accent text-[20px] mt-0.5">"verified_user"</span>
                                <div>
                                    <h3 class="text-sm font-bold text-gc-text mb-1">"Draft-Only Human-in-the-Loop Guarantee"</h3>
                                    <p class="text-xs text-gc-text-muted leading-relaxed">
                                        "bir-mcp exposes zero submit, queue, or payment tools. Your agent prepares and validates the return, opens the official sheet preview, and highlights Submit for your own click."
                                    </p>
                                </div>
                            </div>
                        </div>
                    </div>

                    // Right Column (7 cols): Interactive bir-mcp Tool Pipeline Inspector
                    <div class="lg:col-span-7 w-full">
                        <div class="rounded-xl border border-gc-border bg-gc-bg-elevated shadow-xl overflow-hidden">
                            <div class="p-3 bg-gc-bg-card border-b border-gc-border flex flex-wrap gap-1.5" role="tablist" aria-label="MCP Agent Steps">
                                {mcp_steps
                                    .clone()
                                    .into_iter()
                                    .map(|step| {
                                        let s_check = step.clone();
                                        let s_click = step.clone();
                                        let active = move || active_mcp.read().id == s_check.id;

                                        view! {
                                            <button
                                                type="button"
                                                role="tab"
                                                aria-selected=move || active().to_string()
                                                on:click=move |_| set_active_mcp.set(s_click.clone())
                                                class=move || format!(
                                                    "px-3 py-2 rounded-lg text-xs font-semibold transition-colors cursor-pointer text-left {}",
                                                    if active() {
                                                        "bg-gc-accent-soft text-gc-accent border border-gc-accent/30"
                                                    } else {
                                                        "bg-gc-bg-hover/60 text-gc-text-muted hover:text-gc-text border border-transparent"
                                                    }
                                                )
                                            >
                                                <span class="font-mono">{step.tool_name}</span>
                                            </button>
                                        }
                                    })
                                    .collect_view()}
                            </div>

                            <div class="p-6 space-y-5">
                                <div>
                                    <div class="flex items-center justify-between gap-2 mb-1.5">
                                        <h3 class="text-base font-bold text-gc-text">
                                            {move || active_mcp.read().title}
                                        </h3>
                                        <span class="text-[11px] font-mono px-2.5 py-0.5 rounded-full bg-gc-bg-hover text-gc-accent">
                                            {move || active_mcp.read().tool_name}
                                        </span>
                                    </div>
                                    <p class="text-xs md:text-sm text-gc-text-muted leading-relaxed">
                                        {move || active_mcp.read().summary}
                                    </p>
                                </div>

                                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    <div class="rounded-lg border border-gc-border bg-gc-bg p-4 overflow-x-auto">
                                        <div class="text-[11px] font-mono font-semibold text-gc-text-faint mb-2">
                                            "Agent -> bir-mcp Request"
                                        </div>
                                        <pre class="text-xs font-mono text-gc-text leading-relaxed">
                                            <code>{move || active_mcp.read().request_json}</code>
                                        </pre>
                                    </div>

                                    <div class="rounded-lg border border-gc-border bg-gc-bg p-4 overflow-x-auto">
                                        <div class="text-[11px] font-mono font-semibold text-gc-accent mb-2">
                                            "bir-core Validated Response"
                                        </div>
                                        <pre class="text-xs font-mono text-gc-text leading-relaxed">
                                            <code>{move || active_mcp.read().response_json}</code>
                                        </pre>
                                    </div>
                                </div>

                                <div class="pt-2 flex flex-wrap items-center justify-between gap-2 text-xs text-gc-text-faint border-t border-gc-border">
                                    <span>"Available tools: bir_list_apps, bir_connect, bir_profiles_list, bir_dues_list, bir_form_open, bir_form_fields, bir_form_context, bir_form_fill, bir_form_needs_you, bir_form_validate, bir_form_preview, bir_form_review"</span>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </section>

            // ================================================================
            // SECTION 5: eGovPH TIN Identity & Firm Tax Profile Sharing
            // Layout Family: 2-Column Split Feature Contrast
            // ================================================================
            <section id="ecosystem" class="py-20 px-4 sm:px-6 md:px-10 max-w-[1400px] mx-auto border-t border-gc-border">
                <div class="mb-12 max-w-[65ch]">
                    <h2 class="text-3xl md:text-4xl font-black tracking-tighter text-gc-text mb-3">
                        "Built for taxpayers and the firms behind them."
                    </h2>
                    <p class="text-base text-gc-text-muted leading-relaxed">
                        "One Buwiz account for the web and the desktop app: own your TIN, then decide who else may work on it."
                    </p>
                </div>

                <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                    // Card 1: Buwiz account and TIN ownership (live)
                    <div class="rounded-xl border border-gc-border bg-gc-bg-elevated p-7 md:p-9 flex flex-col justify-between gap-6">
                        <div>
                            <div class="flex items-center justify-between gap-3 mb-4">
                                <div class="w-10 h-10 rounded-lg bg-gc-accent-soft flex items-center justify-center text-gc-accent">
                                    <span class="material-symbols-outlined text-[20px]">"badge"</span>
                                </div>
                                <span class="text-[11px] font-semibold px-3 py-1 rounded-full bg-gc-accent-soft text-gc-accent">
                                    "Live Now"
                                </span>
                            </div>

                            <h3 class="text-2xl font-bold tracking-tight text-gc-text mb-3">
                                "Create Your Account and Claim Your TIN"
                            </h3>
                            <p class="text-sm md:text-base text-gc-text-muted leading-relaxed mb-6">
                                "Register free, verify your email, and add your tax profile. If another workspace already holds your TIN, claim it back with your BIR Form 2303; every claim is reviewed before ownership moves, so no one can take a TIN that is not theirs."
                            </p>
                            <div class="flex flex-wrap gap-3">
                                <AccountActions />
                            </div>
                        </div>

                        <div class="grid grid-cols-1 sm:grid-cols-3 gap-3 pt-4 border-t border-gc-border">
                            <div class="p-3.5 rounded-lg bg-gc-bg border border-gc-border">
                                <div class="text-xs font-bold text-gc-text mb-1">"One Holder per TIN"</div>
                                <p class="text-[11px] text-gc-text-muted leading-relaxed">"Each TIN and branch belongs to exactly one account or workspace."</p>
                            </div>
                            <div class="p-3.5 rounded-lg bg-gc-bg border border-gc-border">
                                <div class="text-xs font-bold text-gc-text mb-1">"Desktop Sign-In"</div>
                                <p class="text-[11px] text-gc-text-muted leading-relaxed">"The desktop app signs in to the same account from your browser."</p>
                            </div>
                            <div class="p-3.5 rounded-lg bg-gc-bg border border-gc-border">
                                <div class="text-xs font-bold text-gc-text mb-1">"eGovPH Next"</div>
                                <p class="text-[11px] text-gc-text-muted leading-relaxed">"Verified national identity is planned as a faster ownership proof."</p>
                            </div>
                        </div>
                    </div>

                    // Card 2: Share Tax Profile with an Accounting Firm
                    <div class="rounded-xl border border-gc-border bg-gc-bg-elevated p-7 md:p-9 flex flex-col justify-between gap-6">
                        <div>
                            <div class="flex items-center justify-between gap-3 mb-4">
                                <div class="w-10 h-10 rounded-lg bg-gc-accent-soft flex items-center justify-center text-gc-accent">
                                    <span class="material-symbols-outlined text-[20px]">"domain"</span>
                                </div>
                                <span class="text-[11px] font-semibold px-3 py-1 rounded-full bg-gc-bg-hover text-gc-text-muted">
                                    "Client-Firm Workflow"
                                </span>
                            </div>

                            <h3 class="text-2xl font-bold tracking-tight text-gc-text mb-3">
                                "Share Your Tax Profile with a Firm to Manage for You"
                            </h3>
                            <p class="text-sm md:text-base text-gc-text-muted leading-relaxed mb-6">
                                "Business owners can share their tax profile directly with an accounting firm or bookkeeper. Your firm manages branches, yearly form sets, templates, and monthly filings on your behalf while you retain ownership of your TIN and filed returns."
                            </p>
                        </div>

                        <div class="grid grid-cols-1 sm:grid-cols-3 gap-3 pt-4 border-t border-gc-border">
                            <div class="p-3.5 rounded-lg bg-gc-bg border border-gc-border">
                                <div class="text-xs font-bold text-gc-text mb-1">"Delegate Filing"</div>
                                <p class="text-[11px] text-gc-text-muted leading-relaxed">"Grant your CPA firm access to prepare and file returns."</p>
                            </div>
                            <div class="p-3.5 rounded-lg bg-gc-bg border border-gc-border">
                                <div class="text-xs font-bold text-gc-text mb-1">"Help-Desk Admin"</div>
                                <p class="text-[11px] text-gc-text-muted leading-relaxed">"Firms switch across client TINs from the right profile drawer."</p>
                            </div>
                            <div class="p-3.5 rounded-lg bg-gc-bg border border-gc-border">
                                <div class="text-xs font-bold text-gc-text mb-1">"Owner Control"</div>
                                <p class="text-[11px] text-gc-text-muted leading-relaxed">"Revoke access anytime while keeping every historical return."</p>
                            </div>
                        </div>
                    </div>
                </div>
            </section>

            // ================================================================
            // SECTION 6: Pricing (Free for Personal / Single Tax Profile vs Paid for Multi-Profile / Firms)
            // + Expandable Technical FAQ Accordion
            // Layout Family: 2-Tier Pricing Comparison + Disclosure Accordion
            // ================================================================
            <section id="pricing" class="py-20 px-4 sm:px-6 md:px-10 max-w-[1400px] mx-auto border-t border-gc-border">
                <div class="mb-12 max-w-[65ch]">
                    <h2 class="text-3xl md:text-4xl font-black tracking-tighter text-gc-text mb-3">
                        "Free for solo taxpayers. Built to scale for firms."
                    </h2>
                    <p class="text-base text-gc-text-muted leading-relaxed">
                        "Start filing on your desktop today at zero cost. Upgrade only when you manage multiple tax profiles, multiple businesses, or client accounts."
                    </p>
                </div>

                <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-20">
                    // Tier 1: Personal / Single Tax Profile (Free)
                    <div class="rounded-xl border border-gc-border bg-gc-bg-elevated p-7 md:p-9 flex flex-col justify-between">
                        <div>
                            <div class="flex items-center justify-between gap-3 mb-4">
                                <h3 class="text-xl font-bold text-gc-text">"Personal & Single Tax Profile"</h3>
                                <span class="text-[11px] font-semibold px-3 py-1 rounded-full bg-gc-accent-soft text-gc-accent">
                                    "Free Forever"
                                </span>
                            </div>

                            <div class="flex items-baseline gap-2 mb-3">
                                <span class="text-4xl md:text-5xl font-black tracking-tight text-gc-text tabular-nums">"₱0"</span>
                                <span class="text-xs font-semibold text-gc-text-muted">"/ forever for 1 tax profile"</span>
                            </div>

                            <p class="text-sm text-gc-text-muted leading-relaxed mb-6">
                                "For individuals, freelancers, professionals, or a single business managing one 9-digit TIN."
                            </p>

                            <div class="space-y-3 pt-6 border-t border-gc-border mb-8">
                                {[
                                    "1 Taxpayer Profile (single 9-digit TIN)",
                                    "All 43 BIR forms with 100% validation parity",
                                    "Real-time side-by-side form & official BIR sheet preview",
                                    "Form templates & Guided Mode (Tab-stop filing)",
                                    "Local MCP server (bir-mcp) for Excel & AI agent filing",
                                    "Encrypted local SQLite storage with PIN or TOTP lock",
                                ]
                                .into_iter()
                                .map(|item| {
                                    view! {
                                        <div class="flex items-start gap-2.5 text-xs md:text-sm text-gc-text">
                                            <span class="material-symbols-outlined text-gc-accent text-[18px] leading-tight shrink-0">"check_circle"</span>
                                            <span>{item}</span>
                                        </div>
                                    }
                                })
                                .collect_view()}
                            </div>
                        </div>

                        <div class="flex flex-wrap gap-3">
                            <DownloadButtons />
                        </div>
                    </div>

                    // Tier 2: Multi-Profile / Business & Accounting Firms (Paid)
                    <div class="rounded-xl border border-gc-accent/40 bg-gc-bg-elevated p-7 md:p-9 flex flex-col justify-between ring-1 ring-gc-accent/20">
                        <div>
                            <div class="flex items-center justify-between gap-3 mb-4">
                                <h3 class="text-xl font-bold text-gc-text">"Multi-Profile, Businesses & Firms"</h3>
                                <span class="text-[11px] font-semibold px-3 py-1 rounded-full bg-gc-accent text-white">
                                    "Paid Plan"
                                </span>
                            </div>

                            <div class="flex items-baseline gap-2 mb-3">
                                <span class="text-4xl md:text-5xl font-black tracking-tight text-gc-text">"Pro & Firm"</span>
                                <span class="text-xs font-semibold text-gc-text-muted">"/ multiple tax profiles & businesses"</span>
                            </div>

                            <p class="text-sm text-gc-text-muted leading-relaxed mb-6">
                                "For entrepreneurs running multiple businesses, in-house tax teams, bookkeepers, and accounting firms managing client TINs."
                            </p>

                            <div class="space-y-3 pt-6 border-t border-gc-border mb-8">
                                {[
                                    "Everything in Personal, plus:",
                                    "Multiple Taxpayer Profiles & unlimited business TINs",
                                    "Multi-branch hierarchy with Head Office (00000) inheritance",
                                    "Receive & manage shared tax profiles from clients",
                                    "Bulk copy templates and form setups across branches & TINs",
                                    "Help-desk / Firm Admin mode (000-000-000 override & recovery keys)",
                                    "ERP & Buwiz Books automated multi-entity workflows",
                                ]
                                .into_iter()
                                .map(|item| {
                                    view! {
                                        <div class="flex items-start gap-2.5 text-xs md:text-sm text-gc-text">
                                            <span class="material-symbols-outlined text-gc-accent text-[18px] leading-tight shrink-0">"check_circle"</span>
                                            <span>{item}</span>
                                        </div>
                                    }
                                })
                                .collect_view()}
                            </div>
                        </div>

                        <div class="flex flex-wrap gap-3">
                            <a
                                href=FIRM_SALES_MAILTO
                                class="h-11 px-6 rounded-lg bg-gc-accent text-white font-bold text-sm inline-flex items-center justify-center gap-2 hover:bg-gc-accent-hover hover:-translate-y-[1px] active:scale-[0.98] transition-all whitespace-nowrap"
                            >
                                <span class="material-symbols-outlined text-[18px] leading-none">"business_center"</span>
                                <span>"Contact Sales"</span>
                            </a>
                        </div>
                    </div>
                </div>

                // Technical FAQ Accordion (Disclosure group matching bir-shell style)
                <div class="max-w-4xl mx-auto">
                    <h3 class="text-xl font-bold tracking-tight text-gc-text mb-4 text-center">
                        "Technical & Licensing FAQ"
                    </h3>
                    <div class="rounded-xl border border-gc-border bg-gc-bg-elevated divide-y divide-gc-border overflow-hidden">
                        {faqs
                            .into_iter()
                            .map(|faq| {
                                let id = faq.id;
                                let is_open = move || open_faq.get() == Some(id);
                                view! {
                                    <div class="p-5">
                                        <button
                                            type="button"
                                            on:click=move |_| {
                                                set_open_faq.update(|cur| {
                                                    if *cur == Some(id) {
                                                        *cur = None;
                                                    } else {
                                                        *cur = Some(id);
                                                    }
                                                });
                                            }
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
