// Schema and semantic layers of SPEC §6, plus the compatibility rules of §8. Validation reports;
// it never throws and never changes the document.
import {
  diagnostic,
  didYouMean,
  oneOf,
  quote,
  type DiagnosticCode,
  type DiagnosticInit,
  type Mode,
  type Position,
} from "./diagnostics.ts";
import {
  DocumentSchema,
  WEFT_VERSION,
  type Catalog,
  type Child,
  type ComponentDef,
  type Diagnostic,
  type Document,
  type Node,
  type PropDef,
  type Value,
} from "./model.ts";
import {
  ACTION,
  ARIA_ROLES,
  BINDING,
  EACH,
  EMBEDDED_REFERENCE,
  EXTENSION_NAME,
  ID,
  LOOP_VARIABLE,
  MAX_DEPTH,
  NAME,
  NON_XML_CHAR,
  own,
  SLOT,
  TOKEN,
  UNIVERSAL_PROPS,
  VERSION,
} from "./rules.ts";
import { pathSegment, type SourceMap } from "./source.ts";

export type ValidateOptions = {
  catalog: Catalog;
  /** `lenient` (default) warns about unknown content; `strict` rejects it (SPEC §8). */
  mode?: Mode | undefined;
  /** Token path → DTCG `$type`. Token references are checked only when this is given. */
  tokens?: ReadonlyMap<string, string> | undefined;
  /** Known host actions. Action names are checked only when this is given. */
  actions?: readonly string[] | undefined;
  /** Positions from `parse`, so that diagnostics carry line and column. */
  source?: SourceMap | undefined;
};

type Owner = {
  /** Component kind the list belongs to; absent above the root. */
  kind?: string | undefined;
  content: ComponentDef["content"];
  allowed?: readonly string[] | undefined;
  /** Named slots hold elements only. */
  slot?: boolean;
  /** Some ancestor is a `form`, which a `submit` button needs (SPEC §5.1). */
  inForm?: boolean;
  /** The list is the body of an `<each>`, which repeats elements only (SPEC §4.3). */
  each?: boolean;
};

type At = { path: string; pos?: Position | undefined };

const [CURRENT_MAJOR, CURRENT_MINOR] = WEFT_VERSION.split(".").map(Number);
const BINDING_GRAMMAR = "$.name(.name)* or $loopVariable(.name)*";

