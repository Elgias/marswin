mod fit;
mod marstek;
mod settings;
mod snapshot;

use serde_json::{json, Value};
use settings::{DeviceRef, Settings, SettingsPatch, CHARGE_MAX, CHARGE_MIN};
use snapshot::{Flow, Snapshot};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tokio::sync::Notify;
use tokio::time::Instant;

const HIDDEN_ARG: &str = "--hidden";
const TRAY_ID: &str = "tray";
/// Passive mode needs a countdown; 24 h is the longest the firmware accepts.
const PASSIVE_SECONDS: u32 = 86_400;
const DISCOVER_WAIT: Duration = Duration::from_millis(2500);
const BG: tauri::window::Color = tauri::window::Color(9, 9, 11, 255);

struct Shared {
    path: PathBuf,
    settings: Mutex<Settings>,
    snapshot: Mutex<Snapshot>,
    /// Wakes the poller early (new device, new interval, mode change).
    wake: Notify,
    /// Makes the next poll also read mode, Wi-Fi and device info.
    full_poll: AtomicBool,
}

impl Shared {
    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    fn update(&self, f: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
        let mut s = self.settings.lock().unwrap();
        f(&mut s);
        s.save(&self.path)?;
        Ok(s.clone())
    }

    fn poll_now(&self, full: bool) {
        if full {
            self.full_poll.store(true, Ordering::SeqCst);
        }
        self.wake.notify_one();
    }
}

// ---------------------------------------------------------------- tray text

fn tr(lang: &str, key: &str) -> &'static str {
    let en = lang == "en";
    match key {
        "show" => if en { "Open MarsWin" } else { "Відкрити MarsWin" },
        "settings" => if en { "Settings" } else { "Налаштування" },
        "autostart" => if en { "Start with Windows" } else { "Запускати з Windows" },
        "quit" => if en { "Quit" } else { "Вийти" },
        "charging" => if en { "Charging" } else { "Заряджання" },
        "discharging" => if en { "Discharging" } else { "Розряджання" },
        "idle" => if en { "Idle" } else { "Очікування" },
        "offline" => if en { "No connection" } else { "Немає зв'язку" },
        "nodevice" => if en { "No device selected" } else { "Пристрій не вибрано" },
        _ => "",
    }
}

fn tooltip(lang: &str, snap: &Snapshot, has_device: bool) -> String {
    let state = if !has_device {
        tr(lang, "nodevice").to_string()
    } else if !snap.online {
        tr(lang, "offline").to_string()
    } else {
        let flow = match snap.flow {
            Flow::Charging => tr(lang, "charging"),
            Flow::Discharging => tr(lang, "discharging"),
            Flow::Idle => tr(lang, "idle"),
        };
        match snap.soc {
            Some(soc) => format!("{soc:.0}% · {flow}"),
            None => flow.to_string(),
        }
    };
    format!("MarsWin — {state}")
}

fn build_menu(app: &AppHandle, lang: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "show", tr(lang, "show"), true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", tr(lang, "settings"), true, None::<&str>)?,
            &CheckMenuItem::with_id(app, "autostart", tr(lang, "autostart"), true, autostart, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", tr(lang, "quit"), true, None::<&str>)?,
        ],
    )
}

fn refresh_tray(app: &AppHandle) {
    let shared = app.state::<Shared>();
    let s = shared.settings();
    let snap = shared.snapshot.lock().unwrap().clone();
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tooltip(&s.lang, &snap, s.device.is_some())));
    }
}

fn rebuild_tray_menu(app: &AppHandle) {
    let lang = app.state::<Shared>().settings().lang;
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app, &lang)) {
        let _ = tray.set_menu(Some(menu));
    }
}

// ---------------------------------------------------------------- windows

fn fit_spec(label: &str) -> &'static fit::Window {
    if label == "settings" { &fit::SETTINGS } else { &fit::MAIN }
}

/// Sizes the window for the monitor it is on (see fit.rs); the page follows with CSS zoom.
fn resize_to_fit(win: &WebviewWindow, center: bool) {
    let spec = fit_spec(win.label());
    let monitor = match win.current_monitor() {
        Ok(Some(m)) => m,
        _ => match win.primary_monitor() {
            Ok(Some(m)) => m,
            _ => return,
        },
    };
    let area = monitor.work_area().size.to_logical::<f64>(monitor.scale_factor());
    let (w, h) = fit::size(spec, (area.width, area.height));
    let _ = win.set_size(tauri::LogicalSize::new(w, h));
    if center {
        let _ = win.center();
    }
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn toggle_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false) {
            let _ = w.hide();
        } else {
            show_main(app);
        }
    }
}

