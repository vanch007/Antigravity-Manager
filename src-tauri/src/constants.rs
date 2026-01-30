use regex::Regex;
use std::sync::LazyLock;

/// URL to fetch the latest Antigravity version
const VERSION_URL: &str = "https://antigravity-auto-updater-974169037036.us-central1.run.app";

/// Hardcoded fallback version if all else fails
/// NOTE: Update this when releasing major versions
const FALLBACK_VERSION: &str = "1.15.8";

/// Pre-compiled regex for version parsing (X.Y.Z pattern)
static VERSION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d+\.\d+\.\d+").expect("Invalid version regex"));

/// Parse version from response text using pre-compiled regex
/// Matches semver pattern: X.Y.Z (e.g., "1.15.8")
fn parse_version(text: &str) -> Option<String> {
    VERSION_REGEX.find(text).map(|m| m.as_str().to_string())
}

/// Version source for logging
#[derive(Debug)]
enum VersionSource {
    Remote,
    CargoToml,
    Fallback,
}

/// Fetch version from remote endpoint, with multiple fallbacks
/// Uses a separate thread to avoid blocking the main/UI thread
fn fetch_remote_version() -> (String, VersionSource) {
    // Spawn a named thread for the blocking HTTP call
    let handle = std::thread::Builder::new()
        .name("version-fetch".to_string())
        .spawn(|| {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(3))
                .build()
                .ok()?;

            let response = client.get(VERSION_URL).send().ok()?;
            let text = response.text().ok()?;
            parse_version(&text)
        });

    // Wait for the thread
    if let Ok(handle) = handle {
        if let Ok(Some(version)) = handle.join() {
            return (version, VersionSource::Remote);
        }
    }

    // Fallback 1: Cargo.toml version
    let cargo_version = env!("CARGO_PKG_VERSION");
    if !cargo_version.is_empty() && cargo_version.contains('.') {
        return (cargo_version.to_string(), VersionSource::CargoToml);
    }

    // Fallback 2: Hardcoded version
    (FALLBACK_VERSION.to_string(), VersionSource::Fallback)
}

/// Shared User-Agent string for all upstream API requests.
/// Format: antigravity/{version} {os}/{arch}
/// Version priority: remote endpoint > Cargo.toml > hardcoded fallback
/// OS and architecture are detected at runtime.
pub static USER_AGENT: LazyLock<String> = LazyLock::new(|| {
    let (version, source) = fetch_remote_version();

    tracing::info!(
        "User-Agent version initialized: {} (source: {:?})",
        version,
        source
    );

    format!(
        "antigravity/{} {}/{}",
        version,
        std::env::consts::OS,
        std::env::consts::ARCH
    )
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_from_updater_response() {
        let text = "Auto updater is running. Stable Version: 1.15.8-5724687216017408";
        assert_eq!(parse_version(text), Some("1.15.8".to_string()));
    }

    #[test]
    fn test_parse_version_simple() {
        assert_eq!(parse_version("1.15.8"), Some("1.15.8".to_string()));
        assert_eq!(parse_version("Version: 2.0.0"), Some("2.0.0".to_string()));
        assert_eq!(parse_version("v1.2.3"), Some("1.2.3".to_string()));
    }

    #[test]
    fn test_parse_version_invalid() {
        assert_eq!(parse_version("no version here"), None);
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("1.2"), None); // Only X.Y, not X.Y.Z
    }

    #[test]
    fn test_parse_version_with_suffix() {
        // Regex only matches X.Y.Z, suffix is naturally excluded
        let text = "antigravity/1.15.8 windows/amd64";
        assert_eq!(parse_version(text), Some("1.15.8".to_string()));
    }
}
