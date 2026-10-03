// The diagnostic code registry of SPEC §6.1. Codes are a public API: a published code keeps its
// meaning forever, so new checks get new codes instead of reusing old ones.
import type { Diagnostic } from "./model.ts";

export type Severity = Diagnostic["severity"];
export type Mode = "lenient" | "strict";
export type Position = { line: number; column: number };

/** `mode` severity: a warning in lenient mode and an error in strict mode (SPEC §8). */
type CodeInfo = { severity: Severity | "mode"; summary: string };

export const DIAGNOSTIC_CODES = {
  W101: { severity: "error", summary: "Malformed markup." },
  W102: { severity: "error", summary: "XML declaration or processing instruction." },
  W103: { severity: "error", summary: "DOCTYPE or other markup declaration." },
  W104: { severity: "error", summary: "CDATA section." },
  W105: { severity: "error", summary: "Element or attribute name breaks the name grammar." },
  W106: { severity: "error", summary: "Attribute value is not double-quoted." },
  W107: { severity: "error", summary: "Attribute without a value." },
  W108: { severity: "error", summary: "Duplicate attribute." },
  W109: { severity: "error", summary: "Closing tag does not match the open element." },
  W110: { severity: "error", summary: "Element, tag or attribute value is never closed." },
  W111: { severity: "error", summary: "Closing tag without an open element." },
  W112: { severity: "error", summary: "Unknown or malformed entity or character reference." },
  W113: { severity: "error", summary: "Character not allowed at this place." },
  W114: { severity: "error", summary: "Content outside the single root element." },
  W115: { severity: "error", summary: "Malformed comment." },
  W116: { severity: "error", summary: "Value starts with `{` but is not a reference." },
  W117: { severity: "error", summary: "Nesting deeper than the limit." },
  W118: { severity: "error", summary: "Misplaced or malformed `<slot>`." },
  W119: { severity: "error", summary: "Slot name used twice under one parent." },

  W200: { severity: "error", summary: "Document does not have the canonical JSON shape." },
  W201: { severity: "error", summary: "Root element is not `screen`." },
  W202: { severity: "error", summary: "Element without `id`." },
  W203: { severity: "error", summary: "Value is not one of the allowed values." },
  W204: { severity: "error", summary: "Value has the wrong type." },
  W205: { severity: "error", summary: "Required attribute is missing." },
  W206: { severity: "error", summary: "Event not declared by the component." },
  W207: { severity: "error", summary: "Slot not declared by the component." },
  W208: { severity: "error", summary: "Required slot is missing." },
  W209: { severity: "error", summary: "`role` on a catalog component." },
  W210: { severity: "error", summary: "Extension element without `role`." },
  W211: { severity: "error", summary: "Not a WAI-ARIA role." },
  W212: { severity: "error", summary: "Id breaks the id grammar." },
  W213: { severity: "error", summary: "Text and a reference mixed in one value." },
  W214: { severity: "error", summary: "Binding path breaks the binding grammar." },
  W215: { severity: "error", summary: "Token path breaks the token grammar." },
  W216: { severity: "error", summary: "Action name breaks the action grammar." },
  W217: { severity: "error", summary: "Binding on a literal-only attribute." },
  W218: { severity: "error", summary: "Negated binding where it cannot apply." },
  W219: { severity: "error", summary: "`weft` version is not `major.minor`." },
  W220: { severity: "error", summary: "Extension name lacks the `x-<vendor>-` prefix." },
  W221: { severity: "error", summary: "String contains characters markup cannot carry." },
  W222: { severity: "error", summary: "Malformed `<each>`." },
  W223: {
    severity: "error",
    summary: "Kind, attribute, event or slot name is invalid or reserved.",
  },

  W301: { severity: "error", summary: "Duplicate id." },
  W302: { severity: "error", summary: "Child kind not allowed here." },
  W303: { severity: "error", summary: "Parent kind not allowed for this component." },
  W304: { severity: "error", summary: "Content breaks the content model." },
  W305: { severity: "error", summary: "Loop variable not in scope." },
  W306: { severity: "error", summary: "Unknown design token." },
  W307: { severity: "error", summary: "Design token has the wrong type." },
  W308: { severity: "error", summary: "Unknown action." },
  W309: { severity: "error", summary: "Id reference points at no suitable element." },
  W310: { severity: "error", summary: "Text given both as content and as `value`." },
  W311: { severity: "error", summary: "Loop variable shadows an outer one." },
  W312: { severity: "error", summary: "`screen` below the root." },

  W401: { severity: "mode", summary: "Unknown element." },
  W402: { severity: "mode", summary: "Unknown attribute." },
  W403: { severity: "mode", summary: "Newer minor version of the format." },
  W404: { severity: "error", summary: "Unsupported major version of the format." },

  W501: { severity: "error", summary: "Patch list or patch has the wrong shape." },
  W502: { severity: "error", summary: "Patch names an id that no element has." },
  W503: { severity: "error", summary: "Prop cannot be set by a patch." },
  W504: { severity: "error", summary: "Patch names a slot the parent does not declare." },
  W505: { severity: "error", summary: "Patch index is outside the target list." },
  W506: { severity: "error", summary: "Element moved into its own subtree." },
  W507: { severity: "error", summary: "The root element cannot be removed or moved." },
  W508: { severity: "error", summary: "Inserted markup is not a list of elements." },
  W509: { severity: "error", summary: "Inserted markup reuses an id of the document." },
} as const satisfies Record<string, CodeInfo>;

