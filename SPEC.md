# Weft 0.1 — specification (draft)

Weft is an open format for describing user interfaces so that AI agents can read, write, validate and patch them. One semantic model has two serializations:

- **Weft markup** (`.weft`) — a strict XML subset with a closed vocabulary. This is what models read and write.
- **Canonical JSON** (`.weft.json`) — the same tree as typed data. This is what tools validate, diff and render.

`parse(markup) → JSON` and `serialize(JSON) → markup` are lossless inverses of each other, except for comments and insignificant whitespace.

Key words MUST, SHOULD and MAY are used as in RFC 2119.

## 1. Design rules

1. **Closed vocabulary.** Every element is a component from a catalog. One element name has one meaning.
2. **One way to say a thing.** No shorthand forms, no optional syntax variants.
3. **No code.** A document contains data bindings and named actions, never expressions that compute or scripts that run.
4. **Semantics come from ARIA.** Every component has an ARIA role; states use ARIA vocabulary where it exists.
5. **Every element is addressable.** Every element has a document-unique `id`.
6. **Values of design come from tokens.** Colors, sizes and spacing are references to Design Tokens (DTCG 2025.10), never raw values.

## 2. Markup syntax

A Weft document is a well-formed XML 1.0 document restricted as follows:

- UTF-8. No XML declaration, DOCTYPE, processing instructions, CDATA sections or entity declarations. A leading byte order mark is ignored; line ends are normalized to LF as in XML.
- Only the five predefined entities (`&lt; &gt; &amp; &quot; &apos;`) and numeric character references.
- Attribute values are always double-quoted. Bare attributes (`<x required>`) are an error.
- Element and attribute names match `[a-z][a-z0-9]*(-[a-z0-9]+)*`. Extension elements and attributes start with `x-` (see §8).
- Comments are allowed anywhere XML allows them and are not part of the model.
- An element with no content MUST be written self-closing by the serializer; the parser accepts both forms.
- Whitespace-only text between elements is insignificant. Other text is trimmed and inner runs of whitespace collapse to one space. Whitespace means the XML whitespace characters (space, tab, CR, LF) only. Text on both sides of a comment is one run; two text runs that end up adjacent in one list (for example around a `<slot>`) join with one space.
- In attribute values, literal tabs and line breaks read as spaces (XML attribute-value normalization); write `&#9;` or `&#10;` to keep them.
- Elements nest at most 256 levels deep.
- Diagnostic positions are 1-based; columns count UTF-16 code units.

The root element is `<screen>` and carries the format version. Every example in this document is in canonical form (§3):

```xml
<screen id="login" label="Sign in" weft="0.1">
  <form id="f1" state="idle" on-submit="auth.submit">
    <field id="email" label="Email" required="true" type="email" value="{$.email}"/>
    <button id="go" disabled="{!$.email}" variant="primary" on-press="auth.submit">Sign in</button>
    <slot name="footer">
      <link id="reset" on-press="nav.reset">Forgot password?</link>
    </slot>
  </form>
</screen>
```

### 2.1 Attribute values

An attribute value is exactly one of:

| Form | Example | Meaning |
| --- | --- | --- |
| Literal | `label="Email"` | A constant, typed by the catalog (string, number, boolean, enum). |
| Binding | `value="{$.user.email}"` | Reads (and for input values, writes) a path in the host data model. |
| Negated binding | `disabled="{!$.email}"` | Boolean negation of a binding. Read-only. |
| Token reference | `gap="{token.space.md}"` | A DTCG token path. |

- A value that starts with `{{` is a literal: the first `{` is dropped, so a literal that must start with `{` is written `{{`. Any other value that starts with `{` MUST be a whole reference, `{$…}`, `{!$…}` or `{token.…}`; anything else is an error. Values that do not start with `{` are literals.
- These forms apply to prop values only. `id`, `on-*` and the slot `name` are always plain text.
- Binding path grammar: `$` (`.` name)+ for the root data model, or `$` name (`.` name)* for a loop variable introduced by `<each>` (§4.3). `name` is `[A-Za-z_][A-Za-z0-9_]*` or a non-negative integer index.
- Mixing text and bindings in one value (`"Hello {$.name}"`) is an error: a literal must not contain `{$`, `{!$` or `{token.` after its first character. Use `<text value="{$.name}"/>`.
- Token path grammar: segments of `[A-Za-z0-9_-]+` joined by `.`.
- Booleans are `true` / `false`. Numbers are JSON numbers.

