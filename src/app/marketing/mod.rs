//! Public buwiz.com site, ported from goldcoders.dev/ebirforms.
//!
//! Styling uses the `gc-*` palette (see `input.css`), which follows the same
//! `html[data-theme]` the app writes, so the site and app share one setting.

mod downloads;
mod privacy;
mod showcase;

use leptos::prelude::*;
use leptos_meta::*;

use super::theme::MarketingThemeToggle;
use super::{browser_load, get_current_session};
use downloads::{download_path, Platform};
use privacy::PrivacyPolicy;
use showcase::Showcase;

const NAV_LINK: &str =
    "text-xs font-semibold text-gc-text-muted hover:text-gc-text transition-colors whitespace-nowrap";
const FOOTER_LINK: &str = "text-xs tracking-tight text-gc-text-muted hover:text-gc-accent transition-colors";
const HEADER_GHOST: &str = "h-9 px-3.5 rounded-lg border border-gc-border bg-gc-bg-hover/60 hover:bg-gc-bg-hover text-gc-text text-xs font-semibold inline-flex items-center gap-1.5 transition-colors whitespace-nowrap active:scale-[0.98]";
const HEADER_SOLID: &str = "h-9 px-4 rounded-lg bg-gc-primary text-gc-primary-fg text-xs font-bold inline-flex items-center gap-1.5 hover:opacity-90 active:scale-[0.98] transition-all whitespace-nowrap";

#[component]
pub fn MarketingHomePage() -> impl IntoView {
    view! {
        <Title text="Buwiz Forms | The Modern Alternative to eBIRForms" />
        <Meta
            name="description"
            content="Meet Buwiz Forms. A faster, free, AI-ready alternative to eBIRForms. Native on Windows, macOS, and Linux, with BIR-matched form validation, live side-by-side preview, templates, Guided Mode, and a built-in MCP server for AI agents."
        />
        <MarketingLayout>
            <Showcase />
        </MarketingLayout>
    }
}

#[component]
pub fn MarketingPrivacyPage() -> impl IntoView {
    view! {
        <MarketingLayout>
            <PrivacyPolicy />
        </MarketingLayout>
    }
}

#[component]
fn MarketingLayout(children: Children) -> impl IntoView {
    view! {
        <Link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Manrope:wght@400;500;600;700;800;900&display=swap" />
        <Link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:wght,FILL@100..700,0..1&display=swap" />
        <div
            class="min-h-[100dvh] bg-gc-bg text-gc-text font-['Manrope',system-ui,sans-serif] antialiased selection:bg-gc-accent/20 selection:text-gc-accent flex flex-col"
            data-testid="marketing-site"
        >
            <header class="bg-gc-bg-elevated/95 backdrop-blur-md border-b border-gc-border sticky top-0 z-50 h-16">
                <div class="flex justify-between items-center w-full px-4 sm:px-6 md:px-10 h-16 max-w-[1400px] mx-auto">
                    <div class="flex items-center gap-8">
                        <a href="/" class="inline-flex items-end gap-2.5 active:opacity-80 transition-opacity" aria-label="Buwiz home">
                            <img src="/assets/brand/buwiz_logo_light.webp" alt="" width="30" height="30" class="block h-7 w-7 shrink-0 object-contain object-bottom dark:hidden" />
                            <img src="/assets/brand/buwiz_logo_dark.webp" alt="" width="30" height="30" class="hidden h-7 w-7 shrink-0 object-contain object-bottom dark:block" />
                            <span class="text-lg font-black tracking-tighter leading-none text-gc-text uppercase pb-px">"BUWIZ"</span>
                        </a>
                        <nav class="hidden lg:flex items-center gap-6" aria-label="Primary Navigation">
                            <a href="/#workspace" class=NAV_LINK>"Workspace"</a>
                            <a href="/#validation" class=NAV_LINK>"Live Form & Validation"</a>
                            <a href="/#mcp" class=NAV_LINK>"MCP Agent"</a>
                            <a href="/#ecosystem" class=NAV_LINK>"Accounts & Firms"</a>
                            <a href="/#pricing" class=NAV_LINK>"Pricing"</a>
                        </nav>
                    </div>
                    <div class="flex items-center gap-2">
                        <MarketingThemeToggle />
                        <HeaderAccountLinks />
                    </div>
                </div>
            </header>

            <div class="flex-grow w-full">{children()}</div>

            <footer class="bg-gc-bg-elevated w-full border-t border-gc-border">
                <div class="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 px-6 md:px-10 py-10 max-w-[1400px] mx-auto">
                    <div>
                        <p class="text-sm font-bold tracking-tight text-gc-text mb-1">"Buwiz eBIRForms"</p>
                        <p class="text-xs tracking-tight text-gc-text-muted">"© 2026 Buwiz by Goldcoders. Philippine tax, books, invoice, and pay."</p>
                    </div>
                    <div class="flex flex-wrap gap-5">
                        <a href="/#pricing" class=FOOTER_LINK>"Pricing"</a>
                        <a href=download_path(Platform::Unknown) class=FOOTER_LINK rel="external">"Downloads"</a>
                        <a href="/privacy" class=FOOTER_LINK>"Privacy Policy"</a>
                        <a href="https://goldcoders.dev/legalities" class=FOOTER_LINK>"Legalities"</a>
                        <a href="mailto:support@buwiz.com" class=FOOTER_LINK>"support@buwiz.com"</a>
                        <a href="https://github.com/hexuria/buwiz-forms" target="_blank" rel="noopener noreferrer" class=FOOTER_LINK>"GitHub"</a>
                    </div>
                </div>
            </footer>
        </div>
    }
}