export type DiagnosticCode = keyof typeof DIAGNOSTIC_CODES;

export type DiagnosticInit = {
  path: string;
  message: string;
  pos?: Position | undefined;
  /** Required: a repairing model needs to know what would have been valid. */
  expected: string;
  got?: string | undefined;
  hint?: string | undefined;
};

export function diagnostic(code: DiagnosticCode, init: DiagnosticInit, mode: Mode = "lenient") {
  const registered: Severity | "mode" = DIAGNOSTIC_CODES[code].severity;
  const severity: Severity =
    registered === "mode" ? (mode === "strict" ? "error" : "warning") : registered;
  const d: Diagnostic = { code, severity, message: init.message, path: init.path };
  if (init.pos !== undefined) {
    d.line = init.pos.line;
    d.column = init.pos.column;
  }
  d.expected = init.expected;
  if (init.got !== undefined) d.got = init.got;
  if (init.hint !== undefined) d.hint = init.hint;
  return d;
}

export function hasErrors(diagnostics: readonly Diagnostic[]): boolean {
  return diagnostics.some((d) => d.severity === "error");
}

export function byPosition(a: Diagnostic, b: Diagnostic): number {
  return (a.line ?? 0) - (b.line ?? 0) || (a.column ?? 0) - (b.column ?? 0);
}

export function quote(value: unknown): string {
  return typeof value === "string" ? JSON.stringify(value) : String(JSON.stringify(value));
}

export function oneOf(values: Iterable<string>): string {
  const list = [...values];
  return list.length === 0 ? "nothing" : `one of: ${list.map((v) => quote(v)).join(", ")}`;
}

/** Nearest candidate by edit distance, close enough to be a likely typo. */
export function nearest(word: string, candidates: Iterable<string>): string | undefined {
  const lower = word.toLowerCase();
  const limit = word.length <= 3 ? 1 : Math.max(2, Math.floor(word.length / 3));
  let best: string | undefined;
  let bestDistance = Infinity;
  for (const candidate of candidates) {
    if (candidate === word) continue;
    if (candidate.toLowerCase() === lower) return candidate;
    const distance = levenshtein(word, candidate);
    if (distance < bestDistance) {
      best = candidate;
      bestDistance = distance;
    }
  }
  return bestDistance <= limit ? best : undefined;
}

export function didYouMean(word: string, candidates: Iterable<string>): string | undefined {
  const match = nearest(word, candidates);
  return match === undefined ? undefined : `did you mean ${quote(match)}?`;
}

function levenshtein(a: string, b: string): number {
  let previous = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    const current = [i];
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      current[j] = Math.min(
        (previous[j] ?? 0) + 1,
        (current[j - 1] ?? 0) + 1,
        (previous[j - 1] ?? 0) + cost,
      );
    }
    previous = current;
  }
  return previous[b.length] ?? 0;
}
