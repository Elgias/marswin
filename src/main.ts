import "./styles.css";
import "./main.css";
import "./motion.css";
import { api, fitZoom, onFade, onSettings, onSnapshot, toast, win, type Settings, type Snapshot } from "./api";
import { errorText, setLang, t, translatePage } from "./i18n";
import { enter, leave, roll, seed, swap } from "./motion";
import { PaceRing } from "./pace";
import { barWidth, percent, statusOf, watts } from "./view";

const $ = (id: string) => document.getElementById(id)!;

let settings: Settings | null = null;
let snapshot: Snapshot | null = null;

const kwh = (wh: number | null) => (wh == null ? "—" : (wh / 1000).toFixed(1));
const celsius = (c: number | null) => (c == null ? "—" : `${Math.round(c)}°C`);

function render(): void {
  const s = snapshot;
  if (!s) return;
  roll($("soc"), s.soc, percent);
  $("bar").style.width = barWidth(s.soc);
  $("energy").textContent = `${kwh(s.energyWh)} / ${kwh(s.ratedWh)} kWh`;

  const status = statusOf(s, !!settings?.device);
  swap($("status"), status.text);
  $("status").dataset.flow = status.flow;
  $("bar").dataset.flow = status.flow;
  // Without a link the last values are kept but dimmed.
  document.body.classList.toggle("stale", !s.online);

  roll($("input"), s.inputW, watts);
  roll($("output"), s.outputW, watts);
  roll($("temp"), s.tempC, celsius);
  swap($("mode"), s.mode ?? "—");
}

const pace = new PaceRing($("pace") as HTMLButtonElement);

function applySettings(next: Settings): void {
  settings = next;
  setLang(next.lang);
  translatePage();
  pace.title();
  render();
}

$("minimize").addEventListener("click", () => void win.minimize());
// Closing hides MarsWin to the tray; "Quit" lives in the tray menu.
$("close").addEventListener("click", () => void win.hide());
onFade(enter, () => void leave());
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
// On start the charge counts up and the bar fills from empty.
for (const id of ["soc", "input", "output"]) seed($(id), 0);
enter();
await new Promise(requestAnimationFrame);
applySettings(first);
