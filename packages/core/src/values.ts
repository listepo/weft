// Attribute value forms of SPEC §2.1, in both directions, so that the parser and the serializer
// share one definition of the `{`-escape and of reference syntax.
import type { PropDef, Value } from "./model.ts";
import { JSON_NUMBER } from "./rules.ts";

export type ReadValue = { ok: true; value: Value } | { ok: false; message: string; hint: string };

/** Reads a decoded attribute value; `type` comes from the catalog and only shapes literals. */
export function readValue(raw: string, type?: PropDef["type"]): ReadValue {
  if (raw.startsWith("{{")) return { ok: true, value: raw.slice(1) };
  if (raw.startsWith("{")) {
    const inner = raw.endsWith("}") && raw.length >= 2 ? raw.slice(1, -1) : undefined;
    if (inner?.startsWith("$")) return { ok: true, value: { bind: inner } };
    if (inner?.startsWith("!$")) return { ok: true, value: { bind: inner.slice(1), not: true } };
    if (inner?.startsWith("token.")) return { ok: true, value: { token: inner.slice(6) } };
    return {
      ok: false,
      message:
        "A value starting with `{` must be a whole `{$…}`, `{!$…}` or `{token.…}` reference.",
      hint: `write "{${raw}" if the text itself starts with "{"`,
    };
  }
  if (type === "number" && JSON_NUMBER.test(raw)) {
    const number = Number(raw);
    if (Number.isFinite(number)) return { ok: true, value: number === 0 ? 0 : number };
  }
  if (type === "boolean" && (raw === "true" || raw === "false")) {
    return { ok: true, value: raw === "true" };
  }
  return { ok: true, value: raw };
}

/** Writes a value as attribute text, before XML escaping. */
export function formatValue(value: Value): string {
  if (typeof value === "string") return value.startsWith("{") ? `{${value}` : value;
  if (typeof value === "number" || typeof value === "boolean") return JSON.stringify(value) ?? "";
  if ("bind" in value) return `{${value.not === true ? "!" : ""}${value.bind}}`;
  return `{token.${value.token}}`;
}