export function validate(input: unknown, options: ValidateOptions): Diagnostic[] {
  const out: Diagnostic[] = [];
  const mode = options.mode ?? "lenient";
  const report = (code: DiagnosticCode, init: DiagnosticInit) =>
    out.push(diagnostic(code, init, mode));

  if (exceedsDepth(input)) {
    report("W200", {
      path: "#",
      message: `The document nests deeper than ${MAX_DEPTH} elements.`,
      expected: `at most ${MAX_DEPTH} levels`,
    });
    return out;
  }
  const shape = DocumentSchema.safeParse(input);
  if (!shape.success) {
    for (const issue of shape.error.issues) {
      report("W200", {
        path: `#/${issue.path.map(String).join("/")}`,
        message: issue.message,
        expected: "the canonical JSON shape of SPEC §3",
      });
    }
    return out;
  }
  // The input itself, not Zod's copy: the source map is keyed by node identity.
  const doc = input as Document;
  const { catalog, tokens, actions, source } = options;
  const components = catalog.components;
  const ids = new Map<string, { kind: string; path: string }>();
  const tabReferences: { value: string; at: At }[] = [];

  const nodeAt = (node: Node, path: string, attr?: string): At => {
    const s = source?.get(node);
    return {
      path: attr === undefined ? path : `${path}/@${attr}`,
      pos: (attr === undefined ? undefined : s?.attrs.get(attr)) ?? s?.pos,
    };
  };

  const checkValue = (
    value: Value,
    def: PropDef | undefined,
    values: readonly string[],
    at: At,
    scope: readonly string[],
  ) => {
    if (typeof value === "object") {
      if ("bind" in value) checkBinding(value.bind, value.not === true, def, at, scope);
      else checkToken(value.token, def, at);
      return;
    }
    if (typeof value === "string") {
      if (value.search(EMBEDDED_REFERENCE) > 0) {
        report("W213", {
          ...at,
          message: "A value is either text or one whole reference, never both.",
          got: value,
          expected: "plain text or one whole reference",
          hint: 'bind the whole value, e.g. <text text="{$.greeting}"/>',
        });
      }
      if (NON_XML_CHAR.test(value))
        report("W221", {
          ...at,
          message: "The value contains a character XML cannot carry.",
          expected: "characters XML allows",
        });
    }
    if (def === undefined) return;
    const got = quote(value);
    switch (def.type) {
      case "string":
        if (typeof value !== "string")
          report("W204", {
            ...at,
            message: "Expected a string.",
            expected: "string",
            got,
            hint: `write "${String(value)}" as text`,
          });
        break;
      case "number":
        if (typeof value !== "number" || !Number.isFinite(value)) {
          report("W204", { ...at, message: "Expected a number.", expected: "a JSON number", got });
        } else checkRange(value, def, at);
        break;
      case "boolean":
        if (typeof value !== "boolean")
          report("W204", { ...at, message: "Expected a boolean.", expected: "true or false", got });
        break;
      case "enum":
        if (typeof value !== "string")
          report("W204", {
            ...at,
            message: "Expected one of the enum values.",
            expected: oneOf(values),
            got,
          });
        else if (!values.includes(value)) {
          report("W203", {
            ...at,
            message: `${got} is not an allowed value.`,
            expected: oneOf(values),
            got,
            hint: didYouMean(value, values),
          });
        }
        break;
      case "token":
        report("W204", {
          ...at,
          message: "Expected a design token reference, not a raw value.",
          expected: `{token.<path>}${def.tokenType === undefined ? "" : ` of type ${def.tokenType}`}`,
          got,
          hint: "use a token such as {token.space.md}",
        });
        break;
    }
  };

  const checkRange = (value: number, def: PropDef, at: At) => {
    const whole = def.integer === true;
    const below = def.min !== undefined && value < def.min;
    const above = def.max !== undefined && value > def.max;
    if (!below && !above && (!whole || Number.isInteger(value))) return;
    const range =
      def.min !== undefined && def.max !== undefined
        ? `from ${def.min} to ${def.max}`
        : def.min !== undefined
          ? `of at least ${def.min}`
          : def.max !== undefined
            ? `of at most ${def.max}`
            : "";
    const expected = `${whole ? "an integer" : "a number"}${range === "" ? "" : ` ${range}`}`;
    let nearest = whole ? Math.round(value) : value;
    if (def.min !== undefined) nearest = Math.max(nearest, whole ? Math.ceil(def.min) : def.min);
    if (def.max !== undefined) nearest = Math.min(nearest, whole ? Math.floor(def.max) : def.max);
    report("W224", {
      ...at,
      message: `${value} is not ${expected}.`,
      expected,
      got: String(value),
      hint: `use ${nearest}`,
    });
  };

  const checkBinding = (
    path: string,
    not: boolean,
    def: PropDef | undefined,
    at: At,
    scope: readonly string[],
  ) => {
    if (!BINDING.test(path)) {
      report("W214", {
        ...at,
        message: "The binding path is malformed.",
        expected: BINDING_GRAMMAR,
        got: path,
      });
    } else if (!path.startsWith("$.")) {
      const variable = path.slice(1).split(".")[0] ?? "";
      if (!scope.includes(variable)) {
        report("W305", {
          ...at,
          message: `Loop variable "${variable}" is not defined by an enclosing <each>.`,
          expected:
            scope.length === 0
              ? "a path starting with $."
              : `$.… or ${oneOf(scope.map((v) => `$${v}`))}`,
          got: path,
          hint: didYouMean(variable, scope),
        });
      }
    }
    if (def === undefined) return;
    if (def.bindable === false) {
      report("W217", {
        ...at,
        message: "This attribute takes a literal only.",
        expected: "a literal",
        got: `{${not ? "!" : ""}${path}}`,
      });
    } else if (not && def.type !== "boolean") {
      report("W218", {
        ...at,
        message: "Only boolean attributes take a negated binding.",
        expected: "a plain binding",
        got: `{!${path}}`,
        hint: `bind {${path}} instead`,
      });
    } else if (not && def.writable === true) {
      report("W218", {
        ...at,
        message: "A two-way attribute cannot take a read-only negated binding.",
        expected: "a plain binding",
        got: `{!${path}}`,
        hint: `bind {${path}} instead`,
      });
    }
  };

  const checkToken = (path: string, def: PropDef | undefined, at: At) => {
    if (def !== undefined && def.type !== "token") {
      report("W204", {
        ...at,
        message: `Expected a ${def.type}, not a token reference.`,
        expected: def.type,
        got: `{token.${path}}`,
      });
    }
    if (!TOKEN.test(path)) {
      report("W215", {
        ...at,
        message: "The token path is malformed.",
        expected: "segments of [A-Za-z0-9_-] joined by dots",
        got: path,
      });
      return;
    }
    if (tokens === undefined) return;
    const type = tokens.get(path);
    if (type === undefined) {
      report("W306", {
        ...at,
        message: `Token "${path}" does not exist.`,
        expected: "a token from the token set",
        got: path,
        hint: didYouMean(path, tokens.keys()),
      });
    } else if (def?.tokenType !== undefined && type !== def.tokenType) {
      report("W307", {
        ...at,
        message: `Token "${path}" has type ${type}.`,
        expected: def.tokenType,
        got: type,
        hint: didYouMean(
          path,
          [...tokens].filter(([, t]) => t === def.tokenType).map(([p]) => p),
        ),
      });
    }
  };

  const checkRole = (value: Value, at: At) => {
    if (typeof value !== "string") {
      report("W217", {
        ...at,
        message: "`role` takes a literal only.",
        expected: "a literal ARIA role",
        got: quote(value),
      });
    } else if (!ARIA_ROLES.includes(value)) {
      report("W211", {
        ...at,
        message: `"${value}" is not a WAI-ARIA role.`,
        expected: "a WAI-ARIA 1.2 role",
        got: value,
        hint: didYouMean(value, ARIA_ROLES),
      });
    }
  };

  const visitList = (
    list: readonly Child[],
    listPath: string,
    owner: Owner,
    scope: readonly string[],
    positions: readonly (Position | undefined)[] | undefined,
  ) => {
    const where =
      owner.each === true
        ? "<each>"
        : owner.slot === true
          ? "This slot"
          : owner.kind === undefined
            ? "This list"
            : `<${owner.kind}>`;
    list.forEach((child, index) => {
      if (typeof child === "string") {
        const at = { path: `${listPath}/#text[${index}]`, pos: positions?.[index] };
        if (
          owner.slot === true ||
          owner.each === true ||
          owner.content === "nodes" ||
          owner.content === "none"
        ) {
          report("W304", {
            ...at,
            message: `${where} does not take text.`,
            expected: owner.content === "none" ? "no content" : "elements",
            got: quote(child),
            hint: 'wrap the text in <text id="…">',
          });
        }
        if (NON_XML_CHAR.test(child))
          report("W221", {
            ...at,
            message: "The text contains a character XML cannot carry.",
            expected: "characters XML allows",
          });
        return;
      }
      const childPath = `${listPath}/${pathSegment(child.kind, child.id, index)}`;
      const at = nodeAt(child, childPath);
      const component = child.kind === EACH ? undefined : own(components, child.kind);
      const opaque = child.kind !== EACH && component === undefined;
      if (owner.content === "none" || owner.content === "text") {
        report("W304", {
          ...at,
          message: `${where} does not take elements.`,
          expected: owner.content === "none" ? "no content" : "text",
          got: `<${child.kind}>`,
        });
      } else if (!opaque && owner.allowed !== undefined && !owner.allowed.includes(child.kind)) {
        // Unknown and extension elements are opaque (SPEC §8), so containment rules skip them.
        report("W302", {
          ...at,
          message: `${where} does not take <${child.kind}>.`,
          expected: oneOf(owner.allowed),
          got: child.kind,
        });
      }
      if (
        component?.allowedParents !== undefined &&
        owner.kind !== undefined &&
        // An unknown or extension parent is opaque (SPEC §8): it may wrap a newer container.
        own(components, owner.kind) !== undefined &&
        !component.allowedParents.includes(owner.kind)
      ) {
        report("W303", {
          ...at,
          message: `<${child.kind}> cannot be placed in <${owner.kind}>.`,
          expected: `a parent ${oneOf(component.allowedParents)}`,
          got: owner.kind,
        });
      }
      visitNode(child, childPath, owner, scope);
    });
  };

  const visitNode = (
    node: Node,
    path: string,
    owner: Owner | undefined,
    scope: readonly string[],
  ) => {
    const { kind } = node;
    const at = nodeAt(node, path);
    const props = node.props ?? {};
    const component = kind === EACH ? undefined : own(components, kind);
    const category =
      kind === EACH
        ? "each"
        : component !== undefined
          ? "component"
          : kind.startsWith("x-")
            ? "extension"
            : "unknown";

    if (category === "unknown") {
      if (kind === SLOT || !NAME.test(kind)) {
        report("W223", {
          ...at,
          message:
            kind === SLOT
              ? "`slot` is structural; named slots live in `slots`."
              : `Kind "${kind}" breaks the name grammar.`,
          expected: "a catalog kind or x-<vendor>-<name>",
          got: kind,
          hint: kind === SLOT ? undefined : didYouMean(kind, Object.keys(components)),
        });
      } else {
        report("W401", {
          ...at,
          message: `<${kind}> is not in catalog ${catalog.name} ${catalog.version}.`,
          expected: "a catalog component or an x-<vendor>- extension",
          got: kind,
          hint:
            didYouMean(kind, Object.keys(components)) ??
            `use a catalog component, or an extension named x-<vendor>-${kind}`,
        });
      }
    } else if (category === "extension" && !EXTENSION_NAME.test(kind)) {
      report("W220", {
        ...at,
        message: `Extension <${kind}> needs a vendor prefix.`,
        expected: "x-<vendor>-<name>",
        got: kind,
      });
    }
    if (owner === undefined && kind !== "screen") {
      report("W201", {
        ...at,
        message: "The root element must be <screen>.",
        expected: "screen",
        got: kind,
      });
    } else if (owner !== undefined && kind === "screen") {
      report("W312", {
        ...at,
        message: "<screen> is allowed only as the root.",
        expected: "a container below the root",
        hint: "use <section> or <stack>",
      });
    }

    if (node.id === undefined) {
      report("W202", {
        ...at,
        message: `<${kind}> has no id.`,
        expected: 'id="…"',
        hint: "add a document-unique id",
      });
    } else if (!ID.test(node.id)) {
      report("W212", {
        ...nodeAt(node, path, "id"),
        message: `Id ${quote(node.id)} breaks the id grammar.`,
        expected: "[A-Za-z][A-Za-z0-9_-]*",
        got: node.id,
      });
    } else {
      const first = ids.get(node.id);
      if (first === undefined) ids.set(node.id, { kind, path });
      else {
        report("W301", {
          ...nodeAt(node, path, "id"),
          message: `Id "${node.id}" is already used.`,
          expected: "a document-unique id",
          got: node.id,
          hint: `first used at ${first.path}; pick another id`,
        });
      }
    }

    for (const [name, value] of Object.entries(props)) {
      const attrAt = nodeAt(node, path, name);
      if (!NAME.test(name) || name === "id" || name.startsWith("on-")) {
        report("W223", {
          ...attrAt,
          message:
            name === "id"
              ? "`id` belongs in Node.id, not in props."
              : name.startsWith("on-")
                ? "Events belong in `on`, not in props."
                : `Attribute name "${name}" breaks the name grammar.`,
          expected: "a prop name matching [a-z][a-z0-9]*(-[a-z0-9]+)*",
          got: name,
        });
        continue;
      }
      if (owner === undefined && name === "weft") {
        report("W200", {
          ...attrAt,
          message: "The format version lives in Document.weft, not in the root's props.",
          expected: "Document.weft",
        });
        continue;
      }
      if (category === "component" && component !== undefined) {
        if (name === "role") {
          report("W209", {
            ...attrAt,
            message: `<${kind}> already has a role from the catalog.`,
            expected: "no role attribute",
            hint: "remove role",
          });
          continue;
        }
        const def = own(component.props, name) ?? own(UNIVERSAL_PROPS, name);
        if (def === undefined) {
          if (!name.startsWith("x-")) {
            const known = [
              ...new Set([...Object.keys(component.props ?? {}), "label", "hidden", "state"]),
            ];
            report("W402", {
              ...attrAt,
              message: `<${kind}> has no attribute "${name}".`,
              expected: oneOf(known),
              got: name,
              hint: didYouMean(name, known),
            });
          }
          checkValue(value, undefined, [], attrAt, scope);
          continue;
        }
        const values =
          name === "state" && own(component.props, name) === undefined
            ? (component.states ?? [])
            : (def.values ?? []);
        checkValue(value, def, values, attrAt, scope);
        continue;
      }
      if (category === "each" && name !== "in" && name !== "as" && !name.startsWith("x-")) {
        report("W402", {
          ...attrAt,
          message: `<each> has no attribute "${name}".`,
          expected: oneOf(["in", "as"]),
          got: name,
          hint: didYouMean(name, ["in", "as"]),
        });
      }
      if (category !== "each" && name === "role") checkRole(value, attrAt);
      else checkValue(value, undefined, [], attrAt, scope);
    }

    let innerScope = scope;
    if (category === "component" && component !== undefined) {
      for (const [name, def] of Object.entries(component.props ?? {})) {
        if (
          def.required !== true ||
          Object.hasOwn(props, name) ||
          (owner === undefined && name === "weft")
        )
          continue;
        report("W205", { ...at, message: `<${kind}> needs "${name}".`, expected: `${name}="…"` });
      }
      if (
        component.requiresLabel === true &&
        !Object.hasOwn(props, "label") &&
        own(component.props, "label")?.required !== true
      ) {
        report("W205", {
          ...at,
          message: `<${kind}> needs an accessible name.`,
          expected: 'label="…"',
        });
      }
      if (
        (component.content === "text" || component.content === "mixed") &&
        Object.hasOwn(props, "text") &&
        (node.children?.length ?? 0) > 0
      ) {
        // SPEC §5.1 note; the catalog format cannot express "content or the text prop".
        report("W310", {
          ...nodeAt(node, path, "text"),
          message: `<${kind}> takes its text from content or from the text attribute, not both.`,
          expected: "content or text, not both",
          hint: "remove the content or the text attribute",
        });
      }
      if (props["submit"] === true && owner?.inForm !== true) {
        report("W313", {
          ...nodeAt(node, path, "submit"),
          message: `<${kind}> submits a form but has no enclosing <form>.`,
          expected: "an enclosing <form>",
          hint: 'move it into a <form>, or remove submit="true" and give it on-press',
        });
      }
      const selected = props["selected"];
      // SPEC §6 layer 3: the catalog format has no id-reference type yet, so this rule is by kind.
      if (kind === "tabs" && typeof selected === "string")
        tabReferences.push({ value: selected, at: nodeAt(node, path, "selected") });
    } else if (category === "extension" && !Object.hasOwn(props, "role")) {
      report("W210", {
        ...at,
        message: `Extension <${kind}> needs a role as its fallback.`,
        expected: 'role="…"',
        hint: 'add role="group"',
      });
    } else if (category === "each") {
      const items = props["in"];
      if (
        items === undefined ||
        typeof items !== "object" ||
        !("bind" in items) ||
        items.not === true
      ) {
        report("W222", {
          ...nodeAt(node, path, "in"),
          message: "<each> needs `in` bound to an array.",
          expected: 'in="{$.items}"',
          got: items === undefined ? undefined : quote(items),
        });
      }
      const variable = props["as"];
      if (typeof variable !== "string" || !LOOP_VARIABLE.test(variable)) {
        report("W222", {
          ...nodeAt(node, path, "as"),
          message: "<each> needs `as` naming the loop variable.",
          expected: "[a-z][A-Za-z0-9]*",
          got: variable === undefined ? undefined : quote(variable),
        });
      } else if (scope.includes(variable)) {
        report("W311", {
          ...nodeAt(node, path, "as"),
          message: `Loop variable "${variable}" is already defined by an outer <each>.`,
          expected: `a name other than ${oneOf(scope)}`,
          got: variable,
          hint: "pick another name",
        });
      } else {
        innerScope = [...scope, variable];
      }
      if (!(node.children ?? []).some((child) => typeof child !== "string")) {
        report("W314", {
          ...at,
          message: "<each> has no element to repeat.",
          expected: "one or more child elements",
          hint: "put the element to repeat inside <each>, or remove it",
        });
      }
    }

    const events = category === "component" ? (component?.events ?? []) : [];
    for (const [event, action] of Object.entries(node.on ?? {})) {
      const eventAt = nodeAt(node, path, `on-${event}`);
      if (!NAME.test(event)) {
        report("W223", {
          ...eventAt,
          message: `Event name "${event}" breaks the name grammar.`,
          expected: "[a-z0-9]+(-[a-z0-9]+)*",
          got: event,
        });
      } else if ((category === "component" || category === "each") && !events.includes(event)) {
        report("W206", {
          ...eventAt,
          message: `<${kind}> has no event "${event}".`,
          expected: oneOf(events),
          got: event,
          hint: didYouMean(event, events),
        });
      }
      if (!ACTION.test(action)) {
        report("W216", {
          ...eventAt,
          message: "The action name is malformed.",
          expected: "[a-z][A-Za-z0-9]*(.[a-z][A-Za-z0-9]*)*",
          got: action,
        });
      } else if (actions !== undefined && !actions.includes(action)) {
        report("W308", {
          ...eventAt,
          message: `Action "${action}" is not provided by the host.`,
          expected: oneOf(actions),
          got: action,
          hint: didYouMean(action, actions),
        });
      }
    }

    const inForm = owner?.inForm === true || kind === "form";
    const slots = node.slots ?? {};
    const slotSources = source?.get(node)?.slots;
    for (const [name, list] of Object.entries(slots)) {
      const slotPath = `${path}/slot[${name}]`;
      const slotAt = { path: slotPath, pos: slotSources?.get(name)?.pos ?? at.pos };
      const declared = category === "component" ? own(component?.slots, name) : undefined;
      if (!NAME.test(name)) {
        report("W223", {
          ...slotAt,
          message: `Slot name "${name}" breaks the name grammar.`,
          expected: "[a-z][a-z0-9]*(-[a-z0-9]+)*",
          got: name,
        });
      } else if ((category === "component" || category === "each") && declared === undefined) {
        const names = Object.keys(component?.slots ?? {});
        report("W207", {
          ...slotAt,
          message: `<${kind}> has no slot "${name}".`,
          expected: oneOf(names),
          got: name,
          hint: didYouMean(name, names),
        });
      }
      const slotOwner: Owner = {
        kind: category === "each" ? owner?.kind : kind,
        content: "nodes",
        allowed: declared?.allowedChildren,
        slot: true,
        inForm,
      };
      visitList(list, slotPath, slotOwner, innerScope, slotSources?.get(name)?.children);
    }
    if (category === "component") {
      for (const [name, def] of Object.entries(component?.slots ?? {})) {
        if (def.required === true && !Object.hasOwn(slots, name)) {
          report("W208", {
            ...at,
            message: `<${kind}> needs slot "${name}".`,
            expected: `<slot name="${name}">`,
          });
        }
      }
    }

    // `<each>` is transparent: its children answer to the list that holds the `<each>` (SPEC §4.3).
    const childOwner: Owner =
      category === "component" && component !== undefined
        ? { kind, content: component.content, allowed: component.allowedChildren, inForm }
        : category === "each"
          ? { ...(owner ?? { content: "mixed" }), each: true }
          : { kind, content: "mixed", inForm };
    visitList(node.children ?? [], path, childOwner, innerScope, source?.get(node)?.children);
  };

  const rootPath = `/${pathSegment(doc.root.kind, doc.root.id)}`;
  checkVersion(doc.weft, nodeAt(doc.root, rootPath, "weft"), report);
  visitNode(doc.root, rootPath, undefined, []);

  const tabIds = [...ids].filter(([, entry]) => entry.kind === "tab").map(([id]) => id);
  for (const { value, at } of tabReferences) {
    if (ids.get(value)?.kind !== "tab") {
      report("W309", {
        ...at,
        message: `"${value}" is not the id of a <tab>.`,
        expected: oneOf(tabIds),
        got: value,
        hint: didYouMean(value, tabIds),
      });
    }
  }
  return out;
}

