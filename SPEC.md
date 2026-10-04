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
    <button id="go" disabled="{!$.email}" submit="true" variant="primary">Sign in</button>
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
- Mixing text and bindings in one value (`"Hello {$.name}"`) is an error: a literal must not contain `{$`, `{!$` or `{token.` after its first character. Use `<text text="{$.name}"/>`.
- Token path grammar: segments of `[A-Za-z0-9_-]+` joined by `.`.
- Booleans are `true` / `false`. Numbers are JSON numbers.

### 2.2 Universal attributes

Allowed on every element:

| Attribute | Type | Meaning |
| --- | --- | --- |
| `id` | string, required | Document-unique, matches `[A-Za-z][A-Za-z0-9_-]*`. Stable across edits. |
| `label` | string | The element's accessible name (see below). |
| `hidden` | boolean | Not rendered and not exposed to assistive technology. |
| `state` | enum | One of the states the component declares (§5). |
| `role` | ARIA role | Required on extension elements; an error on catalog components. |
| `on-<event>` | action name | Binds an event the component declares to a named host action. |

`label` is the element's accessible name, and it MAY be a binding. Components that present a caption (`field`, `checkbox`, `switch`, `radio-group`, `select`) show it visibly as that caption, and `tab` shows it as the tab title; on `image` it is the text alternative; on containers (`screen`, `section`, `table`, `dialog`, `menu`, `tabs`, `form`, `list`) it names the region without being shown. On a component whose text is its content or `text` prop, `label` replaces that text as the accessible name and SHOULD be left out. Components with role `none` (`stack`, `grid`, `text`) add no node to the accessibility tree and so cannot carry a name; `label` on them has no effect.

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
- A named slot is a region whose placement the component decides, not document order: a form's `footer` sits below its fields, a dialog's `actions` in its button bar, a list's `empty` in place of its items. Where a `<slot>` stands among the default content therefore carries no meaning, which is why canonical markup writes named slots after the default content (§3).

### 4.3 Repetition

```xml
<list id="todos">
  <each id="todo-each" as="todo" in="{$.todos}">
    <item id="todo-item">
      <text id="todo-title" text="{$todo.title}"/>
    </item>
  </each>
</list>
```

- `in` MUST be a (non-negated) binding to an array. `as` names the loop variable, matching `[a-z][A-Za-z0-9]*`; it MUST NOT reuse the name of an enclosing loop variable.
- Ids inside `<each>` are template ids: unique in the document, repeated per item at render time. A rendered instance is addressed as `id[index]`.
- `<each>` holds one or more element children and no text; every child is repeated, in order, once per array item.
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
  min?: number;                            // for number: smallest allowed value, inclusive
  max?: number;                            // for number: largest allowed value, inclusive
  integer?: boolean;                       // for number: true = whole numbers only
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
| `grid` | `none` | nodes | `columns`* integer ≥ 1, `gap` token dimension | — | — | — |
| `section` | `region` | nodes | — (needs `label`) | `header` | — | — |
| `heading` | `heading` | text | `level`* integer 1–6 | — | — | — |
| `text` | `none` | text | `tone` enum `default`/`muted`/`success`/`warning`/`danger` | — | — | — |
| `image` | `img` | none | `src`*, `label`* | — | — | — |
| `link` | `link` | text | `href` | — | — | `press` |
| `button` | `button` | text | `variant` enum `primary`/`secondary`/`danger` (default `secondary`), `disabled` boolean, `submit` boolean literal (default `false`) | — | `idle`, `busy` | `press` |
| `form` | `form` | nodes | — | `footer` | `idle`, `submitting`, `invalid` | `submit` |
| `field` | `textbox` | none | `label`*, `type` enum `text`/`email`/`password`/`number`/`search`/`multiline` (default `text`), `value` writable, `placeholder`, `required` boolean, `disabled` boolean, `error` | — | `valid`, `invalid` | `change` |
| `checkbox` | `checkbox` | none | `label`*, `checked` boolean writable, `disabled` boolean | — | — | `change` |
| `switch` | `switch` | none | `label`*, `checked` boolean writable, `disabled` boolean | — | — | `change` |
| `radio-group` | `radiogroup` | nodes (`radio`, `each`) | `label`*, `value` writable | — | — | `change` |
| `radio` | `radio` | text | `value`*, `disabled` boolean | — | — | — |
| `select` | `combobox` | nodes (`option`, `each`) | `label`*, `value` writable, `disabled` boolean | — | — | `change` |
| `option` | `option` | text | `value`* | — | — | — |
| `list` | `list` | nodes (`item`, `each`) | `ordered` boolean | `empty` | `ready`, `loading`, `empty` | — |
| `item` | `listitem` | mixed | — | — | — | `press` |
| `table` | `table` | nodes (`column`, `row`, `each`) | — (needs `label`) | `empty` | `ready`, `loading`, `empty` | — |
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

