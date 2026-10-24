use leptos::prelude::*;

/// Releases page — used for the "all packages" link. Platform downloads go
/// through `/download/<platform>`, which resolves the newest GitHub release
/// asset server-side and 302-redirects to it (`crate::downloads`).
pub const RELEASES_URL: &str = "https://github.com/hexuria/buwiz-forms/releases/latest";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOS,
    Linux,
    Unknown,
}

/// Unknown platforms land on the releases page.
pub fn download_path(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => "/download/windows",
        Platform::MacOS => "/download/macos",
        Platform::Linux => "/download/linux",
        Platform::Unknown => "/download",
    }
}

/// Best-effort OS detection for picking the default download.
pub fn detect_platform() -> Platform {
    #[cfg(feature = "hydrate")]
    {
        let agent = web_sys::window()
            .and_then(|w| w.navigator().user_agent().ok())
            .unwrap_or_default()
            .to_lowercase();
        if agent.contains("android") {
            Platform::Unknown
        } else if agent.contains("windows") {
            Platform::Windows
        } else if agent.contains("mac os") || agent.contains("macintosh") {
            Platform::MacOS
        } else if agent.contains("linux") || agent.contains("x11") {
            Platform::Linux
        } else {
            Platform::Unknown
        }
    }
    #[cfg(not(feature = "hydrate"))]
    {
        Platform::Unknown
    }
}

/// Windows / macOS / Linux download buttons backed by the newest release.
/// The visitor's platform (Windows when undetected) gets the filled style.
#[component]
pub fn DownloadButtons() -> impl IntoView {
    let primary = match detect_platform() {
        Platform::Unknown => Platform::Windows,
        platform => platform,
    };

    let button_class = move |platform: Platform| {
        if platform == primary {
            "h-11 px-6 rounded-lg bg-gc-primary text-gc-primary-fg font-bold text-sm inline-flex items-center justify-center gap-2 hover:-translate-y-[1px] active:scale-[0.98] transition-all whitespace-nowrap shadow-sm"
        } else {
            "h-11 px-5 rounded-lg bg-gc-bg-elevated border border-gc-border-strong text-gc-text font-semibold text-sm inline-flex items-center justify-center gap-2 hover:bg-gc-bg-hover hover:-translate-y-[1px] active:scale-[0.98] transition-all whitespace-nowrap"
        }
    };

    view! {
        <a href=download_path(Platform::Windows) class=button_class(Platform::Windows) rel="external">
            <span class="material-symbols-outlined text-[18px] leading-none">"desktop_windows"</span>
            <span>"Download for Windows"</span>
        </a>
        <a href=download_path(Platform::MacOS) class=button_class(Platform::MacOS) rel="external">
            <span class="material-symbols-outlined text-[18px] leading-none">"laptop_mac"</span>
            <span>"Download for macOS"</span>
        </a>
        <a href=download_path(Platform::Linux) class=button_class(Platform::Linux) rel="external">
            <span class="material-symbols-outlined text-[18px] leading-none">"terminal"</span>
            <span>"Download for Linux"</span>
        </a>
    }
}
