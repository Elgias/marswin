//! Self-update from GitHub Releases (`latest.json`, signed with the key in tauri.conf.json).
//! Only an installed copy updates itself; the portable exe links to the release page.

use serde::Serialize;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};

const RELEASES: &str = "https://github.com/Elgias/marswin/releases/latest";
/// First check a little after start (the network may still be coming up), then every 6 hours.
const FIRST_CHECK: Duration = Duration::from_secs(15);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);

static FOUND: LazyLock<Mutex<Option<Update>>> = LazyLock::new(Default::default);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    /// False for the portable exe: it opens the release page instead.
    pub can_install: bool,
}

/// The NSIS installer puts uninstall.exe next to the app; the portable exe has none.
fn installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("uninstall.exe").exists()))
        .unwrap_or(false)
}

pub fn found() -> Option<UpdateInfo> {
    FOUND.lock().unwrap().as_ref().map(|u| UpdateInfo { version: u.version.clone(), can_install: installed() })
}

/// Asks GitHub for a newer version; tells the pages ("update") and calls `on_found`.
pub async fn check(app: &AppHandle, on_found: impl Fn(&AppHandle)) -> Result<Option<UpdateInfo>, String> {
    let mut builder = app.updater_builder();
    // For testing against a local server instead of GitHub.
    if let Some(url) = std::env::var("MARSWIN_UPDATE_URL").ok().and_then(|u| u.parse().ok()) {
        builder = builder.endpoints(vec![url]).map_err(|e| e.to_string())?;
    }
    let updater = builder.build().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    let had = FOUND.lock().unwrap().as_ref().map(|u| u.version.clone());
    let now = update.as_ref().map(|u| u.version.clone());
    *FOUND.lock().unwrap() = update;
    let info = found();
    if had != now {
        let _ = app.emit("update", info.clone());
        on_found(app);
    }
    Ok(info)
}

pub fn spawn_checks(app: AppHandle, on_found: fn(&AppHandle)) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            // Offline or GitHub down: try again next round.
            let _ = check(&app, on_found).await;
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// Installed copy: downloads, checks the signature, runs the installer (which restarts MarsWin).
/// Portable copy: opens the release page.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    if !installed() {
        return std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", RELEASES])
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string());
    }
    let update = FOUND.lock().unwrap().clone();
    let Some(update) = update else { return Err("no update".into()) };
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    app.restart();
}
