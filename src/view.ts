import type { Snapshot } from "./api";
import { t } from "./i18n";

export const watts = (w: number | null) => (w == null ? "—" : `${Math.round(w)} W`);
export const percent = (soc: number | null) => (soc == null ? "—" : `${soc.toFixed(1)}%`);
export const barWidth = (soc: number | null) => `${Math.max(0, Math.min(100, soc ?? 0))}%`;

/** Status line text and its colour key (data-flow), shared by the window and the widget. */
export function statusOf(s: Snapshot, hasDevice: boolean): { text: string; flow: string } {
  if (!hasDevice) return { text: t("noDevice"), flow: "none" };
  if (!s.online) return s.error ? { text: t("offline"), flow: "offline" } : { text: t("connecting"), flow: "none" };
  return { text: t(s.flow), flow: s.flow };
}
