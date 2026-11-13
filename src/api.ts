import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

export type Lang = "uk" | "en";
export type Flow = "idle" | "charging" | "discharging";

export interface DeviceRef {
  ip: string;
  port: number;
  name: string | null;
  ver: number | null;
}

export interface Settings {
  lang: Lang;
  refreshSec: number;
  modeCheckSec: number;
  timeoutSec: number;
  device: DeviceRef | null;
  /** Low-charge notices: level in % and on/off (alerts.rs). */
  warnPct: number;
  warnOn: boolean;
  criticalPct: number;
  criticalOn: boolean;
}

export interface Snapshot {
  online: boolean;
  error: string | null;
  device: DeviceRef | null;
  soc: number | null;
  energyWh: number | null;
  ratedWh: number | null;
  inputW: number | null;
  outputW: number | null;
  tempC: number | null;
  mode: string | null;
  flow: Flow;
  /** Minutes to empty (discharging) or to full (charging) at the present rate; set from the
   *  3rd poll in a row in the same direction (snapshot.rs, Eta). */
  etaMin: number | null;
  wifi: { ssid: string | null; rssi: number | null; ip: string | null } | null;
}

/** A low-charge notice (alerts.rs): "{soc}% left" with a yellow or red sign. */
export interface Notice {
  level: "warning" | "critical";
  soc: number;
}

export interface Found {
  ip: string;
  port: number;
  name: string | null;
  ver: number | null;
}

/** A newer release on GitHub (update.rs). The portable exe cannot install it, only open the page. */
export interface UpdateInfo {
  version: string;
  canInstall: boolean;
}

/** Poller state for the countdown ring (lib.rs, Pace). */
export type Pace = { state: "idle" } | { state: "busy" } | { state: "wait"; leftMs: number; totalMs: number };

export type SettingsPatch = Partial<Omit<Settings, "device">>;

/** Outside Tauri (plain `npm run dev` in a browser) the app runs on mock data. */
const inTauri = "__TAURI_INTERNALS__" in window;

// MarsWin is an app, not a web page: no browser menu on right click, no reload, print or find.
if (inTauri) {
  document.addEventListener("contextmenu", (e) => e.preventDefault());
  document.addEventListener("keydown", (e) => {
    const key = e.key.toLowerCase();
    if (key === "f5" || key === "f3" || (e.ctrlKey && ["r", "p", "f", "g", "j", "u"].includes(key))) e.preventDefault();
  });
}

// Mock variants for the browser: ?lang=en, ?level=critical (notice.html), ?eta=none, ?flow=charging.
const mockQuery = new URLSearchParams(location.search);
const mockCharging = mockQuery.get("flow") === "charging";
const mockDevice: DeviceRef = { ip: "192.168.1.101", port: 30000, name: "VNSEM-0", ver: 301 };
let mockAutostart = true;
let mockSettings: Settings = {
  lang: mockQuery.get("lang") === "en" ? "en" : "uk",
  refreshSec: 0,
  modeCheckSec: 60,
  timeoutSec: 5,
  device: mockDevice,
  warnPct: 50,
  warnOn: true,
  criticalPct: 20,
  criticalOn: true,
};
const mockSnapshot: Snapshot = {
  online: true,
  error: null,
  device: mockDevice,
  soc: 94.4,
  energyWh: 1888,
  ratedWh: 2000,
  inputW: mockCharging ? 800 : 0,
  outputW: mockCharging ? 0 : 129,
  tempC: 27,
  mode: "UPS",
  flow: mockCharging ? "charging" : "discharging",
  etaMin: mockQuery.get("eta") === "none" ? null : mockCharging ? 8 : 519,
  wifi: { ssid: "MERQ", rssi: -31, ip: "192.168.1.101" },
};
const mockNotice: Notice =
  mockQuery.get("level") === "critical" ? { level: "critical", soc: 20 } : { level: "warning", soc: 50 };

