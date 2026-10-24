use leptos::prelude::*;
use leptos_meta::*;

#[component]
pub fn PrivacyPolicy() -> impl IntoView {
    view! {
        <Title text="Privacy Policy | Buwiz eBIRForms" />
        <Meta name="description" content="Privacy Policy for Buwiz eBIRForms. Local-first desktop application with encrypted SQLite storage and human-in-the-loop MCP controls." />

        <div class="pb-28">
            <section class="pt-16 pb-12 px-6 md:px-10 max-w-[1400px] mx-auto">
                <div class="max-w-3xl mx-auto">
                    <h1 class="text-4xl md:text-5xl font-black tracking-tighter text-gc-text mb-3 leading-tight">
                        "Privacy Policy"
                    </h1>
                    <p class="text-base text-gc-text-muted leading-relaxed">
                        "How Buwiz eBIRForms protects taxpayer profiles, local returns, and agent workflows."
                    </p>
                </div>
            </section>

            <section class="max-w-3xl mx-auto px-6 md:px-10">
                <div class="bg-gc-bg-card rounded-xl border border-gc-border p-8 md:p-10 space-y-8">
                    <div class="flex items-center gap-2.5 text-gc-text-faint text-xs font-mono">
                        <span class="material-symbols-outlined text-base text-gc-accent">"calendar_today"</span>
                        <span>"Last updated: October 16, 2026"</span>
                    </div>

                    <div class="w-full h-px bg-gc-border"></div>

                    <p class="text-gc-text-muted leading-relaxed text-sm md:text-base">
                        "Your privacy is fundamental to tax compliance. This Privacy Policy explains how Buwiz eBIRForms (the \"App\") handles your taxpayer data."
                    </p>

                    <div class="space-y-3">
                        <div class="flex items-start gap-4">
                            <div class="w-9 h-9 rounded-lg bg-gc-accent-soft flex items-center justify-center shrink-0 mt-0.5">
                                <span class="material-symbols-outlined text-gc-accent text-lg">"database"</span>
                            </div>
                            <div>
                                <h2 class="text-lg font-bold tracking-tight text-gc-text mb-2">"1. Local-First Data Ownership"</h2>
                                <p class="text-gc-text-muted leading-relaxed text-sm md:text-base">
                                    "Buwiz eBIRForms is a local-first, native desktop application for preparing and filing Philippine BIR returns on Windows and macOS."
                                </p>
                                <div class="mt-3 bg-gc-accent-soft border border-gc-accent/30 rounded-lg px-4 py-3">
                                    <p class="text-gc-accent font-semibold text-xs md:text-sm leading-relaxed">
                                        "By default, your tax profiles, branch records, templates, and returns stay on your computer inside an encrypted SQLite database."
                                    </p>
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="space-y-3">
                        <div class="flex items-start gap-4">
                            <div class="w-9 h-9 rounded-lg bg-gc-accent-soft flex items-center justify-center shrink-0 mt-0.5">
                                <span class="material-symbols-outlined text-gc-accent text-lg">"hard_drive"</span>
                            </div>
                            <div>
                                <h2 class="text-lg font-bold tracking-tight text-gc-text mb-2">"2. Encrypted Storage & Direct Filing"</h2>
                                <p class="text-gc-text-muted leading-relaxed text-sm md:text-base mb-2.5">
                                    "All data entered into the App, including taxpayer TINs, branch addresses, form templates, and tax returns, is stored locally on your device. Each taxpayer account can be locked with a personal PIN or TOTP authenticator code."
                                </p>
                                <p class="text-gc-text-muted leading-relaxed text-sm md:text-base">
                                    "When you file a return, the App communicates directly with official BIR endpoints. When you opt into firm profile sharing or upcoming eGovPH TIN verification, only the specific profile data you authorize is shared."
                                </p>
                            </div>
                        </div>
                    </div>

                    <div class="space-y-3">
                        <div class="flex items-start gap-4">
                            <div class="w-9 h-9 rounded-lg bg-gc-accent-soft flex items-center justify-center shrink-0 mt-0.5">
                                <span class="material-symbols-outlined text-gc-accent text-lg">"terminal"</span>
                            </div>
                            <div>
                                <h2 class="text-lg font-bold tracking-tight text-gc-text mb-2">"3. Local MCP Server & Agent Safety"</h2>
                                <p class="text-gc-text-muted leading-relaxed text-sm md:text-base">
                                    "The built-in MCP server (bir-mcp) runs locally over stdio or loopback HTTP and requires your explicit confirmation before connecting to real taxpayer data. By construction, MCP tools are draft-only: AI agents cannot submit, queue, or pay a return without your direct click inside the App."
                                </p>
                            </div>
                        </div>
                    </div>

                    <div class="space-y-3">
                        <div class="flex items-start gap-4">
                            <div class="w-9 h-9 rounded-lg bg-gc-accent-soft flex items-center justify-center shrink-0 mt-0.5">
                                <span class="material-symbols-outlined text-gc-accent text-lg">"mail"</span>
                            </div>
                            <div>
                                <h2 class="text-lg font-bold tracking-tight text-gc-text mb-2">"4. Contact Us"</h2>
                                <p class="text-gc-text-muted leading-relaxed text-sm md:text-base">
                                    "If you have any questions about this privacy policy, email us at "
                                    <a href="mailto:support@goldcoders.dev" class="text-gc-accent hover:underline font-medium">
                                        "support@goldcoders.dev"
                                    </a>
                                    " or open an issue on our "
                                    <a href="https://github.com/codeitlikemiley/ebirforms" target="_blank" rel="noopener noreferrer" class="text-gc-accent hover:underline font-medium">
                                        "GitHub repository"
                                    </a>
                                    "."
                                </p>
                            </div>
                        </div>
                    </div>

                    <div class="w-full h-px bg-gc-border"></div>

                    <div class="flex justify-start">
                        <a href="/" class="inline-flex items-center gap-2 text-gc-text-muted hover:text-gc-accent transition-colors text-xs font-semibold">
                            <span class="material-symbols-outlined text-base">"arrow_back"</span>
                            <span>"Back to Buwiz eBIRForms"</span>
                        </a>
                    </div>
                </div>
            </section>
        </div>
    }
}
