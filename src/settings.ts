import "./styles.css";
import "./settings.css";
import "./motion.css";
import {
  api,
  fitZoom,
  onAutostart,
  onFade,
  onUpdate,
  onUpdateFailed,
  onSettings,
  onSnapshot,
  toast,
  win,
  type Found,
  type Lang,
  type Settings,
  type Snapshot,
  type UpdateInfo,
} from "./api";
import { errorText, seconds, setLang, t, translatePage } from "./i18n";
import { enter, leave, swap } from "./motion";
import { Select } from "./select";

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const REFRESH = [0, 5, 10, 15, 30, 60]; // 0 = adaptive
const MODE_CHECK = [30, 60, 300, 600, 1800];
const TIMEOUT = [2, 3, 5, 10];
const IPV4 = /^(25[0-5]|2[0-4]\d|1?\d?\d)(\.(25[0-5]|2[0-4]\d|1?\d?\d)){3}$/;

let settings: Settings;
let snapshot: Snapshot | null = null;
let found: Found[] = [];
let scanning = false;
let shownList = "";
let update: UpdateInfo | null = null;
let updateBusy: "checking" | "installing" | null = null;

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
  refreshSelect.setOptions(
    REFRESH.map((n) => ({ value: n, label: n === 0 ? t("auto") : seconds(n) })),
    settings.refreshSec,
  );
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
  // A new set of devices rises into place; a re-render of the same set stays still.
  const key = list.map((d) => `${d.ip}:${d.port}`).join() + (list.length ? "" : scanning);
  box.classList.toggle("appear", key !== shownList);
  if (key !== shownList) {
    box.querySelectorAll<HTMLElement>(":scope > *").forEach((c, i) => c.style.setProperty("--i", String(i)));
    shownList = key;
  }
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
  swap($("c-ssid"), live?.wifi?.ssid ?? "—");
  swap($("c-rssi"), live?.wifi?.rssi == null ? "—" : `${live.wifi.rssi} dBm`);
  swap($("c-ip"), live?.wifi?.ip ?? dev?.ip ?? "—");
  swap($("c-device"), dev ? [dev.name ?? "Marstek", dev.ver == null ? "" : `v${dev.ver}`].join(" ").trim() : "—");
}

function applySettings(next: Settings): void {
  const langChanged = !settings || settings.lang !== next.lang;
  settings = next;
  if (langChanged) {
    setLang(next.lang);
    translatePage();
  }
  renderSelects();
  renderDevices();
  renderConnection();
  renderUpdate();
}

/** Footer link: "Check for updates", or "Update to 1.4.0" once one is found. */
function renderUpdate(): void {
  const link = $<HTMLButtonElement>("update");
  link.disabled = updateBusy !== null;
  link.classList.toggle("found", !!update && !updateBusy);
  link.textContent = updateBusy
    ? t(updateBusy)
    : update
      ? t(update.canInstall ? "installUpdate" : "downloadUpdate", { v: update.version })
      : t("checkUpdate");
}

async function onUpdateClick(): Promise<void> {
  updateBusy = update?.canInstall ? "installing" : update ? null : "checking";
  renderUpdate();
  try {
    if (update) {
      // The installer closes MarsWin and starts the new version.
      await api.installUpdate();
    } else {
      update = await api.checkUpdate();
      if (!update) toast(t("upToDate"));
    }
  } catch (e) {
    toast(t("failed", { e: errorText(e) }), true);
  } finally {
    updateBusy = null;
    renderUpdate();
  }
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

$("close").addEventListener("click", () => void leave().then(win.close));
onFade(enter, () => void leave());

const autostart = $<HTMLInputElement>("autostart");
autostart.addEventListener("change", async () => {
  const wanted = autostart.checked;
  autostart.disabled = true;
  try {
    autostart.checked = await api.setAutostart(wanted);
  } catch (e) {
    autostart.checked = !wanted;
    toast(t("failed", { e: errorText(e) }), true);
  } finally {
    autostart.disabled = false;
  }
});
onAutostart((on) => (autostart.checked = on));
$("rescan").addEventListener("click", () => void scan());
$("update").addEventListener("click", () => void onUpdateClick());
onUpdate((u) => {
  update = u;
  renderUpdate();
});
onUpdateFailed((e) => toast(t("failed", { e: errorText(e) }), true));

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

for (const id of ["ip", "port"]) {
  $(id).addEventListener("input", (e) => (e.target as HTMLElement).classList.remove("invalid"));
}

fitZoom(1128);
onSettings(applySettings);
onSnapshot((s) => {
  snapshot = s;
  renderConnection();
});

const [first, firstSnap, version, autostartOn, firstUpdate] = await Promise.all([
  api.getSettings(),
  api.getSnapshot(),
  api.appVersion(),
  api.getAutostart(),
  api.getUpdate(),
]);
update = firstUpdate;
autostart.checked = autostartOn;
snapshot = firstSnap;
$("version").textContent = `v${version}`;
applySettings(first);
enter();
void scan();
