import type { Snapshot } from "./api";
import { t } from "./i18n";
import { swap } from "./motion";

export const watts = (w: number | null) => (w == null ? "—" : `${Math.round(w)} W`);
export const percent = (soc: number | null) => (soc == null ? "—" : `${soc.toFixed(1)}%`);
export const barWidth = (soc: number | null) => `${Math.max(0, Math.min(100, soc ?? 0))}%`;

/** "8г 39хв" / "8h 39m"; under an hour only minutes, from a day on days and hours. */
export function span(min: number): string {
  const [d, h, m] = [Math.floor(min / 1440), Math.floor((min % 1440) / 60), min % 60];
  if (min < 60) return t("spanMin", { m });
  if (min < 1440) return t("spanHours", { h, m });
  return t("spanDays", { d, h });
}

export interface Status {
  text: string;
  /** Colour key (data-flow). */
  flow: string;
  /** True when the text is the time left, which changes with every poll. */
  estimate: boolean;
}

/** Status line text and its colour key, shared by the window and the widget.
 *  While discharging, the time left replaces "Discharging" once Rust has an estimate. */
export function statusOf(s: Snapshot, hasDevice: boolean): Status {
  const plain = (text: string, flow: string) => ({ text, flow, estimate: false });
  if (!hasDevice) return plain(t("noDevice"), "none");
  if (!s.online) return s.error ? plain(t("offline"), "offline") : plain(t("connecting"), "none");
  if (s.flow === "discharging" && s.remainingMin != null) {
    return { text: t("timeLeft", { t: span(s.remainingMin) }), flow: s.flow, estimate: true };
  }
  return plain(t(s.flow), s.flow);
}

/** Puts the status into `el`. A new status fades in; a new figure of the same estimate just changes. */
export function showStatus(el: HTMLElement, status: Status): void {
  if (status.estimate && el.dataset.estimate === "1") el.textContent = status.text;
  else swap(el, status.text);
  el.dataset.flow = status.flow;
  el.dataset.estimate = status.estimate ? "1" : "";
}
