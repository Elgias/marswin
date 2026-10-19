import { api, onPace, type Pace } from "./api";
import { t } from "./i18n";

/** The spinner stays at least this long, so a quick poll still reads as "updating". */
const MIN_BUSY_MS = 600;

/**
 * Countdown ring next to the window buttons: the arc shrinks until the next poll,
 * spins while a poll runs, and a click polls at once. Markup: `.pace` button holding
 * an SVG with `.pace-track` and `.pace-arc` circles (pathLength 100).
 */
export class PaceRing {
  private arc: SVGCircleElement;
  private due = 0;
  private busySince = 0;
  private pending = 0;

  constructor(private el: HTMLButtonElement) {
    this.arc = el.querySelector(".pace-arc")!;
    el.addEventListener("click", () => void api.refreshNow());
    // The tooltip counts down while the pointer is on the ring.
    window.setInterval(() => this.title(), 1000);
    onPace((p) => this.set(p));
    void api.getPace().then((p) => this.set(p));
  }

  /** Call after a language change. */
  title(): void {
    const busy = this.el.classList.contains("busy");
    const left = Math.max(0, Math.ceil((this.due - performance.now()) / 1000));
    this.el.title = busy ? t("updating") : t("nextIn", { n: left });
  }

  private set(p: Pace): void {
    clearTimeout(this.pending);
    if (p.state === "busy") {
      this.busySince = performance.now();
      this.show("busy");
      return;
    }
    const wait = Math.max(0, this.busySince + MIN_BUSY_MS - performance.now());
    if (wait > 0) {
      this.pending = window.setTimeout(() => this.set(p.state === "wait" ? this.aged(p, wait) : p), wait);
      return;
    }
    if (p.state === "idle") return this.show("idle");
    this.due = performance.now() + p.leftMs;
    this.show("wait");
    // Jump to the part still left, then let the arc run out over the remaining time.
    const arc = this.arc.style;
    arc.transition = "none";
    arc.strokeDashoffset = String(100 - (100 * p.leftMs) / p.totalMs);
    void this.arc.getBoundingClientRect();
    arc.transition = `stroke-dashoffset ${p.leftMs}ms linear`;
    arc.strokeDashoffset = "100";
  }

  /** The same wait, `ms` later. */
  private aged(p: Extract<Pace, { state: "wait" }>, ms: number): Pace {
    return { ...p, leftMs: Math.max(0, p.leftMs - ms) };
  }

  private show(state: "idle" | "busy" | "wait"): void {
    this.el.classList.toggle("busy", state === "busy");
    this.el.hidden = state === "idle";
    if (state === "busy") {
      this.arc.style.transition = "none";
      this.arc.style.strokeDashoffset = "";
    }
    this.title();
  }
}
