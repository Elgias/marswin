import "./styles.css";
import "./main.css";
import { api, fitZoom, onSettings, onSnapshot, toast, win, type Settings, type Snapshot } from "./api";
import { errorText, setLang, t, translatePage } from "./i18n";

const $ = (id: string) => document.getElementById(id)!;

let settings: Settings | null = null;
let snapshot: Snapshot | null = null;

const watts = (w: number | null) => (w == null ? "—" : `${Math.round(w)} W`);
const kwh = (wh: number | null) => (wh == null ? "—" : (wh / 1000).toFixed(1));

function statusOf(s: Snapshot): { text: string; flow: string } {
  if (!settings?.device) return { text: t("noDevice"), flow: "none" };
  if (!s.online) return s.error ? { text: t("offline"), flow: "offline" } : { text: t("connecting"), flow: "none" };
  return { text: t(s.flow), flow: s.flow };
}

function render(): void {
  const s = snapshot;
  if (!s) return;
  const soc = s.soc;
  $("soc").textContent = soc == null ? "—" : `${soc.toFixed(1)}%`;
  ($("bar") as HTMLDivElement).style.width = `${Math.max(0, Math.min(100, soc ?? 0))}%`;
  $("energy").textContent = `${kwh(s.energyWh)} / ${kwh(s.ratedWh)} kWh`;

  const status = statusOf(s);
  $("status").textContent = status.text;
  $("status").dataset.flow = status.flow;
  // Without a link the last values are kept but dimmed.
  document.body.classList.toggle("stale", !s.online);

  $("input").textContent = watts(s.inputW);
  $("output").textContent = watts(s.outputW);
  $("temp").textContent = s.tempC == null ? "—" : `${Math.round(s.tempC)}°C`;
  $("mode").textContent = s.mode ?? "—";
}

function applySettings(next: Settings): void {
  settings = next;
  setLang(next.lang);
  translatePage();
  render();
}

$("minimize").addEventListener("click", () => void win.minimize());
// Closing hides MarsWin to the tray; "Quit" lives in the tray menu.
$("close").addEventListener("click", () => void win.hide());
$("open-settings").addEventListener("click", () =>
  api.openSettings().catch((e) => toast(t("failed", { e: errorText(e) }), true)),
);

fitZoom(565);
onSnapshot((s) => {
  snapshot = s;
  render();
});
onSettings(applySettings);

const [first, firstSnap] = await Promise.all([api.getSettings(), api.getSnapshot()]);
snapshot = firstSnap;
applySettings(first);
// Enable the bar animation only after the first fill has been painted.
setTimeout(() => document.body.classList.add("ready"), 100);