fn show_settings(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(());
    }
    let title = tr(&app.state::<Shared>().settings().lang, "settings");
    let win = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title(title)
        .inner_size(fit::SETTINGS.design.0 * fit::BASE_SCALE, fit::SETTINGS.design.1 * fit::BASE_SCALE)
        .resizable(false)
        .maximizable(false)
        .decorations(false)
        .background_color(BG)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    resize_to_fit(&win, true);
    win.show().map_err(|e| e.to_string())?;
    let _ = win.set_focus();
    Ok(())
}

// ---------------------------------------------------------------- polling

fn publish(app: &AppHandle, snap: Snapshot) {
    *app.state::<Shared>().snapshot.lock().unwrap() = snap.clone();
    let _ = app.emit("snapshot", snap);
    refresh_tray(app);
}

async fn poll_once(dev: &DeviceRef, wait: Duration, full: bool, prev: Snapshot, last_mode: &mut Value) -> Snapshot {
    let call = |method: &'static str, params: Value| marstek::call(&dev.ip, dev.port, method, params, wait);
    let mut snap = prev;
    snap.device.get_or_insert_with(|| dev.clone());

    let bat = match call("Bat.GetStatus", json!({ "id": 0 })).await {
        Ok(v) => v,
        Err(e) => {
            // The device is unreachable; skip the other calls instead of waiting out each timeout.
            snap.online = false;
            snap.error = Some(e);
            return snap;
        }
    };
    let es = call("ES.GetStatus", json!({ "id": 0 })).await.unwrap_or(Value::Null);
    if full {
        if let Ok(m) = call("ES.GetMode", json!({ "id": 0 })).await {
            snap.apply_mode(&m);
            *last_mode = m;
        }
        if let Ok(w) = call("Wifi.GetStatus", json!({ "id": 0 })).await {
            snap.apply_wifi(&w);
        }
        if let Ok(d) = call("Marstek.GetDevice", json!({ "ble_mac": "0" })).await {
            snap.apply_device(&d);
        }
    }
    snap.apply_battery(&bat, &es);
    snap.apply_power(&es, last_mode);
    snap.online = true;
    snap.error = None;
    snap
}

async fn poller(app: AppHandle) {
    let shared = app.state::<Shared>();
    let mut last_full: Option<Instant> = None;
    let mut last_mode = Value::Null;
    loop {
        let s = shared.settings();
        if let Some(dev) = s.device.clone() {
            let mode_every = Duration::from_secs(s.mode_check_sec.into());
            let full = shared.full_poll.swap(false, Ordering::SeqCst)
                || last_full.is_none_or(|t| t.elapsed() >= mode_every);
            let prev = shared.snapshot.lock().unwrap().clone();
            let prev = if prev.device.as_ref().map(|d| (&d.ip, d.port)) == Some((&dev.ip, dev.port)) {
                prev
            } else {
                last_mode = Value::Null;
                Snapshot::default()
            };
            let wait = Duration::from_secs(s.timeout_sec.into());
            let snap = poll_once(&dev, wait, full, prev, &mut last_mode).await;
            if full && snap.online {
                last_full = Some(Instant::now());
            } else if full {
                shared.full_poll.store(true, Ordering::SeqCst);
            }
            // The user may have picked another device while this poll ran.
            let current = shared.settings().device;
            if current.as_ref().map(|d| (&d.ip, d.port)) == Some((&dev.ip, dev.port)) {
                // Keep the saved model name and firmware version in step with the device.
                if let Some(live) = snap.device.clone().filter(|d| d.name != dev.name || d.ver != dev.ver) {
                    if let Ok(after) = shared.update(|s| s.device = Some(live)) {
                        let _ = app.emit("settings", after);
                    }
                }
                publish(&app, snap);
            }
        } else {
            publish(&app, Snapshot::default());
        }
        let refresh = Duration::from_secs(shared.settings().refresh_sec.into());
        tokio::select! {
            _ = tokio::time::sleep(refresh) => {}
            _ = shared.wake.notified() => {}
        }
    }
}

// ---------------------------------------------------------------- commands

#[tauri::command]
fn get_settings(shared: State<'_, Shared>) -> Settings {
    shared.settings()
}

#[tauri::command]
fn get_snapshot(shared: State<'_, Shared>) -> Snapshot {
    shared.snapshot.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, shared: State<'_, Shared>, patch: SettingsPatch) -> Result<Settings, String> {
    let before = shared.settings();
    let after = shared.update(|s| s.apply(patch))?;
    let _ = app.emit("settings", after.clone());
    if before.lang != after.lang {
        rebuild_tray_menu(&app);
        if let Some(w) = app.get_webview_window("settings") {
            let _ = w.set_title(tr(&after.lang, "settings"));
        }
        refresh_tray(&app);
    }
    if before.refresh_sec != after.refresh_sec || before.mode_check_sec != after.mode_check_sec {
        shared.poll_now(false);
    }
    Ok(after)
}