- The `empty` slot of `list` and `table` is shown instead of the items or rows when there are none to show: every `<each>` in the component iterates an empty array (a binding that does not resolve to an array counts as empty) and it has no static items or rows, hidden ones included, or its `state` is `empty`. A table keeps its column headers.
- A `tab` element holds its panel content; a renderer emits `tab` and `tabpanel` from it.
- Every component whose content model is `text` or `mixed` also takes the prop `text` (string, bindable; declared in each such component's `props`, not repeated in the table). It takes its text either as content or as `text`, never both; `text` is how bound text is written, e.g. `<button id="b" text="{$.cta}"/>`. `value` is a data value (`field`, `radio`, `option`, `select`, `radio-group`), never displayed text.
- "needs `label`" and a starred `label`* in the Props column both mean the universal `label` attribute is required for that component (`requiresLabel` in the catalog); `label` is never declared in `props`.
- `(…)` after a content model lists the only kinds allowed as direct children. `each` is transparent (§4.3), so it is allowed wherever its own children would be, whether listed or not.
- A kind with a required parent context (`radio`, `option`, `item`, `column`, `row`, `cell`, `tab`, `menu-item`) declares it as `allowedParents`: `radio` in `radio-group`, `option` in `select`, `item` in `list`, `column` and `row` in `table`, `cell` in `row`, `tab` in `tabs`, `menu-item` in `menu`.
- A `button` with `submit="true"` submits its nearest enclosing `form`: pressing it fires that form's `submit` event, so it needs no `on-press`. If it has one, `press` fires first; each event fires once per press, also when the press comes from Enter in a field of the form. `submit` is a literal (`bindable: false`) because whether a button submits is structure, not data. A submit button outside a `form` is an error.
- `field` has role `textbox`; a renderer MAY refine it from `type` (`number` → `spinbutton`, `search` → `searchbox`) as ARIA requires.
- `column` is a direct child of `table` although ARIA places `columnheader` inside a `row`; the renderer emits the header row. On the web the header row sits in a header `rowgroup` and the rows in a body `rowgroup` (`<thead>`/`<tbody>`), and those groups are part of the declared tree.
- `tabs.selected` names the selected `tab` by `id`; when it is absent or names no tab, the first tab is selected. Only the selected tab's panel is exposed.
- `form` and `section` are landmarks only when they have a `label`: ARIA exposes `form` and `region` only with an accessible name, so an unlabelled `form` has no role of its own (its children are exposed directly).
- A `dialog` is shown only while `open` is true; an absent `open` means closed, like every boolean prop without a default.
- A `select` always has one option selected: the one whose `value` equals `select.value`, otherwise the first.
- `column.sort` states the current sort only: the host updates it in its data model in response to the column's `press` action, because documents carry no behaviour.
- `dialog.open` is `writable`. Writable bindings are how a renderer reports user input to the host's data model; the document itself carries no behaviour. When the user dismisses an open dialog (Escape on the web), the renderer writes `false` to the path `open` is bound to and fires `close`. Every other change, such as opening the dialog or closing it from a button's `press`, is the host updating that path in response to the action. A literal or negated `open` cannot be written, so dismissing such a dialog only fires `close`.
- "integer 1–6" is a `number` prop with `integer: true`, `min: 1`, `max: 6`; "integer ≥ 1" has `integer: true`, `min: 1`. Validation holds literals to the bounds; a renderer brings a bound value outside them into range, rounding it to a whole number where the prop is `integer` and then clamping it to `min` and `max`.

## 6. Validation

Validation has three layers, each reporting diagnostics rather than throwing:

1. **Syntax** — §2. The document is well-formed restricted XML.
2. **Schema** — the tree matches the catalog: known kinds, known and correctly typed props, numbers within their declared `min`, `max` and `integer`, required props present, declared slots, states and events.
3. **Semantics** — unique ids, parent/child rules, binding paths resolve to a loop variable in scope, token references exist in the supplied token set (when one is supplied), action names exist in the supplied action list (when one is supplied), binding paths are declared, with a type the attribute takes, in the supplied data schema (when one is supplied, §10.5), `selected`/id references point at existing elements (`tabs.selected` names a `tab`), `<screen>` only at the root, a `submit` button inside a `form`, and a component whose content model is `text` or `mixed` takes its text from content or from the `text` prop, not both.

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

Code ranges: `W1xx` syntax, `W2xx` schema, `W3xx` semantics, `W4xx` compatibility, `W5xx` patches, `W6xx` import (§9), `W7xx` projects (§10). A code, once published, never changes meaning.

`path` addresses the element from the root: one segment per element, `kind#id`, or `kind[index]` when the element has no valid id (the index counts the parent's list, text included). A named slot adds `slot[name]`, a text child `#text[index]`, an attribute `@name` (`@on-press` for events, `@weft` for the version). Syntax diagnostics name the open elements only. For JSON that does not have the shape of §3, the path is a JSON Pointer prefixed with `#`, e.g. `#/root/children/0/kind`. A patch diagnostic (§7) about the patch itself points into the patch list the same way, e.g. `#/patches/2/parent`.

### 6.2 Codes

`mode` severity is a warning in lenient mode and an error in strict mode (§8). `W602` and `W710` are warnings. Every other code is an error.

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
| W224 | Number below `min`, above `max`, or not whole where the prop is `integer`. |
| W301 | Duplicate id. |
| W302 | Child kind not in the parent's (or slot's) `allowedChildren`. |
| W303 | Parent kind not in the child's `allowedParents`. |
| W304 | Content breaks the content model: text where only elements go, elements where only text goes, anything in `none`. |
| W305 | Binding uses a loop variable that is not in scope. |
| W306 | Token not in the supplied token set. |
| W307 | Token `$type` differs from the prop's `tokenType`. |
| W308 | Action not in the supplied action list. |
| W309 | Id reference points at no suitable element. |
| W310 | Text given twice: a `text` or `mixed` component has both content and the `text` prop. |
| W311 | Loop variable shadows an enclosing one. |
| W312 | `screen` below the root. |
| W313 | `button` with `submit="true"` outside a `form`. |
| W314 | `<each>` without an element child. |
| W315 | Binding path not declared in the supplied data schema (§10.5). |
| W316 | Data type at the binding path is not one the attribute takes (§10.5). |
| W401 | Unknown element (mode). |
| W402 | Unknown attribute (mode). |
| W403 | Newer minor version of the format (mode). |
| W404 | Unsupported major version. |
| W501 | The patch list is not an array, or a patch does not have the shape of §7. |
| W502 | A patch names an id (`id` or `parent`) that no element has. |
| W503 | `set` names a prop a patch cannot change: `id`, the root's `weft`, a name that breaks the name grammar, or an `on-<event>` value that is not an action name. |
| W504 | `slot` is not declared by the parent. |
| W505 | `index` is greater than the length of the target list. |
| W506 | `move` into the moved element itself or one of its descendants. |
| W507 | `remove` or `move` of the root element. |
| W508 | Inserted `markup` is empty, or holds text or `<slot>` beside its elements. |
| W509 | Inserted `markup` uses an id that the document already has. |
| W601 | Imported input cannot be read: a snapshot that is neither Playwright aria snapshot YAML nor an accessibility tree, HTML that is not a string, or a catalog the importer cannot map with. |
| W602 | Imported input exceeds an import limit (length, element count or nesting depth); the rest is not imported. |
| W701 | Project file is not JSON or not an object, or a member has the wrong type (§10.2). |
| W702 | Unknown member in the project file (mode). |
| W703 | File name in the project file is absolute, leaves the project directory or is malformed. |
| W704 | File named by the project cannot be read or is not JSON. |
| W705 | Problem in the project's token files (§10.3). |
| W706 | Catalog extension, or one of its entries, is not a valid catalog or definition (§10.4). |
| W707 | Catalog extension narrows or changes the core catalog (§10.4). |
| W708 | Action name in the project file breaks the action grammar. |
| W709 | Data schema is malformed (§10.5). |
| W710 | Data schema uses a keyword Weft does not support; that part accepts any data (§10.5). |

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

`applyPatches(document, patches, { catalog, mode?, tokens?, actions? })` takes the patch list as untrusted input (any JSON value), never throws and never changes `document`. It returns `{ document?, diagnostics }`.

- **Atomic.** The patches apply in order to a copy, so a later patch sees the effects of earlier ones. The result is canonicalized and validated as in §6 with the given options. If any patch fails or the result has errors, nothing is applied: `document` is absent and `diagnostics` explain the first failing patch (or the validation errors). Otherwise `document` is the result and `diagnostics` holds its warnings. A patch list does not repair a document that was already invalid: its errors are reported.
- **Shape.** A patch has exactly the members shown and nothing else; `index` is a non-negative integer. A list that is not an array, or a patch that breaks the shape, gives `W501`, one diagnostic per problem, each pointing at `#/patches/<n>/…`. An empty list is valid and changes nothing.
- **Addressing.** `id`, `parent` name elements by id, including `<each>`. A name that no element has gives `W502` with the nearest id as the hint. `<slot>` has no id: it is addressed by `parent` and `slot`.
- **`set`.** `value` is a Value of §3 and is stored as given, so it is typed JSON, not markup text: `7`, `true`, `{ "bind": "$.name" }`, `{ "token": "space.md" }`. Strings are always literals, so a string that starts with `{` needs no escape. `null` removes the prop; removing an absent prop does nothing. The one exception to storing as given is `text` on an element that holds its text as content (**Text** below). A prop name starting with `on-` binds or unbinds the event after it: the value is an action name, or `null` to remove the binding. `id` cannot be set (ids are stable; to rename, remove the element and insert it again), nor the root's `weft`. The prop name must follow the name grammar (§2). Whether the prop, event or value is allowed is left to validation (`W203`, `W204`, `W206`, …).
- **`insert`.** `markup` is one or more sibling elements, parsed like a document (§2) but without a `<screen>` root; literals are typed by the catalog. It must hold elements only (`W508`), with ids that no element of the document has (`W509`; syntax errors keep their `W1xx` codes and point into `#/patches/<n>/markup`). They go into the default slot of `parent`, or into the slot named by `slot` (a name the parent's component declares, `W504`; `<each>` declares none, extension and unknown elements accept any name). `index` counts the entries of that list, text included, and defaults to the end; it must not exceed the length (`W505`).
- **`remove`.** Deletes the element and everything inside it. The root cannot be removed (`W507`).
- **`move`.** Takes the element out and puts it into `parent` as `insert` would, keeping its id and content. `index` counts the target list after the element has left it, so the final order is the one the index names, also when the target is the list it came from. The root cannot move (`W507`); an element cannot move into itself or its descendants (`W506`).
- **Text.** Text is changed with `set` and `prop: "text"`, wherever the element keeps it (§5.1: content or the `text` prop, never both). When a `text` or `mixed` component holds its text as content and nothing else, the patch writes that content: a string replaces it and stays content, any other value replaces it with the `text` prop, and `null` removes it. Otherwise `text` is set like any other prop, so text already in the prop stays there, and a `mixed` component whose content holds elements keeps them and gets `W310` if it is given `text` as well. Text that shares a content list with elements (an `item` holding text beside a button) is not addressable by a patch: `remove` the element that holds it and `insert` it again with the same id.

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
- **To JSX:** a document compiles to one self-contained React function component `({ data, actions, onChange })` with the accessible structure of the reference renderer. A binding becomes a read of `data` or of a loop variable that reads own properties only, `not` becomes `!`, `<each>` becomes `.map` with the item index as key and `id[index]` instance ids, an event calls `actions[name]` with `{ id, action, item }` when the host supplied that action, a writable prop becomes a controlled input that calls `onChange(path, value)` with the absolute data path, a token reference becomes `var(--weft-<path with . as ->)`, and named slots go where §5.1 places them. Document strings appear in the output only as escaped string literals or as JSX text made of inert characters, so a document cannot inject code.
- **From a running UI:** an accessibility snapshot (Playwright aria snapshot YAML or the same tree as objects) or rendered HTML maps back to Weft with losses. The importer maps each role to the first catalog kind with that role, except for the refinements and inversions below; a role no kind has becomes the extension `x-aria-<role>` with `role` set. It returns `{ document, losses, diagnostics }`, where each loss is `{ kind, path, note }` and the document validates in lenient mode without errors.
  - Refinements: `spinbutton` and `searchbox` are a `field` with `type` `number` or `search`, `paragraph` is `text`, an exposed `dialog` has `open` true. `generic`, `none`, `presentation` and `rowgroup` add no element; their children take their place.
  - Inversions: a `tablist` and the `tabpanel`s after it become one `tabs` whose `tab`s hold their panels, and the selected tab becomes `tabs.selected`; a header row of `columnheader`s becomes the table's `column`s; a checked `radio` or selected `option` becomes the `value` of its group or select.
  - HTML is read with the implicit roles of HTML-AAM (a `form` and a `section` always count as `form` and `region`) and a simplified accessible name: `aria-labelledby`, `aria-label`, an image's `alt`, a control's `<label>`, a button input's `value`, a table's `<caption>`, then `title`, else the text content. Elements carrying `data-weft-id` keep it as their id; a stack or grid is recognised from its `display` style.
  - Ids are generated from the kind and the name (`button-save`, `list-1`) when the input has none, and `id[index]` instance ids become `id-index`.
  - Input is untrusted: it is never evaluated, a literal that would read as a binding or token reference has its brace replaced, and length, element count and depth are bounded (`W602`).

| Loss kind | Accessibility snapshot | HTML |
| --- | --- | --- |
| `ids` | Always: the tree has no ids; all are generated. | Elements without a valid, unique `data-weft-id` get a generated id. |
| `bindings`, `actions` | Always: values are the ones shown; handlers are not in the tree. | Always, likewise. |
| `tokens` | Always. | A token-backed `gap` is noted; token values are not recovered. |
| `layout` | Always: `stack` and `grid` are not in the tree. | Only a `gap` that is not a token variable. |
| `repetition` | Always: repeated content is static siblings, not `<each>`. | Each `id[index]` instance becomes a static sibling. |
| `slots` | Always; content a kind does not take by default goes to the first slot that takes it. | Likewise. |
| `hidden` | Always: hidden elements, closed dialogs and unselected tab panels are absent. | Hidden elements and closed dialogs; all tab panels are kept. |
| `props` | Always: props and states without an ARIA equivalent (variant, tone, placeholder, required, sort, modal, `data-state`); also invalid values. | Invalid values only. |
| `values` | A required prop missing from the input is filled with a stand-in. | Likewise. |
| `names` | A required `label` missing from the input is set to `""`. | Likewise. |
| `kinds` | A role without a kind becomes `x-aria-<role>`. | Likewise. |
| `text` | Text with no place in the content model is dropped; a reference-like brace is replaced. | Likewise. |
| `structure` | No single `main` landmark: a `screen` root is added. | Likewise. |

## 10. Projects

Screens that belong together share their resources through a project file named `weft.json`. A screen names neither its project nor its resources.

### 10.1 Finding the project

- A tool that reads a screen from a file looks for `weft.json` in the screen's directory, then in each parent directory up to the root of the file system. The first one found is the project; without one, the tool behaves as before (no project).
- An explicit argument wins: a project file given to the tool replaces the search, and a catalog or token set given to the tool replaces the project's.
- A tool that reads no files (the MCP server) takes the project's content instead: the same members, with each file name replaced by the file's JSON content.

### 10.2 The project file

```json
{
  "tokens": ["tokens/base.tokens.json", "tokens/brand.tokens.json"],
  "catalog": "catalog.json",
  "actions": ["cart.add", "nav.home"],
  "data": "data.schema.json"
}
```

| Member | Type | Meaning |
| --- | --- | --- |
| `tokens` | array of file names, at most 64 | DTCG token files, in layer order (§10.3). More than 64 is `W701`. |
| `catalog` | file name | An extension of the core catalog (§10.4). |
| `actions` | array of action names | The host's actions: `on-*` values are checked against them (`W308`). |
| `data` | file name | A JSON Schema of the host data model (§10.5). |
| `$schema` | string | Ignored; for editors. |

- Every member is optional. Without `catalog` the project's catalog is the core catalog.
- A file name is relative to the directory of the project file, uses `/` as separator and stays inside that directory: it is non-empty and has no empty or `..` segment, no leading `/`, no `\`, no `:` and no NUL. Any other name is `W703` and the file is not read.
- Loading never stops at a problem. Each problem is a diagnostic and the rest of the project still applies: a project file that is not JSON or not an object is `W701` and the project is empty; a member of the wrong type is `W701` and is ignored, and so is an entry of `tokens` or `actions` of the wrong type; an unknown member is `W702`; an action name that breaks the action grammar (§2.2) is `W708` and is left out; a file that cannot be read or is not JSON is `W704` and is left out.
- Project diagnostics point into the project file with a JSON Pointer prefixed with `#`, e.g. `#/tokens/1`; for project content passed as a tool argument the pointer starts at that argument (`#/project/tokens/1`). A pointer continues into a named file as if its content stood in the project file: `#/catalog/components/rating`, `#/data/properties/user/type`. A problem of the merged token tree points at `#/tokens`. Diagnostics of a screen keep the paths of §6.1.

### 10.3 Token layers

- Each file is a DTCG 2025.10 token tree. The files merge in order into one tree, which then loads as one token file. Groups merge member by member; a token (an object with `$value`) replaces whatever the earlier files have at its path, as a whole; any other clash is won by the later file. So a later file overrides a token by declaring it again, and a `$type` an earlier file sets on a group still applies to tokens a later file adds to that group.
- Aliases resolve after the merge, as the DTCG resolver module orders its sets: an alias in the base file to a token the brand file overrides reads the brand value.
- A file that is not a JSON object, and every problem of the merged tree (a bad name, a token without a type, an alias to nothing, an alias cycle), is `W705`. The token in question is left out.

### 10.4 Catalog extension

The `catalog` file is a catalog (§5): `weft`, `name`, `version` and `components`. The project's catalog is the core catalog with the extension's components merged in, under the extension's `name` and `version`.

- A kind the core catalog does not have is a new component and needs a whole definition (`description`, `role`, `content`, …). Its name follows the name grammar, is not `each` or `slot`, and does not start with `x-`, because `x-` elements are opaque to every catalog (§8).
- A kind the core catalog has is extended. The entry may leave out `description`, `role` and `content` to keep the core's. `props` and `slots` merge by name, an entry replacing the core definition of that name (to add a variant, restate the prop with the longer `values` list). `states`, `events`, `allowedChildren` and `allowedParents` are joined: the core's values, then the new ones. Every other field replaces the core's.
- An extension may only widen the core catalog. The merged catalog is compared with the core catalog by the rules of §8: a kind whose merged definition makes a change those rules call major (a changed role or type, a new required prop, a narrowed content model, …) is `W707`, and that kind keeps its core definition.
- An extension that is not a catalog is `W706` and is ignored; an entry that is not a valid definition, or names a kind that breaks the rules above, is `W706` and only that entry is ignored.
- `null` is not a value in a catalog: a member written `null` makes its entry invalid.

### 10.5 Data schema

The `data` file is a JSON Schema (2020-12) of the host data model. Validation checks every binding against it: the path must be declared, and the data there must have a type the attribute takes.

- **Subset.** Weft reads `type` (a type name or a list of them), `properties`, `additionalProperties` and `items` (one schema), and the boolean schemas `true` (any data) and `false` (no data). Keywords that only annotate or constrain values (`$schema`, `$id`, `$comment`, `$defs`, `title`, `description`, `default`, `examples`, `required`, `enum`, `const`, `format`, `minimum`, …) are ignored. Keywords that combine or reference schemas (`$ref`, `$dynamicRef`, `allOf`, `anyOf`, `oneOf`, `not`, `if`, `then`, `else`, `dependentSchemas`, `prefixItems`, `contains`, `patternProperties`, `propertyNames`, `unevaluatedItems`, `unevaluatedProperties`) are not supported: `W710`, and the schema they sit in accepts any data. A schema that is not an object or a boolean, a `type` that names no JSON Schema type, `properties` that is not an object, or nesting deeper than 256 schemas, is `W709`, and that schema accepts any data.
- **Closed objects.** One rule differs from JSON Schema: a schema with `properties` and without `additionalProperties` declares every property the object has, as if `additionalProperties` were `false`. An object with open keys (a map) says so with `"additionalProperties": true` or a schema for its values. A schema with neither keyword declares any name.
- **Paths.** A path from `$.` starts at the root schema; a path from `$item` starts at the `items` schema of the `in` of the `<each>` that names `item`. A name segment steps into `properties`, then `additionalProperties`; an index segment steps into `items` when the schema may be an array. A step the schema does not declare, into `false`, or into a value that is neither an object nor an array, is `W315`, with the nearest declared name as the hint. A schema without `type` may be anything, so every step from it is declared.
- **Types.** The schema at the end of a plain binding must allow a type the attribute takes: a `string` prop (`text`, `label` and `state` included) takes `string`, `number` and `integer`, because text shows numbers; a `number` prop takes `number` and `integer`; a `boolean` prop (and `hidden`) takes `boolean`; an `enum` or `token` prop takes `string`; the `in` of `<each>` takes `array`. Otherwise it is `W316`. A schema without `type` allows every type. A negated binding tests whether a value is empty and so takes any type; an attribute the catalog does not declare is not type-checked.
