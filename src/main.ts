import "./styles.css";
import "./main.css";
import { api, fitZoom, onSettings, onSnapshot, toast, win, type Settings, type Snapshot } from "./api";
import { errorText, setLang, t, translatePage } from "./i18n";
import { barWidth, percent, statusOf, watts } from "./view";

const $ = (id: string) => document.getElementById(id)!;

let settings: Settings | null = null;
let snapshot: Snapshot | null = null;

const kwh = (wh: number | null) => (wh == null ? "—" : (wh / 1000).toFixed(1));

function render(): void {
  const s = snapshot;
  if (!s) return;
  $("soc").textContent = percent(s.soc);
  $("bar").style.width = barWidth(s.soc);
  $("energy").textContent = `${kwh(s.energyWh)} / ${kwh(s.ratedWh)} kWh`;

  const status = statusOf(s, !!settings?.device);
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
