import "./styles.css";
import "./widget.css";
import { api, fitZoom, onSettings, onSnapshot, win, type Settings, type Snapshot } from "./api";
import { setLang, translatePage } from "./i18n";
import { barWidth, percent, statusOf, watts } from "./view";

const $ = (id: string) => document.getElementById(id)!;

let settings: Settings | null = null;
let snapshot: Snapshot | null = null;

function render(): void {
  const s = snapshot;
  if (!s) return;
  $("soc").textContent = percent(s.soc);
  $("bar").style.width = barWidth(s.soc);
  $("input").textContent = watts(s.inputW);
  $("output").textContent = watts(s.outputW);
  const status = statusOf(s, !!settings?.device);
  $("status").textContent = status.text;
  $("status").dataset.flow = status.flow;
  document.body.classList.toggle("stale", !s.online);
}

function applySettings(next: Settings): void {
  settings = next;
  setLang(next.lang);
  translatePage();
  render();
}

$("expand").addEventListener("click", () => {
  void api.openMain();
  void win.hide();
});
$("close").addEventListener("click", () => void win.hide());

fitZoom(545);
onSnapshot((s) => {
  snapshot = s;
  render();
});
onSettings(applySettings);

const [first, firstSnap] = await Promise.all([api.getSettings(), api.getSnapshot()]);
snapshot = firstSnap;
applySettings(first);
setTimeout(() => document.body.classList.add("ready"), 100);