async function mock<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  await new Promise((r) => setTimeout(r, 300));
  switch (cmd) {
    case "get_settings":
      return mockSettings as T;
    case "get_snapshot":
      return mockSnapshot as T;
    case "save_settings":
      mockSettings = { ...mockSettings, ...(args.patch as SettingsPatch) };
      return mockSettings as T;
    case "discover":
      return [{ ...mockDevice }] as T;
    case "connect":
      mockSettings = { ...mockSettings, device: { ...mockDevice, ip: args.ip as string, port: args.port as number } };
      return mockSettings as T;
    case "get_autostart":
      return mockAutostart as T;
    case "set_autostart":
      mockAutostart = args.enabled as boolean;
      return mockAutostart as T;
    case "get_update":
      return null as T;
    case "check_update":
      return { version: "1.4.0", canInstall: true } as T;
    case "get_pace":
      return { state: "wait", leftMs: 20000, totalMs: 30000 } as T;
    case "app_version":
      return "1.0.0" as T;
    case "get_notice":
      return mockNotice as T;
    default:
      return undefined as T;
  }
}

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return inTauri ? invoke<T>(cmd, args) : mock<T>(cmd, args);
}

export const api = {
  getSettings: () => call<Settings>("get_settings"),
  getSnapshot: () => call<Snapshot>("get_snapshot"),
  saveSettings: (patch: SettingsPatch) => call<Settings>("save_settings", { patch }),
  discover: (port?: number) => call<Found[]>("discover", { port }),
  connect: (ip: string, port: number) => call<Settings>("connect", { ip, port }),
  openSettings: () => call<void>("open_settings"),
  openMain: () => call<void>("open_main"),
  getAutostart: () => call<boolean>("get_autostart"),
  setAutostart: (enabled: boolean) => call<boolean>("set_autostart", { enabled }),
  appVersion: () => call<string>("app_version"),
  getPace: () => call<Pace>("get_pace"),
  refreshNow: () => call<void>("refresh_now"),
  getUpdate: () => call<UpdateInfo | null>("get_update"),
  checkUpdate: () => call<UpdateInfo | null>("check_update"),
  installUpdate: () => call<void>("install_update"),
  getNotice: () => call<Notice | null>("get_notice"),
  /** The notice is drawn: Rust places it by the tray, shows it and hides it after 5 s. */
  presentNotice: () => call<void>("present_notice"),
};

/** A new low-charge notice for the notice window (lib.rs, notify). */
export function onNotice(cb: (n: Notice) => void): void {
  if (inTauri) void getCurrentWebviewWindow().listen<Notice>("notice", (e) => cb(e.payload));
}

export function onSnapshot(cb: (s: Snapshot) => void): void {
  if (inTauri) void listen<Snapshot>("snapshot", (e) => cb(e.payload));
}

/** Autostart was switched elsewhere (tray menu or another window). */
export function onAutostart(cb: (enabled: boolean) => void): void {
  if (inTauri) void listen<boolean>("autostart", (e) => cb(e.payload));
}

export function onUpdate(cb: (u: UpdateInfo | null) => void): void {
  if (inTauri) void listen<UpdateInfo | null>("update", (e) => cb(e.payload));
}

export function onUpdateFailed(cb: (error: string) => void): void {
  if (inTauri) void listen<string>("update-failed", (e) => cb(e.payload));
}

export function onPace(cb: (p: Pace) => void): void {
  if (inTauri) void listen<Pace>("pace", (e) => cb(e.payload));
}

export function onSettings(cb: (s: Settings) => void): void {
  if (inTauri) void listen<Settings>("settings", (e) => cb(e.payload));
}

/** Rust sends "enter" before it shows this window and "leave" before it hides it (lib.rs, fade). */
export function onFade(enter: () => void, leave: () => void): void {
  if (!inTauri) return;
  const w = getCurrentWebviewWindow();
  void w.listen("enter", enter);
  void w.listen("leave", leave);
}

export const win = {
  minimize: () => (inTauri ? getCurrentWindow().minimize() : Promise.resolve()),
  /** Fades out, then hides (Rust sends "leave" first). */
  hide: () => call<void>("hide_window"),
  close: () => (inTauri ? getCurrentWindow().close() : Promise.resolve()),
};

/** Scales the design to the window width; Rust sizes the window per screen (fit.rs). */
export function fitZoom(designWidth: number): void {
  const apply = () => {
    const zoom = Math.min(1, window.innerWidth / designWidth);
    document.documentElement.style.setProperty("--zoom", String(zoom));
  };
  apply();
  window.addEventListener("resize", apply);
}

let toastTimer = 0;
export function toast(message: string, error = false): void {
  let el = document.querySelector<HTMLDivElement>(".toast");
  if (!el) {
    el = document.createElement("div");
    el.className = "toast";
    // Inside the zoomed frame, so the toast scales with the rest of the page.
    (document.querySelector(".frame") ?? document.body).append(el);
  }
  el.textContent = message;
  el.classList.toggle("error", error);
  el.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => el.classList.remove("show"), 3500);
}
