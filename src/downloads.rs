//! `GET /download[/<platform>]`: resolve the newest buwiz-forms release asset
//! and redirect to it, so site buttons never hardcode versioned URLs. Any
//! lookup failure falls back to the releases page rather than an error.

use std::time::Duration;

use serde::Deserialize;

const RELEASE_API: &str = "https://api.github.com/repos/hexuria/buwiz-forms/releases/latest";
pub const RELEASES_PAGE: &str = "https://github.com/hexuria/buwiz-forms/releases/latest";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOS,
    Linux,
}

pub fn is_download_path(path: &str) -> bool {
    path == "/download" || path.starts_with("/download/")
}

/// `None` (bare `/download` or an unknown segment) means the releases page.
pub fn platform_for_path(path: &str) -> Option<Platform> {
    let segment = path.trim_start_matches("/download").trim_matches('/');
    match segment.to_ascii_lowercase().as_str() {
        "windows" | "win" => Some(Platform::Windows),
        "macos" | "mac" | "darwin" => Some(Platform::MacOS),
        "linux" => Some(Platform::Linux),
        _ => None,
    }
}

pub async fn resolve(platform: Option<Platform>) -> String {
    let Some(platform) = platform else {
        return RELEASES_PAGE.to_string();
    };
    match latest_assets().await {
        Ok(assets) => pick_asset(&assets, platform).unwrap_or_else(|| RELEASES_PAGE.to_string()),
        Err(error) => {
            tracing::warn!(error = %error, "release lookup failed; redirecting to releases page");
            RELEASES_PAGE.to_string()
        }
    }
}

#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
pub struct Asset {
    name: String,
    browser_download_url: String,
}

fn pick_asset(assets: &[Asset], platform: Platform) -> Option<String> {
    assets
        .iter()
        .find(|asset| {
            let name = asset.name.as_str();
            match platform {
                Platform::Windows => name.contains("Windows-x64") && name.ends_with("-Setup.exe"),
                Platform::MacOS => name.contains("macOS") && name.ends_with(".dmg"),
                Platform::Linux => name.contains("Linux-x64") && name.ends_with(".deb"),
            }
        })
        .map(|asset| asset.browser_download_url.clone())
}

async fn latest_assets() -> Result<Vec<Asset>, String> {
    let mut headers = http::HeaderMap::new();
    headers.insert(
        http::header::USER_AGENT,
        http::HeaderValue::from_static("buwiz-server (+https://buwiz.com)"),
    );
    headers.insert(
        http::header::ACCEPT,
        http::HeaderValue::from_static("application/vnd.github+json"),
    );
    let (status, _, body) = crate::claim_ai::transport::send(
        http::Method::GET,
        RELEASE_API,
        headers,
        bytes::Bytes::new(),
        Duration::from_secs(10),
    )
    .await
    .map_err(|failure| failure.to_string())?;
    if status != 200 {
        return Err(format!("github api returned {status}"));
    }
    let release: Release =
        serde_json::from_slice(&body).map_err(|error| format!("release json: {error}"))?;
    Ok(release.assets)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> Asset {
        Asset { name: name.into(), browser_download_url: format!("https://dl/{name}") }
    }

    #[test]
    fn paths_map_to_platforms() {
        assert!(is_download_path("/download"));
        assert!(is_download_path("/download/macos"));
        assert!(!is_download_path("/downloads"));
        assert_eq!(platform_for_path("/download/Windows"), Some(Platform::Windows));
        assert_eq!(platform_for_path("/download/mac/"), Some(Platform::MacOS));
        assert_eq!(platform_for_path("/download/linux"), Some(Platform::Linux));
        assert_eq!(platform_for_path("/download"), None);
        assert_eq!(platform_for_path("/download/beos"), None);
    }

    #[test]
    fn picks_installer_not_portable_build() {
        let assets = vec![
            asset("Buwiz-Forms-1.2.0-Windows-x64-portable.zip"),
            asset("Buwiz-Forms-1.2.0-Windows-x64-Setup.exe"),
            asset("Buwiz-Forms-1.2.0-macOS-universal.dmg"),
            asset("Buwiz-Forms-1.2.0-Linux-x64.deb"),
        ];
        assert_eq!(
            pick_asset(&assets, Platform::Windows).as_deref(),
            Some("https://dl/Buwiz-Forms-1.2.0-Windows-x64-Setup.exe")
        );
        assert_eq!(
            pick_asset(&assets, Platform::MacOS).as_deref(),
            Some("https://dl/Buwiz-Forms-1.2.0-macOS-universal.dmg")
        );
        assert_eq!(
            pick_asset(&assets, Platform::Linux).as_deref(),
            Some("https://dl/Buwiz-Forms-1.2.0-Linux-x64.deb")
        );
        assert_eq!(pick_asset(&assets[..1], Platform::MacOS), None);
    }
}