### 2.2 Universal attributes

Allowed on every element:

| Attribute | Type | Meaning |
| --- | --- | --- |
| `id` | string, required | Document-unique, matches `[A-Za-z][A-Za-z0-9_-]*`. Stable across edits. |
| `label` | string | Accessible name when the element has no visible text. |
| `hidden` | boolean | Not rendered and not exposed to assistive technology. |
| `state` | enum | One of the states the component declares (§5). |
| `role` | ARIA role | Required on extension elements; an error on catalog components. |
| `on-<event>` | action name | Binds an event the component declares to a named host action. |

Action names match `[a-z][A-Za-z0-9]*(\.[a-z][A-Za-z0-9]*)*`. Actions take no arguments in the document; the host receives the action name, the element id, and for elements inside `<each>` the current loop item path.

`<slot>` and `<each>` are structural elements, not components, and take none of the universal attributes except that `<each>` has an `id`:

- `<slot name="…">` takes exactly one attribute, `name`, which follows the name grammar. A slot holds elements only, no text.
- `<each id="…" in="{…}" as="…">` takes `id`, `in` and `as` (§4.3).

## 3. Canonical JSON

```ts
type Document = { weft: "0.1"; root: Node };  // `weft` is the root element's `weft` attribute

type Node = {
  kind: string;                      // element name
  id?: string;                       // absent only on structural nodes without id
  props?: Record<string, Value>;     // every attribute except id and on-*
  on?: Record<string, string>;       // event → action name
  slots?: Record<string, Child[]>;   // named slots
  children?: Child[];                // default slot
};

type Child = Node | string;          // string = text

type Value =
  | string | number | boolean
  | { bind: string; not?: true }     // path without braces, e.g. "$.user.email"
  | { token: string };               // e.g. "space.md"
```

Canonical form rules, so that equal documents are byte-equal:

