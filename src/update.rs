//! Safe update discovery for this fork.
//!
//! Updates are never downloaded, executed, or installed by the application.
//! The app only queries GitHub's release metadata and opens the official release
//! page so the user can review and install an update themselves.

use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;
use serde_json::Value;

use crate::{config, console, net};

/// This fork's repository (owner/name).
const REPO: &str = "sillyjoshua/nanoMIDIPlayer-rs";
const RELEASES_URL: &str = "https://github.com/sillyjoshua/nanoMIDIPlayer-rs/releases";

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Not checked yet in this session.
    Idle,
    Checking,
    UpToDate,
    Available,
    /// Retained for compatibility with the existing UI; never emitted.
    Downloading(Option<f32>),
    /// Retained for compatibility with the existing UI; never emitted.
    Installed,
    Error(String),
}

#[derive(Clone, Debug, Default)]
pub struct Release {
    pub version: String,
    pub page_url: String,
    /// Retained for compatibility; the app deliberately does not download assets.
    pub asset_url: Option<String>,
    pub size: u64,
    pub notes: String,
    pub published: String,
}

pub struct State {
    pub status: Status,
    pub latest: Option<Release>,
    pub last_checked: Option<String>,
    /// The update banner was closed for this session.
    pub dismissed: bool,
}

static STATE: Mutex<State> = parking_lot::const_mutex(State {
    status: Status::Idle,
    latest: None,
    last_checked: None,
    dismissed: false,
});
static VERSION: AtomicU64 = AtomicU64::new(0);

fn set_status(status: Status) {
    STATE.lock().status = status;
    VERSION.fetch_add(1, Ordering::Release);
}

/// Changes whenever the update state changes (for the UI redraw check).
pub fn version() -> u64 {
    VERSION.load(Ordering::Acquire)
}

pub fn state() -> parking_lot::MutexGuard<'static, State> {
    STATE.lock()
}

pub fn dismiss() {
    STATE.lock().dismissed = true;
    VERSION.fetch_add(1, Ordering::Release);
}

/// The version embedded at build time. Environment variables cannot override it.
pub fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Parses "v1.2.3" / "1.2.3" into comparable numbers.
fn parse_version(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches('v')
        .split('.')
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    parse_version(latest) > parse_version(current)
}

pub fn busy() -> bool {
    matches!(STATE.lock().status, Status::Checking | Status::Downloading(_))
}

fn fetch_latest() -> anyhow::Result<Release> {
    let info: Value =
        net::get_json(&format!("https://api.github.com/repos/{REPO}/releases/latest"))?;
    let version = info["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no release found"))?
        .to_string();

    // Only accept release-page URLs on this exact repository. Never trust a
    // URL supplied by release metadata to send the user to an unrelated host.
    let page_url = info["html_url"]
        .as_str()
        .filter(|url| {
            url.starts_with("https://github.com/sillyjoshua/nanoMIDIPlayer-rs/releases/")
        })
        .unwrap_or(RELEASES_URL)
        .to_string();

    Ok(Release {
        page_url,
        // Deliberately do not retain or use executable asset download URLs.
        asset_url: None,
        size: 0,
        notes: info["body"].as_str().unwrap_or("").trim().to_string(),
        published: info["published_at"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(10)
            .collect(),
        version,
    })
}

/// Checks GitHub release metadata in the background.
///
/// The parameter is retained for compatibility with existing callers. It is
/// intentionally ignored: automatic executable installation is disabled.
pub fn check(_install_found: bool) {
    if busy() {
        return;
    }
    set_status(Status::Checking);
    std::thread::spawn(|| {
        let result = fetch_latest();
        let now = chrono::Local::now().format("%H:%M").to_string();
        {
            let mut st = STATE.lock();
            st.last_checked = Some(now);
            match result {
                Ok(release) => {
                    let newer = is_newer(&release.version, &current_version());
                    st.status = if newer {
                        Status::Available
                    } else {
                        Status::UpToDate
                    };
                    if newer {
                        console::insert(format!("Update {} is available", release.version));
                    }
                    st.latest = Some(release);
                }
                Err(e) => {
                    log::warn!("update check failed: {e}");
                    st.status = Status::Error(short_error(&e));
                }
            }
        }
        VERSION.fetch_add(1, Ordering::Release);
    });
}

fn short_error(e: &anyhow::Error) -> String {
    let s = e.to_string();
    if s.to_lowercase().contains("timeout") || s.to_lowercase().contains("timed out") {
        "Timed out. Check your internet connection.".into()
    } else if s.to_lowercase().contains("dns")
        || s.to_lowercase().contains("connect")
        || s.to_lowercase().contains("resolve")
    {
        "Couldn't reach GitHub. Are you offline?".into()
    } else {
        s.chars().take(120).collect()
    }
}

/// Opens the official release page. The user reviews and installs downloads
/// manually; the application never replaces its own executable.
pub fn install() {
    let url = STATE
        .lock()
        .latest
        .as_ref()
        .map(|release| release.page_url.clone())
        .filter(|url| {
            url.starts_with("https://github.com/sillyjoshua/nanoMIDIPlayer-rs/releases/")
        })
        .unwrap_or_else(|| RELEASES_URL.to_string());

    net::open_url(&url);
    console::toast("Opened the official GitHub releases page. Review downloads before installing.");
}

/// Automatic replacement/relaunch has been removed. Kept temporarily because
/// older UI code references this function; it never starts another process.
pub fn relaunch() -> bool {
    false
}

/// Startup: check for release metadata only. No files are downloaded or changed.
pub fn on_startup() {
    let check_on_start = config::get().read().app_ui.check_for_updates;
    if check_on_start && cfg!(windows) {
        check(false);
    }
}

#[cfg(test)]
mod tests {
    use super::{is_newer, parse_version};

    #[test]
    fn versions() {
        assert!(is_newer("v1.2.0", "1.1.0"));
        assert!(is_newer("1.10.0", "1.9.3"));
        assert!(!is_newer("v1.1.0", "1.1.0"));
        assert!(!is_newer("v1.0.9", "1.1.0"));
    }

    #[test]
    fn version_parser_handles_prefix_and_components() {
        assert_eq!(parse_version("v2.4.1"), vec![2, 4, 1]);
    }
}
