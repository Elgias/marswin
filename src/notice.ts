import "./styles.css";
import "./notice.css";
import "./motion.css";
import { api, fitZoom, onFade, onNotice, onSettings, win, type Notice } from "./api";
import { setLang, t, translatePage } from "./i18n";
import { enter, leave } from "./motion";

/** Low-charge notice by the tray (Rust: alerts.rs decides, lib.rs places and hides it after 5 s). */

const $ = (id: string) => document.getElementById(id)!;

let notice: Notice | null = null;

function render(): void {
  if (!notice) return;
  $("text").textContent = t("socLeft", { n: notice.soc });
  $("notice").dataset.level = notice.level;
}

/** Draws the notice, then asks Rust to show it; a newer notice replaces the one on screen. */
function present(next: Notice): void {
  notice = next;
  render();
  enter();
  void api.presentNotice();
}

$("close").addEventListener("click", () => void win.hide());
onFade(enter, () => void leave());
onSettings((s) => {
  setLang(s.lang);
  translatePage();
  render();
});
onNotice(present);

fitZoom(545);
const [settings, first] = await Promise.all([api.getSettings(), api.getNotice()]);
setLang(settings.lang);
translatePage();
// The window is made for the first notice: show that one.
if (first) present(first);
