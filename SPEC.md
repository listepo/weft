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

- UTF-8. No XML declaration, DOCTYPE, processing instructions, CDATA sections or entity declarations.
- Only the five predefined entities (`&lt; &gt; &amp; &quot; &apos;`) and numeric character references.
- Attribute values are always double-quoted. Bare attributes (`<x required>`) are an error.
- Element and attribute names match `[a-z][a-z0-9]*(-[a-z0-9]+)*`. Extension elements and attributes start with `x-` (see §8).
- Comments are allowed anywhere XML allows them and are not part of the model.
- An element with no content MUST be written self-closing by the serializer; the parser accepts both forms.
- Whitespace-only text between elements is insignificant. Other text is trimmed and inner runs of whitespace collapse to one space.

The root element is `<screen>` and carries the format version:

```xml
<screen id="login" weft="0.1" label="Sign in">
  <form id="f1" on-submit="auth.submit" state="idle">
    <field id="email" type="email" label="Email" value="{$.email}" required="true"/>
    <button id="go" variant="primary" disabled="{!$.email}" on-press="auth.submit">Sign in</button>
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

- A value that starts with `{` and ends with `}` is a binding or token reference; anything else is a literal. A literal that must start with `{` is written `{{`.
- Binding path grammar: `$` (`.` name)+ for the root data model, or `$` name (`.` name)* for a loop variable introduced by `<each>` (§4.3). `name` is `[A-Za-z_][A-Za-z0-9_]*` or a non-negative integer index.
- Mixing text and bindings in one value (`"Hello {$.name}"`) is an error. Use `<text value="{$.name}"/>`.
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

`<slot>` and `<each>` are structural elements, not components; `<slot>` has no `id`.

## 3. Canonical JSON

```ts
type Document = { weft: "0.1"; root: Node };

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

- Object keys in the order shown above; `props`, `on` and `slots` keys sorted lexicographically.
- Empty `props`, `on`, `slots`, `children` are omitted.
- Props whose value equals the catalog default are kept as written (no default elision).
- Markup serialization writes attributes as: `id`, then props sorted, then `on-*` sorted; two-space indentation; named slots after default-slot children, sorted by name.

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
  <each id="todo-each" in="{$.todos}" as="todo">
    <item id="todo-item"><text id="todo-title" value="{$todo.title}"/></item>
  </each>
</list>
```

- `in` MUST be a binding to an array. `as` names the loop variable, matching `[a-z][A-Za-z0-9]*`.
- Ids inside `<each>` are template ids: unique in the document, repeated per item at render time. A rendered instance is addressed as `id[index]`.
- `<each>` is transparent for parent/child rules: its children are validated as children of its parent.
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
3. **Semantics** — unique ids, parent/child rules, binding paths resolve to a loop variable in scope, token references exist in the supplied token set (when one is supplied), action names exist in the supplied action list (when one is supplied), `selected`/id references point at existing elements.

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
- **Extensions** are elements or attributes whose name starts with `x-<vendor>-`. An extension element MUST carry `role` (its ARIA fallback) and follows `content: "mixed"`. A reader that does not know it renders its children inside a container with that role.
- **Unknown, non-extension** elements or attributes come from a newer minor version or another catalog. In *lenient* mode (default for readers) they produce a `W4xx` warning; an unknown element is treated as an extension with role `group`, an unknown attribute is kept in the model and ignored. In *strict* mode (default for writers and CI) they are errors.
- A reader MUST NOT drop unknown content when it round-trips a document.
- A catalog has its own semver. Removing a component, a prop, an enum value, a slot, a state or an event is a major change; adding one is minor.
- A host advertises `{ weft, catalogs: [{ name, version }] }`; an agent writes only what the host advertises.

## 9. Mapping

- **To code:** a renderer maps each kind to a platform component. The reference renderer targets React and MUST produce an accessibility tree whose roles and names equal those the document declares. Text that is not part of a name appears in that tree as text runs; `stack`, `grid`, `text` and other role-`none` elements add no node of their own.
- **States:** a renderer maps a state to ARIA where an equivalent exists (`loading`, `busy`, `submitting` → `aria-busy`; field `invalid` or a non-empty `error` → `aria-invalid`) and exposes every state as `data-state` as well.
- **Trust:** a renderer never interprets document strings as markup or code. URLs in `link.href` and `image.src` are used only when they are `http`, `https`, `mailto` or relative; any other value is dropped.
- **From a running UI:** an accessibility snapshot maps back to Weft with losses (layout, tokens, bindings and actions are not recoverable). The lossy fields are listed by the importer.
