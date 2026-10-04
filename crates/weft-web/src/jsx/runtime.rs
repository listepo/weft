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
        name: "_align",
        js: r#"function _align(v) {
  return v === "start" ? "flex-start" : v === "end" ? "flex-end" : v === "center" || v === "stretch" ? v : undefined;
}"#,
        ts: r#"function _align(v: string): string | undefined {
  return v === "start" ? "flex-start" : v === "end" ? "flex-end" : v === "center" || v === "stretch" ? v : undefined;
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