function checkVersion(
  weft: string,
  at: At,
  report: (code: DiagnosticCode, init: DiagnosticInit) => void,
) {
  const match = VERSION.exec(weft);
  const fix = `weft="${WEFT_VERSION}"`;
  if (weft === "") {
    report("W205", { ...at, message: "The root needs the format version.", expected: fix });
  } else if (match === null) {
    report("W219", {
      ...at,
      message: "The format version must be major.minor.",
      expected: "major.minor",
      got: weft,
      hint: `write ${fix}`,
    });
  } else if (Number(match[1]) !== CURRENT_MAJOR) {
    report("W404", {
      ...at,
      message: `Version ${weft} is not readable by a ${WEFT_VERSION} reader.`,
      expected: `${CURRENT_MAJOR}.x`,
      got: weft,
    });
  } else if (Number(match[2]) > (CURRENT_MINOR ?? 0)) {
    report("W403", {
      ...at,
      message: `Version ${weft} is newer than ${WEFT_VERSION}; unknown content is read as extensions.`,
      expected: `at most ${WEFT_VERSION}`,
      got: weft,
    });
  }
}

/** Iterative, so that hostile JSON cannot overflow the stack before validation starts. */
function exceedsDepth(input: unknown): boolean {
  // A node level costs at most three JSON levels: node → slots → list → node.
  const limit = MAX_DEPTH * 3 + 4;
  const stack: [unknown, number][] = [[input, 0]];
  for (let top = stack.pop(); top !== undefined; top = stack.pop()) {
    const [value, depth] = top;
    if (depth > limit) return true;
    if (typeof value === "object" && value !== null) {
      for (const child of Object.values(value)) stack.push([child, depth + 1]);
    }
  }
  return false;
}
