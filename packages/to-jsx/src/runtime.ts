// Helpers the generated component carries with it, so that it needs nothing from Weft at run
// time. Each one restates a reading of `@weft/render-react` (values.ts, expand.ts, render.ts):
// the generated code must resolve, test and sanitize values exactly as the reference renderer
// does, and test/generate.test.ts checks them against it. Only the helpers a document uses are
// emitted, in this order.
export const RUNTIME: Readonly<Record<string, string>> = {
  _text: `function _text(v) {
  if (typeof v === "string") return v;
  if (typeof v === "number") return Number.isFinite(v) ? String(v) : "";
  if (typeof v === "boolean") return String(v);
  return "";
}`,
  _on: `function _on(v) {
  return v === "false" ? false : Boolean(v);
}`,
  _get: `function _get(value, path) {
  for (const key of path) {
    if (Array.isArray(value)) value = /^\\d+$/.test(key) ? value[Number(key)] : undefined;
    else if (typeof value === "object" && value !== null && Object.hasOwn(value, key)) value = value[key];
    else return undefined;
  }
  return value;
}`,
  _num: `function _num(v, integer, min, max) {
  if (typeof v !== "number") return v;
  if (!Number.isFinite(v)) return undefined;
  if (integer) v = Math.round(v);
  if (min !== undefined) v = Math.max(v, min);
  if (max !== undefined) v = Math.min(v, max);
  return v;
}`,
  _squash: `function _squash(v) {
  return v.trim().replace(/\\s+/g, " ");
}`,
  _list: `function _list(v) {
  return Array.isArray(v) ? v : [];
}`,
  _act: `function _act(actions, event) {
  if (typeof actions !== "object" || actions === null || !Object.hasOwn(actions, event.action)) return;
  const fn = actions[event.action];
  if (typeof fn === "function") fn(event);
}`,
  _url: `function _url(value) {
  const url = value.replace(/[\\t\\n\\r]/g, "").replace(/^[\\u0000- ]+|[\\u0000- ]+$/g, "");
  if (url === "") return undefined;
  const colon = url.indexOf(":");
  const delimiter = url.search(/[/?#]/);
  if (colon < 0 || (delimiter >= 0 && colon > delimiter)) return url;
  const scheme = /^([A-Za-z][A-Za-z0-9+.-]*):/.exec(url)?.[1]?.toLowerCase();
  return scheme === "http" || scheme === "https" || scheme === "mailto" ? url : undefined;
}`,
  _float: `function _float(v) {
  return /^-?(?:\\d+(?:\\.\\d+)?|\\.\\d+)(?:[eE][-+]?\\d+)?$/.test(v) ? v : "";
}`,
  _level: `function _level(v) {
  const n = typeof v === "string" ? Number(v) : v;
  return typeof n === "number" && Number.isInteger(n) && n >= 1 && n <= 6 ? n : 2;
}`,
  _cols: `function _cols(v) {
  return typeof v === "number" && Number.isInteger(v) && v >= 1 && v <= 64 ? "repeat(" + v + ", minmax(0, 1fr))" : undefined;
}`,
  _align: `function _align(v) {
  return v === "start" ? "flex-start" : v === "end" ? "flex-end" : v === "center" || v === "stretch" ? v : undefined;
}`,
  _busy: `function _busy(state) {
  return state === "loading" || state === "busy" || state === "submitting" ? "true" : undefined;
}`,
  _keyed: `function _keyed(list) {
  return list.map((x, i) => <Fragment key={i}>{x}</Fragment>);
}`,
  _press: `function _press(e, fire) {
  if (e.target !== e.currentTarget || (e.key !== "Enter" && e.key !== " ")) return;
  e.preventDefault();
  fire();
}`,
  _tabKey: `function _tabKey(e, i, count, choose) {
  const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
  if (step === 0 || count === 0) return;
  e.preventDefault();
  const next = (i + step + count) % count;
  choose(next);
  e.currentTarget.parentElement?.querySelectorAll('[role="tab"]')[next]?.focus();
}`,
};
