/** Dark dropdown that matches the design (a native <select> pops up an OS-styled list). */

export interface Option<T> {
  value: T;
  label: string;
}

export class Select<T> {
  readonly el: HTMLDivElement;
  private button: HTMLButtonElement;
  private list: HTMLUListElement;
  private options: Option<T>[] = [];
  private value: T | undefined;

  constructor(host: HTMLElement, private onChange: (value: T) => void) {
    this.el = document.createElement("div");
    this.el.className = "select";
    this.button = document.createElement("button");
    this.button.type = "button";
    this.button.className = "select-button";
    this.list = document.createElement("ul");
    this.list.className = "select-list";
    this.list.setAttribute("role", "listbox");
    this.el.append(this.button, this.list);
    host.replaceWith(this.el);

    this.button.addEventListener("click", () => this.toggle());
    this.button.addEventListener("keydown", (e) => this.onKey(e));
    document.addEventListener("pointerdown", (e) => {
      if (!this.el.contains(e.target as Node)) this.toggle(false);
    });
    window.addEventListener("blur", () => this.toggle(false));
  }

  setOptions(options: Option<T>[], value: T): void {
    this.options = options;
    // Keep a stored value visible even when it is not one of the presets.
    if (!options.some((o) => o.value === value)) this.options = [...options, { value, label: String(value) }];
    this.value = value;
    this.render();
  }

  private render(): void {
    const current = this.options.find((o) => o.value === this.value);
    this.button.innerHTML = "";
    const label = document.createElement("span");
    label.textContent = current?.label ?? "";
    const caret = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    caret.setAttribute("viewBox", "0 0 10 6");
    caret.classList.add("caret");
    caret.innerHTML = '<path d="M0 0h10L5 6z"/>';
    this.button.append(label, caret);

    this.list.innerHTML = "";
    for (const o of this.options) {
      const li = document.createElement("li");
      li.textContent = o.label;
      li.setAttribute("role", "option");
      li.classList.toggle("selected", o.value === this.value);
      li.addEventListener("click", () => this.pick(o.value));
      this.list.append(li);
    }
  }

  private pick(value: T): void {
    this.toggle(false);
    if (value === this.value) return;
    this.value = value;
    this.render();
    this.onChange(value);
  }

  private toggle(open = !this.el.classList.contains("open")): void {
    this.el.classList.toggle("open", open);
  }

  private onKey(e: KeyboardEvent): void {
    const i = this.options.findIndex((o) => o.value === this.value);
    if (e.key === "Escape") this.toggle(false);
    else if (e.key === "ArrowDown" && i < this.options.length - 1) this.pick(this.options[i + 1].value);
    else if (e.key === "ArrowUp" && i > 0) this.pick(this.options[i - 1].value);
    else return;
    e.preventDefault();
  }
}
