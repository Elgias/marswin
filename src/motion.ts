/** Small motion helpers: rolling numbers, text swaps and the window entrance (styles in motion.css). */

const calm = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
const easeOut = (k: number) => 1 - (1 - k) ** 3;

const rolling = new WeakMap<HTMLElement, { value: number; frame: number }>();

/** The next `roll` of `el` starts from `value` (for example 0, so the first figure counts up). */
export function seed(el: HTMLElement, value: number): void {
  rolling.set(el, { value, frame: 0 });
}

/** Shows `to` in `el`, counting from the figure shown now. */
export function roll(el: HTMLElement, to: number | null, format: (n: number | null) => string, ms = 700): void {
  const prev = rolling.get(el);
  if (prev) cancelAnimationFrame(prev.frame);
  if (to == null || prev == null || prev.value === to || calm() || document.hidden) {
    el.textContent = format(to);
    if (to == null) rolling.delete(el);
    else seed(el, to);
    return;
  }
  const from = prev.value;
  const start = performance.now();
  const step = (now: number) => {
    const k = Math.min(1, (now - start) / ms);
    const value = from + (to - from) * easeOut(k);
    el.textContent = format(value);
    rolling.set(el, { value, frame: k < 1 ? requestAnimationFrame(step) : 0 });
  };
  rolling.set(el, { value: from, frame: requestAnimationFrame(step) });
}

/** Sets the text; a changed text fades in from slightly below. */
export function swap(el: HTMLElement, text: string): void {
  if (el.textContent === text) return;
  el.textContent = text;
  if (calm()) return;
  el.animate(
    [
      { opacity: 0, transform: "translateY(6px)" },
      { opacity: 1, transform: "none" },
    ],
    { duration: 320, easing: "cubic-bezier(0.22, 1, 0.36, 1)" },
  );
}

/** Numbers the children of every [data-stagger] box, so they rise one after another.
 *  A nested box does not move itself; its children continue the count from its place. */
function index(root: ParentNode = document): void {
  for (const box of root.querySelectorAll<HTMLElement>("[data-stagger]")) {
    const base = Number(box.style.getPropertyValue("--i")) || 0;
    [...box.children].forEach((c, i) => (c as HTMLElement).style.setProperty("--i", String(base + i)));
  }
}

let entered = 0;

/** Plays the entrance: the page fades in and the [data-stagger] parts rise. */
export function enter(): void {
  index();
  const body = document.body;
  body.classList.remove("leaving", "entering");
  void body.offsetWidth; // restart the CSS animations
  body.classList.add("entering");
  // A finished animation still keeps each part in its own layer, and an open dropdown
  // would slide under the next row; drop the animations once they are done.
  clearTimeout(entered);
  entered = window.setTimeout(() => body.classList.remove("entering"), 1200);
}

/** Fades the page out; resolves when the fade is done (same length as FADE_OUT in lib.rs). */
export function leave(): Promise<void> {
  document.body.classList.remove("entering");
  document.body.classList.add("leaving");
  return new Promise((done) => setTimeout(done, calm() ? 0 : 160));
}
