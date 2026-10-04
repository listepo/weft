// The reference forms of attribute values (SPEC §2.1), for text a designer types into a layer.
// @weft/core reads them in its WebAssembly build, which Figma's plugin main thread cannot run
// (README, "Where the code runs"); the cross-check in test/values.test.ts keeps the two in step.
import type { Value } from "@weft/core";

/** Text as a value: a whole `{$…}`, `{!$…}` or `{token.…}` is a reference, `{{` escapes a `{`. */
export function readText(raw: string): Value | undefined {
  if (raw.startsWith("{{")) return raw.slice(1);
  if (!raw.startsWith("{")) return raw;
  const inner = raw.endsWith("}") && raw.length >= 2 ? raw.slice(1, -1) : undefined;
  if (inner?.startsWith("$")) return { bind: inner };
  if (inner?.startsWith("!$")) return { bind: inner.slice(1), not: true };
  if (inner?.startsWith("token.")) return { token: inner.slice(6) };
  return undefined;
}

/** A value as attribute text, before XML escaping. */
export function formatValue(value: Value): string {
  if (typeof value === "string") return value.startsWith("{") ? `{${value}` : value;
  if (typeof value === "number" || typeof value === "boolean") return JSON.stringify(value);
  if ("bind" in value) return `{${value.not === true ? "!" : ""}${value.bind}}`;
  return `{token.${value.token}}`;
}
