import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

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
  wifi: { ssid: string | null; rssi: number | null; ip: string | null } | null;
}

export interface Found {
  ip: string;
  port: number;
  name: string | null;
  ver: number | null;
}

export type SettingsPatch = Partial<Pick<Settings, "lang" | "refreshSec" | "modeCheckSec" | "timeoutSec">>;

/** Outside Tauri (plain `npm run dev` in a browser) the app runs on mock data. */
const inTauri = "__TAURI_INTERNALS__" in window;

const mockDevice: DeviceRef = { ip: "192.168.1.101", port: 30000, name: "VNSEM-0", ver: 301 };
let mockAutostart = true;
let mockSettings: Settings = {
  lang: "uk",
  refreshSec: 30,
  modeCheckSec: 60,
  timeoutSec: 5,
  device: mockDevice,
};
const mockSnapshot: Snapshot = {
  online: true,
  error: null,
  device: mockDevice,
  soc: 94.4,
  energyWh: 1888,
  ratedWh: 2000,
  inputW: 0,
  outputW: 129,
  tempC: 27,
  mode: "UPS",
  flow: "discharging",
  wifi: { ssid: "MERQ", rssi: -31, ip: "192.168.1.101" },
};

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
    case "app_version":
      return "1.0.0" as T;
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
};

export function onSnapshot(cb: (s: Snapshot) => void): void {
  if (inTauri) void listen<Snapshot>("snapshot", (e) => cb(e.payload));
}

/** Autostart was switched elsewhere (tray menu or another window). */
export function onAutostart(cb: (enabled: boolean) => void): void {
  if (inTauri) void listen<boolean>("autostart", (e) => cb(e.payload));
}

export function onSettings(cb: (s: Settings) => void): void {
  if (inTauri) void listen<Settings>("settings", (e) => cb(e.payload));
}

export const win = {
  minimize: () => (inTauri ? getCurrentWindow().minimize() : Promise.resolve()),
  hide: () => (inTauri ? getCurrentWindow().hide() : Promise.resolve()),
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
