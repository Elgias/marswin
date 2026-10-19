import "./styles.css";
import "./settings.css";
import {
  api,
  CHARGE_MAX,
  CHARGE_MIN,
  fitZoom,
  onSettings,
  onSnapshot,
  toast,
  win,
  type Found,
  type Lang,
  type Settings,
  type Snapshot,
} from "./api";
import { errorText, seconds, setLang, t, translatePage } from "./i18n";
import { Select } from "./select";

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const REFRESH = [5, 10, 15, 30, 60];
const MODE_CHECK = [30, 60, 300, 600, 1800];
const TIMEOUT = [2, 3, 5, 10];
const IPV4 = /^(25[0-5]|2[0-4]\d|1?\d?\d)(\.(25[0-5]|2[0-4]\d|1?\d?\d)){3}$/;

let settings: Settings;
let snapshot: Snapshot | null = null;
let found: Found[] = [];
let scanning = false;

const save = (patch: Parameters<typeof api.saveSettings>[0]) =>
  api.saveSettings(patch).then(applySettings, (e) => toast(t("failed", { e: errorText(e) }), true));

const langSelect = new Select<Lang>($("lang"), (lang) => void save({ lang }));
const refreshSelect = new Select<number>($("refresh"), (refreshSec) => void save({ refreshSec }));
const modeSelect = new Select<number>($("mode-check"), (modeCheckSec) => void save({ modeCheckSec }));
const timeoutSelect = new Select<number>($("timeout"), (timeoutSec) => void save({ timeoutSec }));

const isCurrent = (d: { ip: string; port: number }) =>
  settings?.device?.ip === d.ip && settings.device.port === d.port;

function renderSelects(): void {
  langSelect.setOptions(
    [
      { value: "uk", label: "Українська" },
      { value: "en", label: "English" },
    ],
    settings.lang,
  );
  const secs = (list: number[]) => list.map((n) => ({ value: n, label: seconds(n) }));
  refreshSelect.setOptions(secs(REFRESH), settings.refreshSec);
  modeSelect.setOptions(secs(MODE_CHECK), settings.modeCheckSec);
  timeoutSelect.setOptions(secs(TIMEOUT), settings.timeoutSec);
}

function deviceCard(d: Found): HTMLButtonElement {
  const card = document.createElement("button");
  card.type = "button";
  card.className = "device";
  card.classList.toggle("selected", isCurrent(d));
  const name = document.createElement("span");
  name.className = "device-name";
  name.textContent = d.name ?? "Marstek";
  const meta = document.createElement("span");
  meta.className = "device-meta muted";
  const addr = document.createElement("span");
  addr.textContent = `${d.ip}:${d.port}`;
  const ver = document.createElement("span");
  ver.textContent = d.ver == null ? "" : `v${d.ver}`;
  meta.append(addr, ver);
  card.append(name, meta);
  card.addEventListener("click", () => void connect(d.ip, d.port));
  return card;
}

function renderDevices(): void {
  const box = $("devices");
  box.innerHTML = "";
  // The saved device stays listed even when the scan misses it.
  const list = [...found];
  if (settings?.device && !list.some(isCurrent)) list.unshift(settings.device);
  for (const d of list) box.append(deviceCard(d));
  // While a scan runs over a non-empty list, the spinning icon is the only hint.
  if (list.length === 0) {
    const note = document.createElement("div");
    note.className = "note muted";
    note.textContent = scanning ? t("searching") : t("nothingFound");
    box.append(note);
  }
}

function renderConnection(): void {
  const s = snapshot;
  const dev = s?.device ?? settings?.device ?? null;
  const live = s?.online ? s : null;
  $("c-ssid").textContent = live?.wifi?.ssid ?? "—";
  $("c-rssi").textContent = live?.wifi?.rssi == null ? "—" : `${live.wifi.rssi} dBm`;
  $("c-ip").textContent = live?.wifi?.ip ?? dev?.ip ?? "—";
  $("c-device").textContent = dev ? [dev.name ?? "Marstek", dev.ver == null ? "" : `v${dev.ver}`].join(" ").trim() : "—";
}

function applySettings(next: Settings): void {
  const langChanged = !settings || settings.lang !== next.lang;
  settings = next;
  if (langChanged) {
    setLang(next.lang);
    translatePage();
    $("charge-range").textContent = t("chargeRange", { min: CHARGE_MIN, max: CHARGE_MAX });
  }
  renderSelects();
  const charge = $<HTMLInputElement>("charge");
  if (document.activeElement !== charge) charge.value = String(next.chargePower);
  $<HTMLButtonElement>("charge-apply").disabled = !next.device;
  renderDevices();
  renderConnection();
}

async function scan(): Promise<void> {
  if (scanning) return;
  scanning = true;
  $("rescan").classList.add("spinning");
  renderDevices();
  try {
    const port = Number($<HTMLInputElement>("port").value) || undefined;
    found = await api.discover(port);
  } catch (e) {
    toast(t("failed", { e: errorText(e) }), true);
  } finally {
    scanning = false;
    $("rescan").classList.remove("spinning");
    renderDevices();
  }
}

async function connect(ip: string, port: number): Promise<void> {
  const button = $<HTMLButtonElement>("connect");
  button.disabled = true;
  try {
    const next = await api.connect(ip, port);
    applySettings(next);
    toast(t("connected", { name: next.device?.name ?? ip }));
  } catch (e) {
    toast(t("failed", { e: errorText(e) }), true);
  } finally {
    button.disabled = false;
  }
}

$("close").addEventListener("click", () => void win.close());
$("rescan").addEventListener("click", () => void scan());

$("manual-form").addEventListener("submit", (e) => {
  e.preventDefault();
  const ipEl = $<HTMLInputElement>("ip");
  const portEl = $<HTMLInputElement>("port");
  const ip = ipEl.value.trim();
  const port = Number(portEl.value);
  const ipOk = IPV4.test(ip);
  const portOk = Number.isInteger(port) && port > 0 && port < 65536;
  ipEl.classList.toggle("invalid", !ipOk);
  portEl.classList.toggle("invalid", !portOk);
  if (!ipOk) return toast(t("badIp"), true);
  if (!portOk) return toast(t("badPort"), true);
  void connect(ip, port);
});

for (const id of ["ip", "port", "charge"]) {
  $(id).addEventListener("input", (e) => (e.target as HTMLElement).classList.remove("invalid"));
}

$("charge-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const input = $<HTMLInputElement>("charge");
  const power = Number(input.value);
  if (!Number.isInteger(power) || power < CHARGE_MIN || power > CHARGE_MAX) {
    input.classList.add("invalid");
    return toast(t("badPower", { min: CHARGE_MIN, max: CHARGE_MAX }), true);
  }
  const button = $<HTMLButtonElement>("charge-apply");
  button.disabled = true;
  toast(t("chargeSending"));
  try {
    applySettings(await api.setChargePower(power));
    input.blur();
    toast(t("chargeDone", { w: power }));
  } catch (e) {
    toast(t("failed", { e: errorText(e) }), true);
  } finally {
    button.disabled = !settings.device;
  }
});

fitZoom(1128);
onSettings(applySettings);
onSnapshot((s) => {
  snapshot = s;
  renderConnection();
});

const [first, firstSnap, version] = await Promise.all([api.getSettings(), api.getSnapshot(), api.appVersion()]);
snapshot = firstSnap;
$("version").textContent = `v${version}`;
applySettings(first);
void scan();
