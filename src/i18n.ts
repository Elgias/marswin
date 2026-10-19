import type { Lang } from "./api";

const uk = {
  settings: "Налаштування",
  minimize: "Згорнути",
  close: "Закрити",
  input: "Вхід",
  output: "Вихід",
  temperature: "Температура",
  mode: "Режим",
  charging: "Заряджання",
  discharging: "Розряджання",
  idle: "Очікування",
  offline: "Немає зв'язку",
  connecting: "Підключення…",
  noDevice: "Пристрій не вибрано",
  general: "Загальне",
  language: "Мова",
  refresh: "Частота оновлення",
  modeCheck: "Перевірка режиму",
  timeout: "Тайм-аут",
  device: "Пристрій",
  charge: "Заряджання (W)",
  chargeRange: "Діапазон {min}-{max}",
  chargeApply: "Увімкнути заряджання з цією потужністю",
  chargeDone: "Режим Passive: заряджання {w} W",
  pickDevice: "Вибрати пристрій",
  searching: "Пошук пристроїв…",
  nothingFound: "Пристроїв не знайдено. Перевірте, що Open API увімкнено в застосунку Marstek.",
  rescan: "Шукати знову",
  manualIp: "Ввести IP вручну",
  connect: "Підключитись",
  connected: "Підключено: {name}",
  connection: "З'єднання",
  network: "Мережа",
  signal: "Сигнал",
  ip: "IP",
  sec: "{n} сек",
  min: "{n} хв",
  badIp: "Невірна IP-адреса",
  badPort: "Невірний порт",
  badPower: "Потужність має бути від {min} до {max} W",
  failed: "Помилка: {e}",
  timeoutErr: "пристрій не відповідає",
  rejected: "пристрій відхилив команду",
};

const en: typeof uk = {
  settings: "Settings",
  minimize: "Minimize",
  close: "Close",
  input: "Input",
  output: "Output",
  temperature: "Temperature",
  mode: "Mode",
  charging: "Charging",
  discharging: "Discharging",
  idle: "Idle",
  offline: "No connection",
  connecting: "Connecting…",
  noDevice: "No device selected",
  general: "General",
  language: "Language",
  refresh: "Refresh rate",
  modeCheck: "Mode check",
  timeout: "Timeout",
  device: "Device",
  charge: "Charging (W)",
  chargeRange: "Range {min}-{max}",
  chargeApply: "Start charging at this power",
  chargeDone: "Passive mode: charging at {w} W",
  pickDevice: "Select device",
  searching: "Searching for devices…",
  nothingFound: "No devices found. Check that Open API is enabled in the Marstek app.",
  rescan: "Search again",
  manualIp: "Enter IP manually",
  connect: "Connect",
  connected: "Connected: {name}",
  connection: "Connection",
  network: "Network",
  signal: "Signal",
  ip: "IP",
  sec: "{n} sec",
  min: "{n} min",
  badIp: "Invalid IP address",
  badPort: "Invalid port",
  badPower: "Power must be {min} to {max} W",
  failed: "Error: {e}",
  timeoutErr: "device does not respond",
  rejected: "device rejected the command",
};

export type Key = keyof typeof uk;
const dict: Record<Lang, typeof uk> = { uk, en };
let current: Lang = "uk";

export function setLang(lang: Lang): void {
  current = lang;
  document.documentElement.lang = lang;
}

export function t(key: Key, vars: Record<string, string | number> = {}): string {
  return dict[current][key].replace(/\{(\w+)\}/g, (_, k: string) => String(vars[k] ?? ""));
}

/** Fills every element that carries data-i18n / data-i18n-title. */
export function translatePage(): void {
  document.querySelectorAll<HTMLElement>("[data-i18n]").forEach((el) => {
    el.textContent = t(el.dataset.i18n as Key);
  });
  document.querySelectorAll<HTMLElement>("[data-i18n-title]").forEach((el) => {
    el.title = t(el.dataset.i18nTitle as Key);
  });
}

/** Turns backend error codes into readable text. */
export function errorText(e: unknown): string {
  const raw = String(e);
  if (raw === "timeout") return t("timeoutErr");
  if (raw === "rejected") return t("rejected");
  return raw;
}

export function seconds(n: number): string {
  return n >= 60 && n % 60 === 0 ? t("min", { n: n / 60 }) : t("sec", { n });
}