#[tauri::command]
async fn discover(port: Option<u16>) -> Result<Vec<marstek::Found>, String> {
    marstek::discover(port.unwrap_or(marstek::DEFAULT_PORT), DISCOVER_WAIT).await
}

#[tauri::command]
async fn connect(app: AppHandle, shared: State<'_, Shared>, ip: String, port: u16) -> Result<Settings, String> {
    let ip = ip.trim().to_string();
    let wait = Duration::from_secs(shared.settings().timeout_sec.into());
    let info = marstek::call(&ip, port, "Marstek.GetDevice", json!({ "ble_mac": "0" }), wait).await?;
    let dev = DeviceRef {
        ip,
        port,
        name: info.get("device").and_then(Value::as_str).map(String::from),
        ver: info.get("ver").and_then(Value::as_u64).map(|v| v as u32),
    };
    let after = shared.update(|s| s.device = Some(dev.clone()))?;
    publish(&app, Snapshot { device: Some(dev), ..Default::default() });
    let _ = app.emit("settings", after.clone());
    shared.poll_now(true);
    Ok(after)
}

/// Switches the device to Passive mode, charging from the grid at `power` W.
#[tauri::command]
async fn set_charge_power(app: AppHandle, shared: State<'_, Shared>, power: u32) -> Result<Settings, String> {
    if !(CHARGE_MIN..=CHARGE_MAX).contains(&power) {
        return Err(format!("range {CHARGE_MIN}-{CHARGE_MAX}"));
    }
    let s = shared.settings();
    let dev = s.device.ok_or("no device")?;
    let wait = Duration::from_secs(s.timeout_sec.into());
    let config = json!({
        "mode": "Passive",
        // Negative power means charge.
        "passive_cfg": { "power": -(power as i64), "cd_time": PASSIVE_SECONDS }
    });
    let result = marstek::call(&dev.ip, dev.port, "ES.SetMode", json!({ "id": 0, "config": config }), wait).await?;
    if result.get("set_result").and_then(Value::as_bool) == Some(false) {
        return Err("rejected".into());
    }
    let after = shared.update(|s| s.charge_power = power)?;
    let _ = app.emit("settings", after.clone());
    shared.poll_now(true);
    Ok(after)
}

/// Must be async: creating a window from a sync command deadlocks on Windows.
#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<(), String> {
    show_settings(&app)
}

#[tauri::command]
fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

// ---------------------------------------------------------------- app

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN_ARG])))
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("settings.json");
            let settings = Settings::load(&path);
            let handle = app.handle().clone();

            if !settings.autostart_initialized {
                let _ = handle.autolaunch().enable();
            }
            app.manage(Shared {
                path,
                settings: Mutex::new(settings),
                snapshot: Mutex::new(Snapshot::default()),
                wake: Notify::new(),
                full_poll: AtomicBool::new(true),
            });
            let _ = app.state::<Shared>().update(|s| s.autostart_initialized = true);

            let lang = app.state::<Shared>().settings().lang;
            let tray: TrayIcon = TrayIconBuilder::with_id(TRAY_ID)
                .icon(app.default_window_icon().cloned().expect("bundled icon"))
                .tooltip("MarsWin")
                .menu(&build_menu(&handle, &lang)?)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        toggle_main(tray.app_handle());
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "settings" => {
                        let _ = show_settings(app);
                    }
                    "autostart" => {
                        let al = app.autolaunch();
                        let _ = if al.is_enabled().unwrap_or(false) { al.disable() } else { al.enable() };
                        rebuild_tray_menu(app);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            drop(tray);

            let main = app.get_webview_window("main").expect("main window");
            resize_to_fit(&main, true);
            if !std::env::args().any(|a| a == HIDDEN_ARG) {
                main.show()?;
                main.set_focus()?;
            }

            tauri::async_runtime::spawn(poller(handle));
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the main window keeps MarsWin in the tray.
            WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                api.prevent_close();
                let _ = window.hide();
            }
            // Moved to a monitor with other scaling or size: fit it again.
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(w) = window.app_handle().get_webview_window(window.label()) {
                    resize_to_fit(&w, false);
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            get_snapshot,
            save_settings,
            discover,
            connect,
            set_charge_power,
            open_settings,
            app_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running MarsWin");
}
