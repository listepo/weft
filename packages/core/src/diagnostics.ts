// The diagnostic code registry of SPEC §6.1. Codes are a public API: a published code keeps its
// meaning forever, so new checks get new codes instead of reusing old ones.
import type { Diagnostic } from "./model.ts";
import { toJson, wasm, wellFormed } from "./wasm.ts";

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
  W224: { severity: "error", summary: "Number outside the declared range or not whole." },

  W301: { severity: "error", summary: "Duplicate id." },
  W302: { severity: "error", summary: "Child kind not allowed here." },
  W303: { severity: "error", summary: "Parent kind not allowed for this component." },
  W304: { severity: "error", summary: "Content breaks the content model." },
  W305: { severity: "error", summary: "Loop variable not in scope." },
  W306: { severity: "error", summary: "Unknown design token." },
  W307: { severity: "error", summary: "Design token has the wrong type." },
  W308: { severity: "error", summary: "Unknown action." },
  W309: { severity: "error", summary: "Id reference points at no suitable element." },
  W310: { severity: "error", summary: "Text given both as content and as `text`." },
  W311: { severity: "error", summary: "Loop variable shadows an outer one." },
  W312: { severity: "error", summary: "`screen` below the root." },
  W313: { severity: "error", summary: "Submit button outside a `form`." },
  W314: { severity: "error", summary: "`<each>` without an element to repeat." },
  W315: { severity: "error", summary: "Binding path not declared in the data schema." },
  W316: { severity: "error", summary: "Bound data has a type the attribute does not take." },
  W317: { severity: "error", summary: "Asset path of a `model` is not an allowed path." },
  W318: { severity: "error", summary: "`grow` on an element whose parent is not a `stack`." },

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

  W601: { severity: "error", summary: "Imported input cannot be read." },
  W602: { severity: "warning", summary: "Imported input exceeds an import limit." },

  W701: { severity: "error", summary: "Project file or one of its members has the wrong shape." },
  W702: { severity: "warning", summary: "Unknown member in the project file." },
  W703: { severity: "error", summary: "File name in the project file is not allowed." },
  W704: { severity: "error", summary: "File named by the project cannot be read." },
  W705: { severity: "error", summary: "Problem in the project's token files." },
  W706: { severity: "error", summary: "Catalog extension is not a valid catalog." },
  W707: { severity: "error", summary: "Catalog extension narrows or changes the core catalog." },
  W708: { severity: "error", summary: "Project action name breaks the action grammar." },
  W709: { severity: "error", summary: "Data schema is malformed." },
  W710: { severity: "warning", summary: "Data schema keyword is not supported." },
  W711: { severity: "error", summary: "Two catalogs claim the same name, prefix or kind." },
  W712: {
    severity: "error",
    summary: "Catalog prefix is malformed or reserved, or a second catalog lacks one.",
  },
  W713: { severity: "error", summary: "Catalog defines or extends a kind it does not own." },
  W714: {
    severity: "warning",
    summary: "Catalog requirement is not loaded at a compatible version.",
  },
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

/** Nearest candidate by edit distance, close enough to be a likely typo (the Rust core decides). */
export function didYouMean(word: string, candidates: Iterable<string>): string | undefined {
  // The addon answers `null` where WebAssembly answers `undefined`.
  return wasm.didYouMean(wellFormed(word), toJson([...candidates]) ?? "[]") ?? undefined;
}
