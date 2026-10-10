//! Helpers the generated component carries with it, so that it needs nothing from Weft at run
//! time. Each one restates a reading of `@weft/render-react` (values.ts, expand.ts, render.ts):
//! the generated code must resolve, test and sanitize values exactly as the reference renderer
//! does, and packages/to-jsx/test/generate.test.ts checks them against it. Only the helpers a
//! document uses are emitted, in this order.

/// A helper: its name, its JavaScript source, and the same function with TypeScript types.
pub struct Helper {
    pub name: &'static str,
    pub js: &'static str,
    pub ts: &'static str,
}

pub const RUNTIME: &[Helper] = &[
    Helper {
        name: "_text",
        js: r#"function _text(v) {
  if (typeof v === "string") return v;
  if (typeof v === "number") return Number.isFinite(v) ? String(v) : "";
  if (typeof v === "boolean") return String(v);
  return "";
}"#,
        ts: r#"function _text(v: unknown): string {
  if (typeof v === "string") return v;
  if (typeof v === "number") return Number.isFinite(v) ? String(v) : "";
  if (typeof v === "boolean") return String(v);
  return "";
}"#,
    },
    Helper {
        name: "_on",
        js: r#"function _on(v) {
  return v === "false" ? false : Boolean(v);
}"#,
        ts: r#"function _on(v: unknown): boolean {
  return v === "false" ? false : Boolean(v);
}"#,
    },
    Helper {
        name: "_get",
        js: r#"function _get(value, path) {
  for (const key of path) {
    if (Array.isArray(value)) value = /^\d+$/.test(key) ? value[Number(key)] : undefined;
    else if (typeof value === "object" && value !== null && Object.hasOwn(value, key)) value = value[key];
    else return undefined;
  }
  return value;
}"#,
        ts: r#"function _get(value: any, path: string[]): any {
  for (const key of path) {
    if (Array.isArray(value)) value = /^\d+$/.test(key) ? value[Number(key)] : undefined;
    else if (typeof value === "object" && value !== null && Object.hasOwn(value, key)) value = value[key];
    else return undefined;
  }
  return value;
}"#,
    },
    Helper {
        name: "_num",
        js: r#"function _num(v, integer, min, max) {
  if (typeof v !== "number") return v;
  if (!Number.isFinite(v)) return undefined;
  if (integer) v = Math.round(v);
  if (min !== undefined) v = Math.max(v, min);
  if (max !== undefined) v = Math.min(v, max);
  return v;
}"#,
        ts: r#"function _num(v: unknown, integer: boolean, min?: number, max?: number): unknown {
  if (typeof v !== "number") return v;
  if (!Number.isFinite(v)) return undefined;
  if (integer) v = Math.round(v);
  if (min !== undefined) v = Math.max(v, min);
  if (max !== undefined) v = Math.min(v, max);
  return v;
}"#,
    },
    Helper {
        name: "_squash",
        js: r#"function _squash(v) {
  return v.trim().replace(/\s+/g, " ");
}"#,
        ts: r#"function _squash(v: string): string {
  return v.trim().replace(/\s+/g, " ");
}"#,
    },
    Helper {
        name: "_list",
        js: r#"function _list(v) {
  return Array.isArray(v) ? v : [];
}"#,
        ts: r#"function _list(v: unknown): any[] {
  return Array.isArray(v) ? v : [];
}"#,
    },
    Helper {
        name: "_act",
        js: r#"function _act(actions, event) {
  if (typeof actions !== "object" || actions === null || !Object.hasOwn(actions, event.action)) return;
  const fn = actions[event.action];
  if (typeof fn === "function") fn(event);
}"#,
        ts: r#"function _act(actions: WeftProps["actions"], event: WeftEvent): void {
  if (typeof actions !== "object" || actions === null || !Object.hasOwn(actions, event.action)) return;
  const fn = actions[event.action];
  if (typeof fn === "function") fn(event);
}"#,
    },
    Helper {
        name: "_url",
        js: r#"function _url(value) {
  const url = value.replace(/[\t\n\r]/g, "").replace(/^[\u0000- ]+|[\u0000- ]+$/g, "");
  if (url === "") return undefined;
  const colon = url.indexOf(":");
  const delimiter = url.search(/[/?#]/);
  if (colon < 0 || (delimiter >= 0 && colon > delimiter)) return url;
  const scheme = /^([A-Za-z][A-Za-z0-9+.-]*):/.exec(url)?.[1]?.toLowerCase();
  return scheme === "http" || scheme === "https" || scheme === "mailto" ? url : undefined;
}"#,
        ts: r#"function _url(value: string): string | undefined {
  const url = value.replace(/[\t\n\r]/g, "").replace(/^[\u0000- ]+|[\u0000- ]+$/g, "");
  if (url === "") return undefined;
  const colon = url.indexOf(":");
  const delimiter = url.search(/[/?#]/);
  if (colon < 0 || (delimiter >= 0 && colon > delimiter)) return url;
  const scheme = /^([A-Za-z][A-Za-z0-9+.-]*):/.exec(url)?.[1]?.toLowerCase();
  return scheme === "http" || scheme === "https" || scheme === "mailto" ? url : undefined;
}"#,
    },
    Helper {
        name: "_float",
        js: r#"function _float(v) {
  return /^-?(?:\d+(?:\.\d+)?|\.\d+)(?:[eE][-+]?\d+)?$/.test(v) ? v : "";
}"#,
        ts: r#"function _float(v: string): string {
  return /^-?(?:\d+(?:\.\d+)?|\.\d+)(?:[eE][-+]?\d+)?$/.test(v) ? v : "";
}"#,
    },
    Helper {
        name: "_level",
        js: r#"function _level(v) {
  const n = typeof v === "string" ? Number(v) : v;
  return typeof n === "number" && Number.isInteger(n) && n >= 1 && n <= 6 ? n : 2;
}"#,
        ts: r#"function _level(v: unknown): number {
  const n = typeof v === "string" ? Number(v) : v;
  return typeof n === "number" && Number.isInteger(n) && n >= 1 && n <= 6 ? n : 2;
}"#,
    },
    Helper {
        name: "_cols",
        js: r#"function _cols(v) {
  return typeof v === "number" && Number.isInteger(v) && v >= 1 && v <= 64 ? "repeat(" + v + ", minmax(0, 1fr))" : undefined;
}"#,
        ts: r#"function _cols(v: unknown): string | undefined {
  return typeof v === "number" && Number.isInteger(v) && v >= 1 && v <= 64 ? "repeat(" + v + ", minmax(0, 1fr))" : undefined;
}"#,
    },
    Helper {
        name: "_fill",
        // The same cap as `html::capped_columns`: at most `v` tracks, fewer when `min` does not fit.
        js: r#"function _fill(v, min, gap) {
  if (!(typeof v === "number" && Number.isInteger(v) && v >= 1 && v <= 64) || typeof min !== "string") return undefined;
  const gaps = v - 1;
  const floor = typeof gap === "string" && gaps > 0 ? "max(" + min + ", calc((100% - " + gaps + " * " + gap + ") / " + v + "))" : "max(" + min + ", calc(100% / " + v + "))";
  return "repeat(auto-fill, minmax(" + floor + ", 1fr))";
}"#,
        ts: r#"function _fill(v: unknown, min: string, gap?: string): string | undefined {
  if (!(typeof v === "number" && Number.isInteger(v) && v >= 1 && v <= 64) || typeof min !== "string") return undefined;
  const gaps = v - 1;
  const floor = typeof gap === "string" && gaps > 0 ? "max(" + min + ", calc((100% - " + gaps + " * " + gap + ") / " + v + "))" : "max(" + min + ", calc(100% / " + v + "))";
  return "repeat(auto-fill, minmax(" + floor + ", 1fr))";
}"#,
    },
    Helper {
        name: "_justify",
        // `start` is the initial value, so it is left out (SPEC §5.1).
        js: r#"function _justify(v) {
  return v === "center" ? "center" : v === "end" ? "flex-end" : v === "space-between" ? "space-between" : undefined;
}"#,
        ts: r#"function _justify(v: string): string | undefined {
  return v === "center" ? "center" : v === "end" ? "flex-end" : v === "space-between" ? "space-between" : undefined;
}"#,
    },
    Helper {
        name: "_justify",
        js: r#"function _justify(v) {
  return v === "center" ? "center" : v === "end" ? "flex-end" : v === "space-between" ? "space-between" : undefined;
}"#,
        ts: r#"function _justify(v: string): string | undefined {
  return v === "center" ? "center" : v === "end" ? "flex-end" : v === "space-between" ? "space-between" : undefined;
}"#,
    },
    Helper {
        name: "_fill",
        js: r#"function _fill(columns, min, gap) {
  if (typeof columns !== "number" || !Number.isInteger(columns) || columns < 1 || columns > 64) return undefined;
  const gaps = columns - 1;
  const floor = gaps > 0 && typeof gap === "string" ? "max(" + min + ", calc((100% - " + gaps + " * " + gap + ") / " + columns + "))" : "max(" + min + ", calc(100% / " + columns + "))";
  return "repeat(auto-fill, minmax(" + floor + ", 1fr))";
}"#,
        ts: r#"function _fill(columns: unknown, min: string, gap: string | undefined): string | undefined {
  if (typeof columns !== "number" || !Number.isInteger(columns) || columns < 1 || columns > 64) return undefined;
  const gaps = columns - 1;
  const floor = gaps > 0 && typeof gap === "string" ? "max(" + min + ", calc((100% - " + gaps + " * " + gap + ") / " + columns + "))" : "max(" + min + ", calc(100% / " + columns + "))";
  return "repeat(auto-fill, minmax(" + floor + ", 1fr))";
}"#,
    },
    Helper {
        name: "_align",
        js: r#"function _align(v, direction) {
  return v === "start" ? "flex-start" : v === "end" ? "flex-end" : v === "center" || v === "stretch" ? v : direction === "row" ? "center" : undefined;
}"#,
        ts: r#"function _align(v: string, direction: string): string | undefined {
  return v === "start" ? "flex-start" : v === "end" ? "flex-end" : v === "center" || v === "stretch" ? v : direction === "row" ? "center" : undefined;
}"#,
    },
    Helper {
        name: "_busy",
        js: r#"function _busy(state) {
  return state === "loading" || state === "busy" || state === "submitting" ? "true" : undefined;
}"#,
        ts: r#"function _busy(state: string): "true" | undefined {
  return state === "loading" || state === "busy" || state === "submitting" ? "true" : undefined;
}"#,
    },
    Helper {
        name: "_keyed",
        js: r#"function _keyed(list) {
  return list.map((x, i) => <Fragment key={i}>{x}</Fragment>);
}"#,
        ts: r#"function _keyed(list: ReactNode[]): ReactNode[] {
  return list.map((x, i) => <Fragment key={i}>{x}</Fragment>);
}"#,
    },
    Helper {
        name: "_press",
        js: r#"function _press(e, fire) {
  if (e.target !== e.currentTarget || (e.key !== "Enter" && e.key !== " ")) return;
  e.preventDefault();
  fire();
}"#,
        ts: r#"function _press(e: WeftKey, fire: () => void): void {
  if (e.target !== e.currentTarget || (e.key !== "Enter" && e.key !== " ")) return;
  e.preventDefault();
  fire();
}"#,
    },
    Helper {
        name: "_tabKey",
        js: r#"function _tabKey(e, i, count, choose) {
  const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
  if (step === 0 || count === 0) return;
  e.preventDefault();
  const next = (i + step + count) % count;
  choose(next);
  e.currentTarget.parentElement?.querySelectorAll('[role="tab"]')[next]?.focus();
}"#,
        ts: r#"function _tabKey(e: WeftKey, i: number, count: number, choose: (i: number) => void): void {
  const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
  if (step === 0 || count === 0) return;
  e.preventDefault();
  const next = (i + step + count) % count;
  choose(next);
  e.currentTarget.parentElement?.querySelectorAll<HTMLElement>('[role="tab"]')[next]?.focus();
}"#,
    },
    Helper {
        name: "_numeric",
        js: r#"function _numeric(v) {
  if (typeof v === "number") return Number.isFinite(v) ? v : undefined;
  if (typeof v === "string" && /^-?(?:\d+(?:\.\d+)?|\.\d+)(?:[eE][-+]?\d+)?$/.test(v.trim())) {
    const n = Number(v);
    return Number.isFinite(n) ? n : undefined;
  }
  return undefined;
}"#,
        ts: r#"function _numeric(v: unknown): number | undefined {
  if (typeof v === "number") return Number.isFinite(v) ? v : undefined;
  if (typeof v === "string" && /^-?(?:\d+(?:\.\d+)?|\.\d+)(?:[eE][-+]?\d+)?$/.test(v.trim())) {
    const n = Number(v);
    return Number.isFinite(n) ? n : undefined;
  }
  return undefined;
}"#,
    },
    Helper {
        name: "_tidy",
        js: r#"function _tidy(x, ...inputs) {
  const d = (y) => {
    const [m = "", e] = String(y).split("e");
    return Math.max(0, (m.split(".")[1]?.length ?? 0) - (e === undefined ? 0 : Number(e)));
  };
  return Number(x.toFixed(Math.min(100, Math.max(...inputs.map(d)))));
}"#,
        ts: r#"function _tidy(x: number, ...inputs: number[]): number {
  const d = (y: number): number => {
    const [m = "", e] = String(y).split("e");
    return Math.max(0, (m.split(".")[1]?.length ?? 0) - (e === undefined ? 0 : Number(e)));
  };
  return Number(x.toFixed(Math.min(100, Math.max(...inputs.map(d)))));
}"#,
    },
    Helper {
        name: "_span",
        js: r#"function _span(min, max, step) {
  const lo = _numeric(min) ?? 0;
  const st = _numeric(step);
  return { min: lo, max: Math.max(_numeric(max) ?? 100, lo), step: st !== undefined && st > 0 ? st : 1 };
}"#,
        ts: r#"function _span(min: unknown, max: unknown, step: unknown): { min: number; max: number; step: number } {
  const lo = _numeric(min) ?? 0;
  const st = _numeric(step);
  return { min: lo, max: Math.max(_numeric(max) ?? 100, lo), step: st !== undefined && st > 0 ? st : 1 };
}"#,
    },
    Helper {
        name: "_slide",
        js: r#"function _slide(value, s) {
  const v = Math.min(Math.max(_numeric(value) ?? s.min, s.min), s.max);
  let on = s.min + Math.floor((v - s.min) / s.step + 0.5) * s.step;
  if (on > s.max) on -= s.step;
  return String(_tidy(Math.max(on, s.min), s.min, s.step));
}"#,
        ts: r#"function _slide(value: unknown, s: { min: number; max: number; step: number }): string {
  const v = Math.min(Math.max(_numeric(value) ?? s.min, s.min), s.max);
  let on = s.min + Math.floor((v - s.min) / s.step + 0.5) * s.step;
  if (on > s.max) on -= s.step;
  return String(_tidy(Math.max(on, s.min), s.min, s.step));
}"#,
    },
    Helper {
        name: "_bounds",
        js: r#"function _bounds(min, max, step) {
  const lo = _numeric(min);
  const hi = _numeric(max);
  const st = _numeric(step);
  const b = { step: st !== undefined && st > 0 ? st : 1 };
  if (lo !== undefined) b.min = lo;
  if (hi !== undefined) b.max = lo === undefined ? hi : Math.max(hi, lo);
  return b;
}"#,
        ts: r#"function _bounds(min: unknown, max: unknown, step: unknown): { min?: number; max?: number; step: number } {
  const lo = _numeric(min);
  const hi = _numeric(max);
  const st = _numeric(step);
  const b: { min?: number; max?: number; step: number } = { step: st !== undefined && st > 0 ? st : 1 };
  if (lo !== undefined) b.min = lo;
  if (hi !== undefined) b.max = lo === undefined ? hi : Math.max(hi, lo);
  return b;
}"#,
    },
    Helper {
        name: "_clamp",
        js: r#"function _clamp(v, b) {
  if (b.min !== undefined) v = Math.max(v, b.min);
  if (b.max !== undefined) v = Math.min(v, b.max);
  return v;
}"#,
        ts: r#"function _clamp(v: number, b: { min?: number; max?: number; step: number }): number {
  if (b.min !== undefined) v = Math.max(v, b.min);
  if (b.max !== undefined) v = Math.min(v, b.max);
  return v;
}"#,
    },
    Helper {
        name: "_count",
        js: r#"function _count(value, b) {
  return _clamp(_numeric(value) ?? 0, b);
}"#,
        ts: r#"function _count(value: unknown, b: { min?: number; max?: number; step: number }): number {
  return _clamp(_numeric(value) ?? 0, b);
}"#,
    },
    Helper {
        name: "_step",
        js: r#"function _step(current, direction, b) {
  return _tidy(_clamp(current + direction * b.step, b), current, b.step);
}"#,
        ts: r#"function _step(current: number, direction: 1 | -1, b: { min?: number; max?: number; step: number }): number {
  return _tidy(_clamp(current + direction * b.step, b), current, b.step);
}"#,
    },
    Helper {
        name: "_dateType",
        js: r#"function _dateType(v) {
  return v === "time" || v === "datetime" ? v : "date";
}"#,
        ts: r#"function _dateType(v: string): "date" | "time" | "datetime" {
  return v === "time" || v === "datetime" ? v : "date";
}"#,
    },
    Helper {
        name: "_dateText",
        js: r#"function _dateText(type, value) {
  const day = (s) => {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
    if (!m) return false;
    const [year, month, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
    if (year < 1 || month < 1 || month > 12 || d < 1) return false;
    const leap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
    return d <= [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1];
  };
  const time = (s) => /^([01]\d|2[0-3]):[0-5]\d$/.test(s);
  if (type === "date") return day(value) ? value : "";
  if (type === "time") return time(value) ? value : "";
  const [d = "", t = "", ...rest] = value.split("T");
  return rest.length === 0 && day(d) && time(t) ? value : "";
}"#,
        ts: r#"function _dateText(type: string, value: string): string {
  const day = (s: string): boolean => {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
    if (!m) return false;
    const [year, month, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
    if (year < 1 || month < 1 || month > 12 || d < 1) return false;
    const leap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
    return d <= [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1]!;
  };
  const time = (s: string): boolean => /^([01]\d|2[0-3]):[0-5]\d$/.test(s);
  if (type === "date") return day(value) ? value : "";
  if (type === "time") return time(value) ? value : "";
  const [d = "", t = "", ...rest] = value.split("T");
  return rest.length === 0 && day(d) && time(t) ? value : "";
}"#,
    },
    Helper {
        name: "_color",
        js: r##"function _color(v) {
  return /^#[0-9a-fA-F]{6}$/.test(v) ? v.toLowerCase() : "#000000";
}"##,
        ts: r##"function _color(v: string): string {
  return /^#[0-9a-fA-F]{6}$/.test(v) ? v.toLowerCase() : "#000000";
}"##,
    },
    // SolidJS only: loops outside JSX get the index as an accessor, as `<For>` passes it, so one
    // expression reads the index the same way in both places.
    Helper {
        name: "_ix",
        js: r#"function _ix(list) {
  return list.map((v, i) => [v, () => i]);
}"#,
        ts: r#"function _ix(list: any[]): [any, () => number][] {
  return list.map((v, i) => [v, () => i]);
}"#,
    },
];

/// The JavaScript source of every helper React output may use, by name, in emission order: what
/// `@weft/to-jsx` exports as `RUNTIME`.
pub fn react_runtime() -> impl Iterator<Item = (&'static str, &'static str)> {
    RUNTIME
        .iter()
        .filter(|h| h.name != "_ix")
        .map(|h| (h.name, h.js))
}
