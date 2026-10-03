// Syntax layer (SPEC §2, §6 layer 1): a strict tokenizer for the restricted XML subset.
//
// It is hand-written rather than built on htmlparser2 because htmlparser2 recovers silently from
// exactly what Weft must reject (implied closing tags, unknown entities left as text, unquoted
// values) and reports no positions for attributes, which repair-oriented diagnostics need.
import type { Diagnostic } from "./model.ts";
import {
  byPosition,
  diagnostic,
  type DiagnosticCode,
  type DiagnosticInit,
  type Position,
} from "./diagnostics.ts";
import { MAX_DEPTH, NAME, NON_XML_CHAR } from "./rules.ts";
import { pathSegment } from "./source.ts";

export type RawAttribute = { name: string; value: string; pos: Position };
/** Text as written after reference decoding; whitespace is normalized later by the builder. */
export type RawText = { type: "text"; text: string; pos: Position };
export type RawElement = {
  type: "element";
  name: string;
  attrs: RawAttribute[];
  children: RawChild[];
  pos: Position;
};
export type RawChild = RawElement | RawText;

export type SyntaxResult = { root?: RawElement | undefined; diagnostics: Diagnostic[] };

const PREDEFINED: Readonly<Record<string, string>> = {
  lt: "<",
  gt: ">",
  amp: "&",
  quot: '"',
  apos: "'",
};
const REFERENCE = /&(?:([A-Za-z][A-Za-z0-9]*)|#([0-9]+)|#x([0-9A-Fa-f]+));/y;
const NAME_CHARS = /[^\s/>=<"']+/y;
const WHITESPACE = /[ \t\n]/;

export function tokenize(input: string): SyntaxResult {
  // XML end-of-line handling, so that positions and values agree with any conforming parser.
  const src = (input.startsWith("\uFEFF") ? input.slice(1) : input).replace(/\r\n?/g, "\n");
  const n = src.length;
  const lineStarts = [0];
  for (let k = 0; k < n; k++) if (src.charCodeAt(k) === 10) lineStarts.push(k + 1);

  const diagnostics: Diagnostic[] = [];
  const stack: RawElement[] = [];
  let root: RawElement | undefined;
  let text = "";
  let textStart = -1;
  let i = 0;

  const posAt = (offset: number): Position => {
    let lo = 0;
    let hi = lineStarts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if ((lineStarts[mid] ?? 0) <= offset) lo = mid;
      else hi = mid - 1;
    }
    return { line: lo + 1, column: offset - (lineStarts[lo] ?? 0) + 1 };
  };
  const pathOf = (extra?: string): string => {
    const segments = stack.map((el) =>
      pathSegment(el.name, el.attrs.find((a) => a.name === "id")?.value),
    );
    if (extra !== undefined) segments.push(extra);
    return `/${segments.join("/")}`;
  };
  const report = (
    code: DiagnosticCode,
    offset: number,
    init: Omit<DiagnosticInit, "pos" | "path">,
    extra?: string,
  ) => {
    diagnostics.push(diagnostic(code, { ...init, path: pathOf(extra), pos: posAt(offset) }));
  };

  // A character reference that cannot be decoded is kept verbatim so that tokenizing continues.
  const readReference = (at: number): { value: string; end: number } => {
    REFERENCE.lastIndex = at;
    const m = REFERENCE.exec(src);
    if (m === null) {
      report("W112", at, {
        message: "A bare `&` must start an entity or character reference.",
        expected: "&lt; &gt; &amp; &quot; &apos; or &#N;",
        hint: 'write "&amp;" for a literal ampersand',
      });
      return { value: "&", end: at + 1 };
    }
    const end = at + m[0].length;
    if (m[1] !== undefined) {
      const value = PREDEFINED[m[1]];
      if (value !== undefined && Object.hasOwn(PREDEFINED, m[1])) return { value, end };
      report("W112", at, {
        message: `Entity "&${m[1]};" is not one of the five predefined XML entities.`,
        expected: "&lt; &gt; &amp; &quot; &apos; or a numeric reference",
        got: m[0],
        hint: 'use the character itself (the file is UTF-8) or a numeric reference such as "&#160;"',
      });
      return { value: m[0], end };
    }
    const code = m[2] !== undefined ? Number.parseInt(m[2], 10) : Number.parseInt(m[3] ?? "", 16);
    const char = code <= 0x10ffff ? String.fromCodePoint(code) : "";
    if (char === "" || NON_XML_CHAR.test(char)) {
      report("W112", at, {
        message: `Character reference "${m[0]}" names a character XML does not allow.`,
        expected: "a reference to a character XML allows",
        got: m[0],
      });
      return { value: m[0], end };
    }
    return { value: char, end };
  };

  const flushText = () => {
    if (textStart >= 0) {
      const parent = stack.at(-1);
      if (parent === undefined) {
        report("W114", textStart, {
          message: "Text outside the root element.",
          expected: "only comments and whitespace around the root",
          hint: "move the text inside an element such as <text>",
        });
      } else {
        parent.children.push({ type: "text", text, pos: posAt(textStart) });
      }
    }
    text = "";
    textStart = -1;
  };

  const appendText = (chunk: string, at: number) => {
    if (textStart < 0) {
      const firstSignificant = chunk.search(/[^ \t\n]/);
      if (firstSignificant >= 0) textStart = at + firstSignificant;
    }
    text += chunk;
  };

  const readName = (at: number): string => {
    NAME_CHARS.lastIndex = at;
    return NAME_CHARS.exec(src)?.[0] ?? "";
  };
  const skipWhitespace = (at: number): number => {
    let k = at;
    while (k < n && WHITESPACE.test(src[k] ?? "")) k++;
    return k;
  };
  const checkName = (name: string, at: number, what: string, extra?: string) => {
    if (NAME.test(name)) return;
    const suggestion = name
      .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
      .toLowerCase()
      .replace(/[_:.]+/g, "-");
    report(
      "W105",
      at,
      {
        message: `${what} name "${name}" breaks the name grammar.`,
        expected: "[a-z][a-z0-9]*(-[a-z0-9]+)*",
        got: name,
        hint: NAME.test(suggestion) ? `did you mean "${suggestion}"?` : undefined,
      },
      extra,
    );
  };

  const readAttributeValue = (
    at: number,
    quote: string | undefined,
    name: string,
  ): { value: string; end: number } => {
    let value = "";
    let k = at;
    for (;;) {
      if (k >= n) {
        report(
          "W110",
          at,
          {
            message: `Value of attribute "${name}" is never closed.`,
            expected: 'a closing "',
            hint: 'close it with "',
          },
          `@${name}`,
        );
        return { value, end: k };
      }
      const c = src[k] ?? "";
      if (quote === undefined ? /[\s>]/.test(c) || src.startsWith("/>", k) : c === quote) {
        return { value, end: quote === undefined ? k : k + 1 };
      }
      if (c === "<") {
        report(
          "W113",
          k,
          {
            message: "`<` is not allowed inside an attribute value.",
            expected: "&lt;",
            got: "<",
            hint: 'write "&lt;"',
          },
          `@${name}`,
        );
        value += c;
        k++;
      } else if (c === "&") {
        const ref = readReference(k);
        value += ref.value;
        k = ref.end;
      } else {
        // XML attribute-value normalization: literal tabs and newlines read as spaces.
        value += c === "\t" || c === "\n" ? " " : c;
        k++;
      }
    }
  };

  // Returns the offset after the start tag, or -1 when parsing must stop.
  const readStartTag = (at: number): number => {
    const name = readName(at + 1);
    if (name === "") {
      report("W101", at, {
        message: "`<` must start a tag.",
        expected: "an element name after <",
        hint: 'write "&lt;" for a literal "<"',
      });
      appendText("<", at);
      return at + 1;
    }
    const element: RawElement = { type: "element", name, attrs: [], children: [], pos: posAt(at) };
    const parent = stack.at(-1);
    if (parent !== undefined) parent.children.push(element);
    else if (root === undefined) root = element;
    else
      report("W114", at, {
        message: "A document has exactly one root element.",
        expected: "one root element",
        got: `<${name}>`,
      });
    if (stack.length >= MAX_DEPTH) {
      report("W117", at, {
        message: `Elements nest deeper than ${MAX_DEPTH} levels.`,
        expected: `at most ${MAX_DEPTH}`,
      });
      return -1;
    }
    // The element is open while its attributes are read, so that their paths include it.
    stack.push(element);
    checkName(name, at + 1, "Element");

    let k = at + 1 + name.length;
    const seen = new Set<string>();
    for (;;) {
      const before = k;
      k = skipWhitespace(k);
      if (k >= n) {
        report("W110", at, {
          message: `Start tag <${name}> is never closed.`,
          expected: '">" or "/>"',
          hint: 'end it with ">" or "/>"',
        });
        stack.pop();
        return n;
      }
      if (src.startsWith("/>", k)) {
        stack.pop();
        return k + 2;
      }
      if (src[k] === ">") return k + 1;
      const attrName = readName(k);
      if (attrName === "") {
        report("W101", k, {
          message: `Unexpected "${src[k] ?? ""}" inside <${name}>.`,
          expected: 'an attribute, ">" or "/>"',
          got: src[k],
        });
        k++;
        continue;
      }
      if (k === before) {
        report("W101", k, {
          message: "Attributes must be separated by whitespace.",
          expected: "whitespace",
          hint: `add a space before ${attrName}`,
        });
      }
      const attrAt = k;
      checkName(attrName, attrAt, "Attribute", `@${attrName}`);
      k = skipWhitespace(k + attrName.length);
      let value = "";
      if (src[k] !== "=") {
        report(
          "W107",
          attrAt,
          {
            message: `Attribute "${attrName}" has no value.`,
            expected: `${attrName}="…"`,
            hint: `write ${attrName}="true"`,
          },
          `@${attrName}`,
        );
      } else {
        k = skipWhitespace(k + 1);
        const q = src[k];
        if (q !== '"') {
          report(
            "W106",
            k,
            {
              message: `Value of attribute "${attrName}" must be in double quotes.`,
              expected: "double quotes",
              got: q === "'" ? "single quotes" : "no quotes",
              hint: `write ${attrName}="…"`,
            },
            `@${attrName}`,
          );
        }
        const quote = q === '"' || q === "'" ? q : undefined;
        const start = quote === undefined ? k : k + 1;
        const read = readAttributeValue(start, quote, attrName);
        value = read.value;
        k = read.end;
      }
      if (seen.has(attrName)) {
        report(
          "W108",
          attrAt,
          {
            message: `Attribute "${attrName}" appears twice.`,
            expected: "each attribute once",
            hint: "keep one of them",
          },
          `@${attrName}`,
        );
      } else {
        seen.add(attrName);
        element.attrs.push({ name: attrName, value, pos: posAt(attrAt) });
      }
    }
  };

  const readEndTag = (at: number): number => {
    const name = readName(at + 2);
    let k = skipWhitespace(at + 2 + name.length);
    if (src[k] === ">") k++;
    else {
      report("W101", k, { message: `Closing tag </${name}> must end with ">".`, expected: ">" });
      const close = src.indexOf(">", k);
      k = close < 0 ? n : close + 1;
    }
    const top = stack.at(-1);
    if (top === undefined) {
      report("W111", at, {
        message: `Closing tag </${name}> has no open element.`,
        expected: "no closing tag",
        got: `</${name}>`,
        hint: "remove it",
      });
      return k;
    }
    if (top.name === name) {
      stack.pop();
      return k;
    }
    report("W109", at, {
      message: `Closing tag </${name}> does not match <${top.name}>.`,
      expected: `</${top.name}>`,
      got: `</${name}>`,
      hint: `close <${top.name}> first`,
    });
    // Recover by closing up to a matching open element, if there is one.
    const match = stack.findLastIndex((el) => el.name === name);
    if (match >= 0) stack.length = match;
    return k;
  };

  for (const m of src.matchAll(new RegExp(NON_XML_CHAR.source, "gu"))) {
    const code = (m[0].codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, "0");
    report("W113", m.index, {
      message: `Character U+${code} is not allowed in XML.`,
      expected: "a character XML allows",
      got: `U+${code}`,
      hint: "remove it",
    });
  }

  while (i < n) {
    const c = src[i];
    if (c === "<") {
      if (src.startsWith("<!--", i)) {
        // Comments are not part of the model, so text on both sides forms one run.
        const end = src.indexOf("-->", i + 4);
        if (end < 0) {
          report("W115", i, {
            message: "Comment is never closed.",
            expected: "-->",
            hint: 'end it with "-->"',
          });
          i = n;
          continue;
        }
        const body = src.slice(i + 4, end);
        if (body.includes("--") || body.endsWith("-")) {
          report("W115", i, {
            message: 'A comment must not contain "--" or end with "-".',
            expected: 'a comment without "--"',
          });
        }
        i = end + 3;
        continue;
      }
      flushText();
      if (src.startsWith("<![CDATA[", i)) {
        report("W104", i, {
          message: "CDATA sections are not allowed.",
          expected: "escaped text",
          hint: "write the text with &lt; and &amp; instead",
        });
        const end = src.indexOf("]]>", i);
        i = end < 0 ? n : end + 3;
      } else if (src.startsWith("<!", i)) {
        report("W103", i, {
          message: "DOCTYPE and other declarations are not allowed.",
          expected: "no declaration",
          hint: "remove it",
        });
        let depth = 0;
        let k = i + 2;
        for (; k < n; k++) {
          const d = src[k];
          if (d === "[") depth++;
          else if (d === "]") depth--;
          else if (d === ">" && depth <= 0) break;
        }
        i = k + 1;
      } else if (src.startsWith("<?", i)) {
        report("W102", i, {
          message: "XML declarations and processing instructions are not allowed.",
          expected: "no declaration",
          hint: "remove it",
        });
        const end = src.indexOf("?>", i);
        i = end < 0 ? n : end + 2;
      } else if (src.startsWith("</", i)) {
        i = readEndTag(i);
      } else {
        const next = readStartTag(i);
        if (next < 0) return { diagnostics: diagnostics.toSorted(byPosition) };
        i = next;
      }
      continue;
    }
    if (c === "&") {
      const ref = readReference(i);
      appendText(ref.value, i);
      i = ref.end;
      continue;
    }
    let end = i;
    while (end < n && src[end] !== "<" && src[end] !== "&") end++;
    const chunk = src.slice(i, end);
    const cdataEnd = chunk.indexOf("]]>");
    if (cdataEnd >= 0) {
      report("W113", i + cdataEnd, {
        message: '"]]>" is not allowed in text.',
        expected: "]]&gt;",
        hint: 'write "]]&gt;"',
      });
    }
    appendText(chunk, i);
    i = end;
  }
  flushText();
  for (const el of stack.toReversed()) {
    diagnostics.push(
      diagnostic("W110", {
        message: `<${el.name}> is never closed.`,
        expected: `</${el.name}>`,
        path: pathOf(),
        pos: el.pos,
        hint: `add </${el.name}>`,
      }),
    );
    stack.pop();
  }
  if (root === undefined) {
    diagnostics.push(
      diagnostic("W114", {
        message: "The document has no root element.",
        path: "/",
        pos: posAt(n),
        expected: '<screen id="…" weft="0.1">',
      }),
    );
  }
  return { root, diagnostics: diagnostics.toSorted(byPosition) };
}
