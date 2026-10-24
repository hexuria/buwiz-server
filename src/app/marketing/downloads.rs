use leptos::prelude::*;

// Platform downloads go through `/download/<platform>`, which resolves the
// newest GitHub release asset server-side and 302-redirects to it
// (`crate::downloads`).

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
pub enum Platform {
    Windows,
    MacOS,
    Linux,
    Unknown,
}

/// Unknown platforms land on `/download`.
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

/// One download button for the visitor's platform. An undetected platform
/// gets a generic button to `/download`.
#[component]
pub fn DownloadButton() -> impl IntoView {
    let platform = detect_platform();
    let (icon, label) = match platform {
        Platform::Windows => ("desktop_windows", "Download for Windows"),
        Platform::MacOS => ("laptop_mac", "Download for macOS"),
        Platform::Linux => ("terminal", "Download for Linux"),
        Platform::Unknown => ("download", "Download Buwiz Forms"),
    };

    view! {
        <a
            href=download_path(platform)
            class="h-11 px-6 rounded-lg bg-gc-primary text-gc-primary-fg font-bold text-sm inline-flex items-center justify-center gap-2 hover:-translate-y-[1px] active:scale-[0.98] transition-all whitespace-nowrap shadow-sm"
            rel="external"
        >
            <span class="material-symbols-outlined text-[18px] leading-none">{icon}</span>
            <span>{label}</span>
        </a>
    }
}