/// Signed in: one link to the dashboard. Signed out (or still loading):
/// sign in and register, so the header never flashes empty.
#[island]
fn HeaderAccountLinks() -> impl IntoView {
    let session = browser_load(get_current_session);
    view! {
        {move || match session.get() {
            Some(Ok(view)) if view.authenticated => view! {
                <a href="/dashboard" class=HEADER_SOLID data-testid="marketing-dashboard">
                    <span class="material-symbols-outlined text-[15px] leading-none">"space_dashboard"</span>
                    <span>"Dashboard"</span>
                </a>
            }
            .into_any(),
            _ => view! {
                <a href="/login" class=HEADER_GHOST data-testid="marketing-sign-in">"Sign in"</a>
                <a href="/register" class=HEADER_SOLID data-testid="marketing-register">
                    <span class="hidden sm:inline">"Create free account"</span>
                    <span class="sm:hidden">"Register"</span>
                </a>
            }
            .into_any(),
        }}
    }
}

/// Account call to action for the page body; same session rule as the header.
#[island]
fn AccountActions() -> impl IntoView {
    let session = browser_load(get_current_session);
    view! {
        {move || match session.get() {
            Some(Ok(view)) if view.authenticated => view! {
                <a href="/tax-profiles" class=BODY_SOLID>
                    <span class="material-symbols-outlined text-[18px] leading-none">"badge"</span>
                    <span>"Manage tax profiles"</span>
                </a>
                <a href="/dashboard" class=BODY_GHOST>"Open dashboard"</a>
            }
            .into_any(),
            _ => view! {
                <a href="/register" class=BODY_SOLID>
                    <span class="material-symbols-outlined text-[18px] leading-none">"person_add"</span>
                    <span>"Create free account"</span>
                </a>
                <a href="/login" class=BODY_GHOST>"Sign in"</a>
            }
            .into_any(),
        }}
    }
}

const BODY_SOLID: &str = "h-11 px-6 rounded-lg bg-gc-accent text-white font-bold text-sm inline-flex items-center justify-center gap-2 hover:bg-gc-accent-hover hover:-translate-y-[1px] active:scale-[0.98] transition-all whitespace-nowrap";
const BODY_GHOST: &str = "h-11 px-5 rounded-lg bg-gc-bg-elevated border border-gc-border-strong text-gc-text font-semibold text-sm inline-flex items-center justify-center gap-2 hover:bg-gc-bg-hover hover:-translate-y-[1px] active:scale-[0.98] transition-all whitespace-nowrap";

/// A 1440×1129 bir-shell screenshot in both themes; CSS shows the one that
/// matches `data-theme` (including `system`), and lazy loading skips the
/// hidden one.
fn themed_screenshot<S, A>(src: S, alt: A, eager: bool) -> impl IntoView
where
    S: Fn(bool) -> &'static str + Clone + Send + Sync + 'static,
    A: Fn() -> &'static str + Clone + Send + Sync + 'static,
{
    let (dark_src, dark_alt) = (src.clone(), alt.clone());
    let loading = if eager { "eager" } else { "lazy" };
    view! {
        <img
            src=move || src(false)
            alt=alt
            width="1440"
            height="1129"
            loading=loading
            class="block w-full h-full object-cover object-top dark:hidden"
        />
        <img
            src=move || dark_src(true)
            alt=dark_alt
            width="1440"
            height="1129"
            loading=loading
            class="hidden w-full h-full object-cover object-top dark:block"
        />
    }
}
