import "./styles.css";
import "./widget.css";
import "./motion.css";
import { api, fitZoom, onFade, onSettings, onSnapshot, win, type Settings, type Snapshot } from "./api";
import { setLang, translatePage } from "./i18n";
import { enter, leave, roll, seed, swap } from "./motion";
import { PaceRing } from "./pace";
import { barWidth, percent, statusOf, watts } from "./view";

const $ = (id: string) => document.getElementById(id)!;

let settings: Settings | null = null;
let snapshot: Snapshot | null = null;

function render(): void {
  const s = snapshot;
  if (!s) return;
  roll($("soc"), s.soc, percent);
  $("bar").style.width = barWidth(s.soc);
  roll($("input"), s.inputW, watts);
  roll($("output"), s.outputW, watts);
  const live = s.online;
  $("input").parentElement!.classList.toggle("active", live && (s.inputW ?? 0) > 0);
  $("output").parentElement!.classList.toggle("active", live && (s.outputW ?? 0) > 0);
  const status = statusOf(s, !!settings?.device);
  swap($("status"), status.text);
  $("status").dataset.flow = status.flow;
  $("bar").dataset.flow = status.flow;
  document.body.classList.toggle("stale", !s.online);
}

const pace = new PaceRing($("pace") as HTMLButtonElement);

function applySettings(next: Settings): void {
  settings = next;
  setLang(next.lang);
  translatePage();
  pace.title();
  render();
}

$("expand").addEventListener("click", () => {
  void win.hide();
  void api.openMain();
});
onFade(enter, () => void leave());
$("close").addEventListener("click", () => void win.hide());

fitZoom(545);
onSnapshot((s) => {
  snapshot = s;
  render();
});
onSettings(applySettings);

const [first, firstSnap] = await Promise.all([api.getSettings(), api.getSnapshot()]);
snapshot = firstSnap;
for (const id of ["soc", "input", "output"]) seed($(id), 0);
await new Promise(requestAnimationFrame);
applySettings(first);