- Object keys in the order shown above; `props`, `on` and `slots` keys sorted lexicographically by UTF-16 code unit.
- Empty `props`, `on`, `slots`, `children` and empty slot lists are omitted.
- Text children are whitespace-normalized as in §2, and adjacent text children are joined with one space. `-0` is written `0`.
- Props whose value equals the catalog default are kept as written (no default elision).
- The `weft` attribute of the root element is `Document.weft` and never appears in the root's `props`.
- The JSON text is indented by two spaces and ends with a newline.
- Markup serialization writes attributes as: `id`, then props sorted (the root's `weft` sorts with them), then `on-*` sorted; two-space indentation; named slots after default-slot children, sorted by name. An element with no content is self-closing; an element whose only content is one text child is written on one line; otherwise every child goes on its own line. The text ends with a newline.
- Escaping: in attribute values `&`, `<`, `"`, tab, LF and CR are written as references; in text `&`, `<` and `>` are. No other references are written.

Literal typing needs the catalog: `level="2"` is the number `2` only because `heading.level` is declared a number. For extension elements and unknown attributes, literals stay strings.

## 4. Structure

### 4.1 Children and the default slot

Content placed directly inside an element is its default slot. The catalog says what a default slot accepts: `none`, `text`, `nodes` or `mixed` (text and nodes).

### 4.2 Named slots

```xml
<dialog id="confirm" label="Delete file?">
  <text id="t1">This cannot be undone.</text>
  <slot name="actions">
    <button id="cancel" on-press="dialog.close">Cancel</button>
    <button id="ok" variant="danger" on-press="file.delete">Delete</button>
  </slot>
</dialog>
```

- `<slot name="…">` MUST be a direct child of a component that declares that slot.
- A slot name appears at most once per parent.

### 4.3 Repetition

```xml
<list id="todos">
  <each id="todo-each" as="todo" in="{$.todos}">
    <item id="todo-item">
      <text id="todo-title" value="{$todo.title}"/>
    </item>
  </each>
</list>
```

- `in` MUST be a (non-negated) binding to an array. `as` names the loop variable, matching `[a-z][A-Za-z0-9]*`; it MUST NOT reuse the name of an enclosing loop variable.
- Ids inside `<each>` are template ids: unique in the document, repeated per item at render time. A rendered instance is addressed as `id[index]`.
- `<each>` is transparent for parent/child rules: its children are validated as children of its parent. `<each>` itself must be allowed by the parent's `allowedChildren` when that list is given.
- Inside nested `<each>` elements the indexes are appended outermost first: `id[outer][inner]`. The loop item path a host receives with an action is the absolute data path of the innermost item, e.g. `$.todos.2`.

## 5. Catalog

A catalog is a JSON document that defines the vocabulary. It is modelled on Custom Elements Manifest.

```ts
type Catalog = {
  weft: "0.1";
  name: string;
  version: string;                         // semver of the catalog
  components: Record<string, ComponentDef>;
};

type ComponentDef = {
  description: string;                     // one sentence, written for a model
  role: string;                            // ARIA role, or "none"
  content: "none" | "text" | "nodes" | "mixed";
  allowedChildren?: string[];              // kinds; absent = any
  allowedParents?: string[];               // kinds; absent = any
  requiresLabel?: boolean;                 // the universal `label` attribute is mandatory
  props?: Record<string, PropDef>;
  slots?: Record<string, SlotDef>;
  states?: string[];                       // values allowed for `state`
  events?: string[];                       // names allowed after `on-`
};

type PropDef = {
  description: string;
  type: "string" | "number" | "boolean" | "enum" | "token";
  values?: string[];                       // for enum
  tokenType?: string;                      // for token: DTCG $type, e.g. "dimension"
  required?: boolean;
  default?: string | number | boolean;
  bindable?: boolean;                      // default true; false = literal only
  writable?: boolean;                      // true = two-way binding target
};

type SlotDef = { description: string; allowedChildren?: string[]; required?: boolean };
```

### 5.1 Core catalog `weft-core` 0.1

Props are strings unless a type is given. `*` marks required props.

| Kind | Role | Content | Props | Slots | States | Events |
| --- | --- | --- | --- | --- | --- | --- |
| `screen` | `main` | nodes | `weft`* | — | `ready`, `loading`, `error` | — |
| `stack` | `none` | nodes | `direction` enum `column`/`row` (default `column`), `gap` token dimension, `align` enum `start`/`center`/`end`/`stretch`, `wrap` boolean | — | — | — |
| `grid` | `none` | nodes | `columns`* number, `gap` token dimension | — | — | — |
| `section` | `region` | nodes | — (needs `label`) | `header` | — | — |
| `heading` | `heading` | text | `level`* number 1–6, `value` | — | — | — |
| `text` | `none` | text | `value`, `tone` enum `default`/`muted`/`success`/`warning`/`danger` | — | — | — |
| `image` | `img` | none | `src`*, `label`* | — | — | — |
| `link` | `link` | text | `href` | — | — | `press` |
| `button` | `button` | text | `variant` enum `primary`/`secondary`/`danger` (default `secondary`), `disabled` boolean | — | `idle`, `busy` | `press` |
| `form` | `form` | nodes | — | `footer` | `idle`, `submitting`, `invalid` | `submit` |
| `field` | `textbox` | none | `label`*, `type` enum `text`/`email`/`password`/`number`/`search`/`multiline` (default `text`), `value` writable, `placeholder`, `required` boolean, `disabled` boolean, `error` | — | `valid`, `invalid` | `change` |
| `checkbox` | `checkbox` | none | `label`*, `checked` boolean writable, `disabled` boolean | — | — | `change` |
| `switch` | `switch` | none | `label`*, `checked` boolean writable, `disabled` boolean | — | — | `change` |
| `radio-group` | `radiogroup` | nodes (`radio`, `each`) | `label`*, `value` writable | — | — | `change` |
| `radio` | `radio` | text | `value`*, `disabled` boolean | — | — | — |
| `select` | `combobox` | nodes (`option`, `each`) | `label`*, `value` writable, `disabled` boolean | — | — | `change` |
| `option` | `option` | text | `value`* | — | — | — |
| `list` | `list` | nodes (`item`, `each`) | `ordered` boolean | — | `ready`, `loading`, `empty` | — |
| `item` | `listitem` | mixed | — | — | — | `press` |
| `table` | `table` | nodes (`column`, `row`, `each`) | — (needs `label`) | — | `ready`, `loading`, `empty` | — |
| `column` | `columnheader` | text | `sort` enum `none`/`ascending`/`descending` | — | — | `press` |
| `row` | `row` | nodes (`cell`) | `selected` boolean | — | — | `press` |
| `cell` | `cell` | mixed | — | — | — | — |
| `tabs` | `tablist` | nodes (`tab`) | `selected` writable (id of the selected `tab`) | — | — | `change` |
| `tab` | `tab` | nodes | `label`* | — | — | — |
| `dialog` | `dialog` | nodes | `modal` boolean (default `true`), `open` boolean writable (needs `label`) | `actions` | — | `close` |
| `alert` | `alert` | mixed | `tone` enum `info`/`success`/`warning`/`danger` (default `info`) | — | — | — |
| `menu` | `menu` | nodes (`menu-item`, `each`) | `label`* | — | — | — |
| `menu-item` | `menuitem` | text | `disabled` boolean | — | — | `press` |

Notes:

- A `tab` element holds its panel content; a renderer emits `tab` and `tabpanel` from it.
- `text` and `heading` take their text either as content or as `value` (for bindings), never both.
- "needs `label`" and a starred `label`* in the Props column both mean the universal `label` attribute is required for that component (`requiresLabel` in the catalog); `label` is never declared in `props`.
- `(…)` after a content model lists the only kinds allowed as direct children. `each` is transparent (§4.3), so it is allowed wherever its own children would be, whether listed or not.
- A kind with a required parent context (`radio`, `option`, `item`, `column`, `row`, `cell`, `tab`, `menu-item`) declares it as `allowedParents`: `radio` in `radio-group`, `option` in `select`, `item` in `list`, `column` and `row` in `table`, `cell` in `row`, `tab` in `tabs`, `menu-item` in `menu`.
- `field` has role `textbox`; a renderer MAY refine it from `type` (`number` → `spinbutton`, `search` → `searchbox`) as ARIA requires.
- `column` is a direct child of `table` although ARIA places `columnheader` inside a `row`; the renderer emits the header row. On the web the header row sits in a header `rowgroup` and the rows in a body `rowgroup` (`<thead>`/`<tbody>`), and those groups are part of the declared tree.
- `tabs.selected` names the selected `tab` by `id`; when it is absent or names no tab, the first tab is selected. Only the selected tab's panel is exposed.
- `form` and `section` are landmarks only when they have a `label`: ARIA exposes `form` and `region` only with an accessible name, so an unlabelled `form` has no role of its own (its children are exposed directly).
- A `dialog` is shown only while `open` is true; an absent `open` means closed, like every boolean prop without a default.
- A `select` always has one option selected: the one whose `value` equals `select.value`, otherwise the first.
- `heading.level` is an integer from 1 to 6; the catalog shape has no range, so the validator enforces it.

## 6. Validation

Validation has three layers, each reporting diagnostics rather than throwing:

1. **Syntax** — §2. The document is well-formed restricted XML.
2. **Schema** — the tree matches the catalog: known kinds, known and correctly typed props, required props present, declared slots, states and events.
3. **Semantics** — unique ids, parent/child rules, binding paths resolve to a loop variable in scope, token references exist in the supplied token set (when one is supplied), action names exist in the supplied action list (when one is supplied), `selected`/id references point at existing elements (`tabs.selected` names a `tab`), `<screen>` only at the root, and `text`/`heading` take their text from content or `value`, not both.

A document that does not have the JSON shape of §3 gets `W200` diagnostics only; the other checks need the shape.

### 6.1 Diagnostics

```ts
type Diagnostic = {
  code: string;                 // stable, e.g. "W203"
  severity: "error" | "warning";
  message: string;              // one sentence, says what is wrong
  path: string;                 // e.g. "/screen#login/form#f1/@state"
  line?: number;                // 1-based, when the input was markup
  column?: number;
  expected?: string;            // e.g. 'one of: "idle", "submitting", "invalid"'
  got?: string;
  hint?: string;                // the smallest fix, e.g. 'did you mean "submitting"?'
};
```

Code ranges: `W1xx` syntax, `W2xx` schema, `W3xx` semantics, `W4xx` compatibility. A code, once published, never changes meaning.

`path` addresses the element from the root: one segment per element, `kind#id`, or `kind[index]` when the element has no valid id (the index counts the parent's list, text included). A named slot adds `slot[name]`, a text child `#text[index]`, an attribute `@name` (`@on-press` for events, `@weft` for the version). Syntax diagnostics name the open elements only. For JSON that does not have the shape of §3, the path is a JSON Pointer prefixed with `#`, e.g. `#/root/children/0/kind`.

### 6.2 Codes

`mode` severity is a warning in lenient mode and an error in strict mode (§8). Every other code is an error.

| Code | Meaning |
| --- | --- |
| W101 | Malformed markup: a stray `<`, a broken tag, attributes not separated by whitespace. |
| W102 | XML declaration or processing instruction. |
| W103 | DOCTYPE or another markup declaration. |
| W104 | CDATA section. |
| W105 | Element or attribute name breaks the name grammar. |
| W106 | Attribute value not in double quotes. |
| W107 | Attribute without a value. |
| W108 | Duplicate attribute. |
| W109 | Closing tag does not match the open element. |
| W110 | Element, start tag or attribute value never closed. |
| W111 | Closing tag without an open element. |
| W112 | Unknown entity, malformed or disallowed character reference, bare `&`. |
| W113 | Character not allowed: outside the XML character range, `<` in an attribute value, `]]>` in text. |
| W114 | Content outside the single root element, a second root, or no root. |
| W115 | Unterminated comment, or `--` inside a comment. |
| W116 | Value starts with `{` but is not a whole reference (§2.1). |
| W117 | Nesting deeper than 256 levels. |
| W118 | `<slot>` misplaced (root, inside `<each>` or another `<slot>`) or malformed (no valid `name`, other attributes). |
| W119 | The same slot name twice under one parent. |
| W200 | Document does not have the JSON shape of §3 (or nests too deep, or the root's `props` holds `weft`). |
| W201 | Root element is not `screen`. |
| W202 | Element without `id`. |
| W203 | Value not one of the enum values or declared states. |
| W204 | Value of the wrong type. |
| W205 | Required prop, `label` or the root's `weft` missing. |
| W206 | Event not declared by the component. |
| W207 | Slot not declared by the component. |
| W208 | Required slot missing. |
| W209 | `role` on a catalog component. |
| W210 | Extension element without `role`. |
| W211 | `role` value is not a WAI-ARIA 1.2 role. |
| W212 | Id breaks the id grammar. |
| W213 | Text and a reference mixed in one value. |
| W214 | Binding path breaks the binding grammar. |
| W215 | Token path breaks the token grammar. |
| W216 | Action name breaks the action grammar. |
| W217 | Binding on a literal-only prop (`bindable: false`, `role`). |
| W218 | Negated binding on a non-boolean or two-way prop. |
| W219 | `weft` version is not `major.minor`. |
| W220 | Extension name lacks the `x-<vendor>-` prefix. |
| W221 | String holds a character XML cannot carry. |
| W222 | `<each>` without a binding `in` or a valid `as`. |
| W223 | Kind, prop, event or slot name invalid or reserved (`slot` as a kind, `id` or `on-*` in `props`). |
| W301 | Duplicate id. |
| W302 | Child kind not in the parent's (or slot's) `allowedChildren`. |
| W303 | Parent kind not in the child's `allowedParents`. |
| W304 | Content breaks the content model: text where only elements go, elements where only text goes, anything in `none`. |
| W305 | Binding uses a loop variable that is not in scope. |
| W306 | Token not in the supplied token set. |
| W307 | Token `$type` differs from the prop's `tokenType`. |
| W308 | Action not in the supplied action list. |
| W309 | Id reference points at no suitable element. |
| W310 | `text` or `heading` has both content and `value`. |
| W311 | Loop variable shadows an enclosing one. |
| W312 | `screen` below the root. |
| W401 | Unknown element (mode). |
| W402 | Unknown attribute (mode). |
| W403 | Newer minor version of the format (mode). |
| W404 | Unsupported major version. |

Diagnostics are written for a model that will repair the document: they name the exact location, the expectation and the nearest valid alternative.

## 7. Patches

Agents edit documents with patches addressed by `id`:

```ts
type Patch =
  | { op: "set"; id: string; prop: string; value: Value | null }     // null removes the prop
  | { op: "insert"; parent: string; slot?: string; index?: number; markup: string }
  | { op: "remove"; id: string }
  | { op: "move"; id: string; parent: string; slot?: string; index?: number };
```

A patch list applies atomically: the result is validated, and if it has errors nothing is applied.

## 8. Versioning and extensibility

- `weft` on `<screen>` is `major.minor`. A minor version only adds; a reader of `0.x` MUST accept any `0.y` document under the rules below. A major version may break.
- **Extensions** are elements or attributes whose name starts with `x-<vendor>-`. An extension element MUST carry `role` (its ARIA fallback) and follows `content: "mixed"`; it may have any attributes, slots and events, and its literals stay strings. A reader that does not know it renders its children inside a container with that role. Extension attributes are allowed on every element in both modes.
- **Unknown, non-extension** elements or attributes come from a newer minor version or another catalog. In *lenient* mode (default for readers) they produce a `W4xx` warning; an unknown element is treated as an extension with role `group`, an unknown attribute is kept in the model and ignored. In *strict* mode (default for writers and CI) they are errors. A newer minor `weft` version is reported the same way. Unknown and extension elements are opaque: parent/child rules skip them, but the parent's content model still applies. Undeclared events, slots, states and enum values of a known component are schema errors, not compatibility warnings.
- A reader MUST NOT drop unknown content when it round-trips a document.
- A catalog has its own semver. Removing a component, a prop, an enum value, a slot, a state or an event is a major change; adding one is minor.
- The rule does not spell out every case, so the following are fixed. A change is **major** when a document valid against the previous catalog can become invalid or mean something else, **minor** when it only admits more documents, and **none** when no document is affected. The version bump of a catalog is the highest level among its changes.
  - Major: a prop becomes required; a new prop or slot is required; a prop's `type` or `tokenType` changes; a prop's `default` changes, appears or disappears; a component's `role` changes; `requiresLabel` turns on; `bindable` turns off; `writable` turns off.
  - Content narrowing is major and widening is minor. For `content`, `mixed` accepts everything `text` and `nodes` accept, and `none` accepts nothing; a change to a model that does not accept everything the old one did is narrowing. For `allowedChildren` and `allowedParents` (component or slot), an absent list means any: adding a list or removing a kind narrows, removing the list or adding a kind widens.
  - Minor: a prop stops being required; a slot stops being required; `requiresLabel` turns off; `bindable` or `writable` turns on; a numeric range widens.
  - A numeric range narrows when its lower bound rises, its upper bound falls, or a bound appears; the opposite is widening. A prop field the classifier does not know is major when it changes.
  - A change to a `description` only is none.
- A host advertises `{ weft, catalogs: [{ name, version }] }`; an agent writes only what the host advertises.

## 9. Mapping

- **To code:** a renderer maps each kind to a platform component. The reference renderer targets React and MUST produce an accessibility tree whose roles and names equal those the document declares. Text that is not part of a name appears in that tree as text runs; `stack`, `grid`, `text` and other role-`none` elements add no node of their own.
- **States:** a renderer maps a state to ARIA where an equivalent exists (`loading`, `busy`, `submitting` → `aria-busy`; field `invalid` or a non-empty `error` → `aria-invalid`) and exposes every state as `data-state` as well.
- **Trust:** a renderer never interprets document strings as markup or code. URLs in `link.href` and `image.src` are used only when they are `http`, `https`, `mailto` or relative; any other value is dropped.
- **From a running UI:** an accessibility snapshot maps back to Weft with losses (layout, tokens, bindings and actions are not recoverable). The lossy fields are listed by the importer.
