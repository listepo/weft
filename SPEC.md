# Weft 0.2 — specification (draft)

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
<screen id="login" label="Sign in" weft="0.2">
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
- A binding read as a boolean is true unless its value is falsy: `false`, `0`, an empty string, `null` or a missing path. An empty array or object is true. A negated binding is the opposite, so `{!$.email}` is true while `$.email` is empty or missing.
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
| `rotate-x`, `rotate-y`, `rotate-z` | number, -360 to 360 | A 3D tilt in degrees: about the horizontal axis, the vertical axis, and in the screen plane. Literal only. |
| `perspective` | number, at least 1 | How far the viewer is from the element, in px; the nearer, the stronger the depth. Absent: no perspective, a flat projection. Literal only. |
| `grow` | boolean | The element takes a share of its parent `stack`'s free space along the stack's direction. Absent means `false`. Literal only; only on a child of a `stack` (`W318`). |
| `on-<event>` | action name | Binds an event the component declares to a named host action. |

`label` is the element's accessible name, and it MAY be a binding. Components that present a caption (`field`, `checkbox`, `switch`, `radio-group`, `select`, `combobox`, `slider`, `stepper`, `date-picker`, `color-picker`, `segmented-control`) show it visibly as that caption, and `tab` shows it as the tab title; on `image` it is the text alternative; on containers (`screen`, `section`, `table`, `dialog`, `menu`, `tabs`, `form`, `list`) it names the region without being shown. On a component whose text is its content or `text` prop, `label` replaces that text as the accessible name and SHOULD be left out. Components with role `none` (`stack`, `grid`, `text`) add no node to the accessibility tree and so cannot carry a name; `label` on them has no effect.

The 3D tilt is the transform `perspective(p) rotateX(x) rotateY(y) rotateZ(z)` (CSS Transforms, so `rotate-z` acts on the element first), about the element's centre; an absent value is 0, and an element with none of the four attributes is not transformed. It changes how the element is drawn, not where it sits in the layout or in the accessibility tree. A target that cannot tilt in 3D draws the nearest 2D projection and keeps the values: `rotate-z` as the rotation, a tilt about x or y as a scale by its cosine (mirrored past 90 degrees), `perspective` ignored. SwiftUI has no length for the viewer's distance, so it draws `perspective` as its own relative value; a card tilted with a perspective looks alike on the web and in SwiftUI, not pixel for pixel.

`grow="true"` makes the element take an equal share of its stack's free space (§5.1, `stack.justify`): its width in a row, its height in a column. Elements without it keep the size of their content; several growing children share the free space equally, with no ratios, and leave none for `justify`. Which child grows is structure, not data, so a binding is `W217`. `grow="true"` on an element whose parent is not a `stack` (the root, a child of a `grid` or any other kind, the content of a named slot) is `W318` in both modes. `<each>` is transparent (§4.3), so an element it repeats inside a stack is that stack's child; an unknown or extension parent is skipped, as for the parent/child rules (§8).

Action names match `[a-z][A-Za-z0-9]*(\.[a-z][A-Za-z0-9]*)*`. Actions take no arguments in the document; the host receives the action name, the element id, and for elements inside `<each>` the current loop item path.

`<slot>` and `<each>` are structural elements, not components, and take none of the universal attributes except that `<each>` has an `id`:

- `<slot name="…">` takes exactly one attribute, `name`, which follows the name grammar. A slot holds elements only, no text.
- `<each id="…" in="{…}" as="…">` takes `id`, `in` and `as` (§4.3).

### 2.3 Context

A screen MAY carry a `<context>` block: notes that people and agents left for whoever works on the screen next. It holds why the screen is the way it is (its purpose, decisions, rules it must keep, open questions, work left, where its content came from), not what it shows. Context is data for a reader: no tool follows, executes, fetches or renders anything in it, and end users never see it. The reasons for this design are in `docs/context-design.md`.

```xml
<screen id="login" label="Sign in" weft="0.2">
  <context>
    <entry id="why" by="human" kind="intent" name="Ivan">Returning users sign in with email and password.</entry>
    <entry id="submit-disabled" by="agent" for="go" kind="decision" name="claude-opus-5-5">Disabled until an email is typed, so auth.submit never gets an empty request.</entry>
    <entry id="reset-where" by="agent" for="reset" kind="question" name="claude-opus-5-5" status="open">Should reset open a dialog or a screen of its own?</entry>
  </context>
  <form id="f1" state="idle" on-submit="auth.submit">
    <field id="email" label="Email" required="true" type="email" value="{$.email}"/>
    <button id="go" disabled="{!$.email}" submit="true" variant="primary">Sign in</button>
    <slot name="footer">
      <link id="reset" on-press="nav.reset">Forgot password?</link>
    </slot>
  </form>
</screen>
```

- `<context>` is a structural element, like `<slot>` and `<each>`: not a component, no attributes, no `id`. It is a direct child of the root, at most once, and holds `<entry>` elements and whitespace only. Anywhere else (deeper in the tree, inside `<each>` or a `<slot>`, in patch `insert` markup) it is `W120`, as are a second block, an attribute on it and other content in it.
- The parser accepts `<context>` anywhere among the root's children and lifts it out of the content, so it never counts in content models, `index` positions or text joining. The serializer always writes it first.
- `<entry>` takes exactly the attributes `id`, `kind`, `by`, `name`, and where they apply `for` and `status`; extension attributes are not allowed on it. Its content is its text: one run of text, whitespace-normalized like all text (§2), no elements. An `<entry>` outside `<context>`, an element inside it, another attribute, or `kind`, `by` or `name` missing is `W121`.
- Entry attribute values are plain text, like `id` and `on-*` (§2.1): never references, so `{` needs no escape. The text is content, so a note may say `{$.email}` without `W116` or `W213`; like all text it escapes `&` and `<`.

| Field | Required | Value |
| --- | --- | --- |
| `id` | yes | The id grammar of §2.2, document-unique across elements and entries (`W202`, `W212`, `W301`). |
| `kind` | yes | `intent`, `decision`, `constraint`, `question`, `todo` or `source` (`W203`). |
| `by` | yes | `human` or `agent` (`W203`). |
| `name` | yes | Who wrote it: a person's name or handle, or an agent's model id. 1–64 characters: letters and digits of any script, space, `.`, `_`, `@`, `/`, `+`, `-`, starting with a letter or digit (`W229`). |
| `for` | no | The id of the element the entry is about: any element but the root, including `<each>`, extension elements and template ids inside `<each>`. Absent means the screen (`W309`). |
| `status` | on `question` and `todo` only | `open` or `resolved`: required on those two kinds and not allowed on the others (`W227`, `W203`). |
| text | yes | 1–500 characters after normalization (`W229` when empty, `W228` when longer). |

| Kind | Meaning |
| --- | --- |
| `intent` | What the screen or element is for: the goal it serves. |
| `decision` | A choice that was made, and why. Keep it unless asked to revisit it. |
| `constraint` | A rule the design must keep: legal, accessibility, product or technical. |
| `question` | Something not decided yet. Open until answered. |
| `todo` | Work left to do. Open until done. |
| `source` | Where content or design came from: a guide, a ticket, research, a design file. A URL in the text is text; no tool fetches it. |

Limits, because context is the one part of a document written as free prose for models to read: at most 100 entries, 500 characters per text and 16,000 characters of text in all (`W228`, a `mode` code: an error for writers, a warning for readers, since a reader that never shows context has no reason to refuse a screen whose notes are too long). Characters are UTF-16 code units, as diagnostic columns are. `by` and `name` are claims nobody verifies; an entry has no date.

## 3. Canonical JSON

```ts
type Document = {
  weft: "0.3";                       // the root element's `weft` attribute
  version?: string;                  // the root's `version` attribute: MAJOR.MINOR.PATCH
  context?: Entry[];                 // the `<context>` block (§2.3), in written order
  fragments?: Record<string, Node>; // inline fragments (§10.7), by name
  root: Node;
};

type Entry = {
  id: string;
  kind: "intent" | "decision" | "constraint" | "question" | "todo" | "source";
  by: "human" | "agent";
  name: string;
  for?: string;                      // element id; absent = the screen
  status?: "open" | "resolved";      // question and todo only
  text: string;
};

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
- Empty `props`, `on`, `slots`, `children` and empty slot lists are omitted, and so are an empty `context`, an empty `fragments` and the absent members of an entry.
- `context` keeps the order the entries were written in; it is not sorted. Entry text is whitespace-normalized like text children. The JSON shape check reads every entry member as a string, so a wrong value is a validation diagnostic (`W203`, `W227`) rather than `W200`.
- `fragments` is a map from an inline fragment's name to its node (kind `fragment`, `label` and `version` in `props`, parameters and body in `children`). Keys sort like every other object key. Markup writes the fragments after the `<context>` block and before the body, sorted by name.
- Text children are whitespace-normalized as in §2, and adjacent text children are joined with one space. `-0` is written `0`.
- Props whose value equals the catalog default are kept as written (no default elision).
- The `weft` attribute of the root element is `Document.weft` and never appears in the root's `props`. The optional `version` attribute of the root `<screen>` or of the root `<fragment>` of a fragment file is `Document.version` and never appears in the root's `props`. It is a literal `MAJOR.MINOR.PATCH` with no leading zeros, no pre-release and no build metadata, the form a catalog's `version` already has (§5). A binding, a token reference or anything else is `W230`. In markup it sorts with the other attributes, as `weft` does.
- The JSON text is indented by two spaces and ends with a newline.
- Markup serialization writes attributes as: `id`, then props sorted (the root's `weft` sorts with them), then `on-*` sorted; two-space indentation; named slots after default-slot children, sorted by name. The root writes the `<context>` block before its children, one entry per line with `id` first and the other attributes sorted, the text as the entry's content; a document without context is written as if the block did not exist. Inline fragments are written next, each as `<fragment name="…">` with `name` sorted among its attributes, then the body. An element with no content is self-closing; an element whose only content is one text child is written on one line; otherwise every child goes on its own line. The text ends with a newline.
- Escaping: in attribute values `&`, `<`, `"`, tab, LF and CR are written as references; in text `&`, `<` and `>` are. No other references are written.

Literal typing needs the catalog: `level="2"` is the number `2` only because `heading.level` is declared a number. For extension elements and unknown attributes, literals stay strings.

### 3.1 JSON Schema of the canonical form

A catalog (§5) determines a JSON Schema of the canonical JSON documents it admits, so that a writer whose decoder takes a schema (a structured-output mode) is held to the catalog while it writes. The schema is generated from the catalog and is a projection of validation (§6), not a second definition: the validator always has the last word. The dialect is JSON Schema 2020-12 (`$schema` is `https://json-schema.org/draft/2020-12/schema`).

| Catalog construct | Schema |
| --- | --- |
| Document | A closed object (`additionalProperties: false`) with `weft` (`enum` of the versions this reader reads without a diagnostic, `0.1`, `0.2` and `0.3`), optional `version` (a literal `MAJOR.MINOR.PATCH`) and `root`. `weft` and `root` are required. |
| `root` kind | `root` is a `$ref` to the kind marked `root` (`anyOf` when several are); with none, any kind. That kind is admitted nowhere else, and its `weft` prop is not in its `props` (it is `Document.weft`). |
| Kind | One `$defs` entry per kind, named after it: a closed object with `kind` (`const`), `id` (the id grammar as `pattern`), `props`, `on`, `slots` and `children`, in that order. `kind` and `id` are required, `props` when a prop or `label` is. `on`, `slots` and `children` are present only when the kind declares events, slots, or a content model other than `none`. |
| Props | A closed object: the declared props, then the universal `label`, `hidden`, `state` (only when the kind declares `states`) and the tilt attributes (§2.2), sorted as canonical JSON sorts keys. `required` lists the required props, and `label` when `requiresLabel` is set. `role` is not admitted on a catalog component. |
| Prop value | `anyOf` over the forms the prop takes. Literal: `string`; `number`, or `integer` when `integer` is set, with `minimum` and `maximum` from `min` and `max`; `boolean`; `enum` of `values` (of `states` for `state`). A token prop takes no literal but `{ token }`, and no other prop takes `{ token }`. Unless `bindable` is false, `{ bind }`; `{ bind, not: true }` only on a boolean prop that is not `writable`. Binding paths and token paths carry their grammars (§2.1) as `pattern`. `description` is the prop's. |
| `states` | The values of `state`. |
| `events` | `on`: a closed object with one property per declared event, the action grammar as `pattern`. |
| `slots` | `slots`: a closed object with one property per declared slot, each an array of the elements that slot admits (no text); `required` lists the required slots. |
| `content` | `children`: an array of strings for `text`, of the admitted elements for `nodes`, of either for `mixed`; absent for `none`. |
| Admitted elements | A list of parent P (its children, or its slot S) admits the kinds in P's `allowedChildren` (S's for a slot), or every kind when that list is absent, keeping only kinds whose `allowedParents` is absent or names P and leaving out the `root` kind. The kinds without `allowedParents` are collected once, in `$defs/Node`. |
| `each` | Transparent (§4.3): every list that admits `each` (P's `allowedChildren` is absent or names it) has its own entry, `each:P` or `each:P:S`, with `kind`, `id`, `props` holding exactly `in` (a binding, not negated) and `as` (the loop variable grammar), both required, and `children`: at least one element, from the same admitted elements as that list, which includes the entry itself. |
| `description` | The component's, on its entry. |

Helper entries (`Id`, `Binding`, `NegatableBinding`, `Token`, `Action`, `Node`) start with a capital letter, and the `each` entries and the shared entries of the universal attributes (`universal:label`, `universal:hidden`, …) hold a `:`, so none can clash with a kind name.

The schema leaves out what it cannot express, and the validator still checks it: unique ids (`W301`), loop variables in scope and not shadowed (`W305`, `W311`), `references` (`W309`), text given both as content and as `text` (`W310`), a submit button outside a `form` (`W313`), the asset paths of a `model` (`W317`), a reference inside a literal (`W213`) and characters XML cannot carry (`W221`), whose patterns would need look-around or ranges beyond the Basic Multilingual Plane that structured-output modes do not reliably support. It describes the catalog only, so the project's tokens, actions and data schema (`W306`–`W308`, `W315`, `W316`) are not narrowed into it.

It never rejects a canonical document that strict validation (§8) accepts against the same catalog, with five exceptions: it admits no extension (`x-`) elements or attributes, which a constrained writer has no use for; it admits no `state` on a kind that declares no states, where validation lets a bound one through; it admits no `context` (§2.3), which describes no part of the screen a writer builds; it admits no `<use>` (§10.7), whose parameters belong to the project's fragments rather than to the catalog; and it admits no inline `fragments`, for the same reason. Equal catalogs give byte-equal schemas: `$defs` are sorted by name, the members of `props`, `on` and `slots` as canonical JSON sorts keys, and node members follow canonical order, so a decoder that writes members in schema order writes canonical key order.

`weft schema` prints the schema of the project's catalog (or of `--catalog`) indented, or writes it as `document.schema.json` (§10.6); `documentSchema` of `@weft/core/document-schema` and the MCP tool `weft_schema` return the same schema as compact JSON text, byte for byte what the generator writes. The schema of §3 that `documentJsonSchema()` of `@weft/core` returns is a different one: it knows no catalog and admits any kind and any prop.

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
  prefix?: string;                         // owns the kinds named `<prefix>-…`; absent = a project catalog
  requires?: Record<string, string>;       // catalog name → the version it was written against
  components: Record<string, ComponentDef>;
  fragments?: Record<string, Document | string>; // merged: every fragment by name (§10.7); a library file: name → file (§10.4)
};

type ComponentDef = {
  description: string;                     // one sentence, written for a model
  role: string;                            // ARIA role, or "none"
  content: "none" | "text" | "nodes" | "mixed";
  allowedChildren?: string[];              // kinds; absent = any
  allowedParents?: string[];               // kinds; absent = any
  requiresLabel?: boolean;                 // the universal `label` attribute is mandatory
  root?: boolean;                          // the document root must be this kind, and it may stand nowhere else
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
  references?: string;                     // for string: a literal value is the id of an element of this kind
};

type SlotDef = { description: string; allowedChildren?: string[]; required?: boolean };
```

- `name` identifies the catalog in diagnostics, in the advertisement (§8) and in `requires`. Two catalogs of one project never share a name (§10.4). A catalog published as a package uses the package name.
- `prefix` makes the catalog a *library* that owns every kind named `<prefix>-…` (`acme` owns `acme-button`). It is one name segment, `[a-z][a-z0-9]*`, and is not `x` (opaque extensions, §8), `weft` (catalogs of this specification), a core kind or the first segment of one (`button`, `date`, `menu`, …). The core catalog has no prefix and never adds a kind whose first segment is a known library prefix. A catalog without `prefix` is a project's own catalog (§10.4).
- `fragments` in a library file lists the library's fragments (§10.4), each name mapped to a fragment file. A project's merged catalog maps every fragment it knows, the libraries' and the project's, to its document (§10.7). A project catalog file has none.
- `requires` lists the catalogs, `weft-core` included, and the versions the catalog was written against. A version is `MAJOR.MINOR.PATCH` (semver) and is read by Cargo's compatibility rule: `"1.2.0"` admits `>=1.2.0, <2.0.0`, and `"0.1.0"` admits `>=0.1.0, <0.2.0`; the left-most non-zero component must match. There is no range syntax.

### 5.1 Core catalog `weft-core` 0.2

Props are strings unless a type is given. `*` marks required props.

| Kind | Role | Content | Props | Slots | States | Events |
| --- | --- | --- | --- | --- | --- | --- |
| `screen` | `main` | nodes | `weft`* | — | `ready`, `loading`, `error` | — |
| `stack` | `none` | nodes | `direction` enum `column`/`row` (default `column`), `gap` token dimension, `align` enum `start`/`center`/`end`/`stretch`, `justify` enum `start`/`center`/`end`/`space-between` (default `start`), `wrap` boolean, `padding` token dimension, `max-width` token dimension, `material` token material | — | — | — |
| `grid` | `none` | nodes | `columns`* integer ≥ 1, `gap` token dimension, `min-column-width` token dimension, `padding` token dimension, `max-width` token dimension, `material` token material | — | — | — |
| `section` | `region` | nodes | — (needs `label`) | `header` | — | — |
| `heading` | `heading` | text | `level`* integer 1–6 | — | — | — |
| `text` | `none` | text | `tone` enum `default`/`muted`/`success`/`warning`/`danger` | — | — | — |
| `image` | `img` | none | `src`*, `label`* | — | — | — |
| `model` | `img` | none | `src`*, `usdz`, `fallback`*, `label`* (the three paths are literals) | — | — | — |
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
| `combobox` | `combobox` | nodes (`option`, `each`) | `label`*, `value` writable, `placeholder`, `disabled` boolean | — | — | `change` |
| `slider` | `slider` | none | `label`*, `value` number writable, `min` number (default 0), `max` number (default 100), `step` number (default 1), `disabled` boolean | — | — | `change` |
| `stepper` | `spinbutton` | none | `label`*, `value` number writable, `min` number, `max` number, `step` number (default 1), `disabled` boolean | — | — | `change` |
| `date-picker` | `textbox` | none | `label`*, `type` enum `date`/`time`/`datetime` (default `date`), `value` writable, `min`, `max`, `disabled` boolean | — | — | `change` |
| `color-picker` | `textbox` | none | `label`*, `value` writable, `disabled` boolean | — | — | `change` |
| `segmented-control` | `radiogroup` | nodes (`segment`, `each`) | `label`*, `value` writable | — | — | `change` |
| `segment` | `radio` | text | `value`*, `disabled` boolean | — | — | — |
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
- `stack.align` is the alignment across the stack's direction, and its default depends on it: a `row` without `align` centres its children on the cross axis (SwiftUI's `HStack`), so a button next to a field keeps its own height, while a `column` without `align` keeps its host's own layout (it has no default, and SPEC does not define one). An explicit `align` always wins, and `stretch` has no SwiftUI equivalent. A renderer writes the default out, so an importer does not read a row's `center` back as `align`.
- `stack.justify` shares the stack's free space along its direction: its length minus its children's lengths and the gaps. `start` leaves it after the last child, `end` before the first, `center` half on each side, and `space-between` equally between the children, none at the ends (a single child sits at the start). `gap` stays the least space between two children. A row takes the full width its parent gives it, so it has free space whenever its children are narrower; a column is as tall as its children unless a row with `align="stretch"` stretches it, so `justify` on a column matters only there. A child with `grow` (§2.2) leaves no free space, and `justify` then has no effect. The default `start` is every target's initial value: a renderer writes nothing for it, and an importer does not write `justify="start"` back for a stack that does not say it.
- `padding` on a `stack` or `grid` is the inner spacing on all four sides, inside its `material` surface when it has one. Other kinds keep the padding their renderer gives them; a padded group is a stack.
- `max-width` makes a `stack` or `grid` as wide as its parent lets it be, but never wider than the token; where it sits inside a wider parent is the parent's cross-axis `align`. It does not make the element grow along a row; that is `grow`.
- `grid.min-column-width` lets a grid reflow by its own width, not the viewport's: it shows `columns` columns while each can be at least the token wide, and fewer below that, down to one, with children kept in order and filling the rows left to right. Without it the grid has `columns` columns at every width. `columns` stays required: it is the most columns the grid shows.
- `padding`, `max-width` and `min-column-width` take `dimension` tokens only (the default token set has `space.*` for spacing and `size.sm`, `size.md`, `size.lg`, `size.xl`, 240 to 960 px, for widths). No target-specific length, breakpoint or per-side value exists.
- Every component whose content model is `text` or `mixed` also takes the prop `text` (string, bindable; declared in each such component's `props`, not repeated in the table). It takes its text either as content or as `text`, never both; `text` is how bound text is written, e.g. `<button id="b" text="{$.cta}"/>`. `value` is a data value (`field`, `radio`, `option`, `select`, `radio-group`, `segment`, `segmented-control`), never displayed text. The `combobox`, `date-picker` and `color-picker` `value` is the text the user sees or types.
- "needs `label`" and a starred `label`* in the Props column both mean the universal `label` attribute is required for that component (`requiresLabel` in the catalog); `label` is never declared in `props`.
- `(…)` after a content model lists the only kinds allowed as direct children. `each` is transparent (§4.3), so it is allowed wherever its own children would be, whether listed or not.
- A kind with a required parent context (`radio`, `option`, `segment`, `item`, `column`, `row`, `cell`, `tab`, `menu-item`) declares it as `allowedParents`: `radio` in `radio-group`, `segment` in `segmented-control`, `option` in `select` or `combobox`, `item` in `list`, `column` and `row` in `table`, `cell` in `row`, `tab` in `tabs`, `menu-item` in `menu`.
- A `button` with `submit="true"` submits its nearest enclosing `form`: pressing it fires that form's `submit` event, so it needs no `on-press`. If it has one, `press` fires first; each event fires once per press, also when the press comes from Enter in a field of the form. `submit` is a literal (`bindable: false`) because whether a button submits is structure, not data. A submit button outside a `form` is an error.
- A `model` shows a 3D asset. `src` is a glTF 2.0 file (`.glb` or `.gltf`) for the web, `usdz` a USDZ file (`.usdz`) for Apple platforms, which cannot read glTF, and `fallback` a still image (`.png`, `.jpg`, `.jpeg` or `.webp`) shown while the model loads, where it cannot be shown, and in design tools; `label` is its text alternative. Each path is a literal (`bindable: false`): a path relative to the project, or an `https` URL, of at most 2048 bytes, with no control character, whitespace, backslash or (in a relative path) scheme, `..` segment, leading `/`, `%`, `?` or `#`, and with the extension above in any case; anything else is `W317`. A renderer applies the same rule again before it uses a path, and a document is never allowed to name a `file:`, `data:` or `http` location. The still is also the only part a screenshot is taken of: a live viewer is not deterministic.
- `field` has role `textbox`; a renderer MAY refine it from `type` (`number` → `spinbutton`, `search` → `searchbox`) as ARIA requires.
- `column` is a direct child of `table` although ARIA places `columnheader` inside a `row`; the renderer emits the header row. On the web the header row sits in a header `rowgroup` and the rows in a body `rowgroup` (`<thead>`/`<tbody>`), and those groups are part of the declared tree.
- `tabs.selected` names the selected `tab` by `id`; when it is absent or names no tab, the first tab is selected. Only the selected tab's panel is exposed.
- `form` and `section` are landmarks only when they have a `label`: ARIA exposes `form` and `region` only with an accessible name, so an unlabelled `form` has no role of its own (its children are exposed directly).
- A `dialog` is shown only while `open` is true; an absent `open` means closed, like every boolean prop without a default.
- A `select` always has one option selected: the one whose `value` equals `select.value`, otherwise the first.
- A `combobox` is an editable text input with suggestions, where a `select` is a choice among its options. Its `value` is the text in the input, whatever the user typed or picked, and need not match any option; the options only suggest. Picking an option writes that option's `value`; the option's content is the suggestion's label. A renderer MAY filter the suggestions by the text. On the web the suggestions link to the input by the element's `id`.
- `slider` and `stepper` hold a number in `value`. A renderer reads a `step` that is not a number above 0 as 1 and a `max` below `min` as `min`; it shows a `value` outside `min` and `max` as the nearest bound, a `value` that is absent or not a number as `min` (`slider`) or as 0 within the bounds (`stepper`), and does not write the corrected number back. A `slider` always has both bounds (defaults 0 and 100); a `stepper` is unbounded on a side without `min` or `max`. A value between two steps is shown as the platform rounds it.
- `date-picker.value`, `min` and `max` are text in the format of `type`: `date` is `yyyy-mm-dd`, `time` is `hh:mm` on a 24-hour clock, `datetime` is `yyyy-mm-ddThh:mm`, all without a time zone and without seconds, as the HTML `date`, `time` and `datetime-local` inputs write them. A `value` that is not valid in that format is an empty picker where the platform can show one, otherwise the `min`, or when that is absent 2000-01-01 at 00:00. A `value` outside `min` and `max` is kept as it is: the picker does not let the user choose it again.
- `color-picker.value` is a seven-character hex colour, `#rrggbb`, any case on input and lowercase when written; no transparency. An absent or invalid value shows `#000000`.
- `segmented-control` is a `radio-group` drawn as joined segments: the same roles, the same `value` rule (the chosen `segment` is the one whose `value` equals the control's, none otherwise) and the same keys. A renderer without a segmented look draws it as a radio group.
- `date-picker` and `color-picker` have the role `textbox` because no ARIA role is defined for a native date or colour input and an accessibility tree built from one reports a textbox.
- `column.sort` states the current sort only: the host updates it in its data model in response to the column's `press` action, because documents carry no behaviour.
- `dialog.open` is `writable`. Writable bindings are how a renderer reports user input to the host's data model; the document itself carries no behaviour. When the user dismisses an open dialog (Escape on the web), the renderer writes `false` to the path `open` is bound to and fires `close`. Every other change, such as opening the dialog or closing it from a button's `press`, is the host updating that path in response to the action. A literal or negated `open` cannot be written, so dismissing such a dialog only fires `close`.
- "integer 1–6" is a `number` prop with `integer: true`, `min: 1`, `max: 6`; "integer ≥ 1" has `integer: true`, `min: 1`. Validation holds literals to the bounds; a renderer brings a bound value outside them into range, rounding it to a whole number where the prop is `integer` and then clamping it to `min` and `max`.

## 6. Validation

Validation has three layers, each reporting diagnostics rather than throwing:

1. **Syntax** — §2. The document is well-formed restricted XML.
2. **Schema** — the tree matches the catalog: known kinds, known and correctly typed props, numbers within their declared `min`, `max` and `integer`, required props present, declared slots, states and events.
3. **Semantics** — unique ids, parent/child rules, binding paths resolve to a loop variable in scope, token references exist in the supplied token set (when one is supplied), action names exist in the supplied action list (when one is supplied), binding paths are declared, with a type the attribute takes, in the supplied data schema (when one is supplied, §10.5), a prop the catalog marks `references` holds the id of an element of that kind (`tabs.selected` names a `tab`), the root is the kind the catalog marks `root` (`screen`) and that kind stands nowhere else, a `submit` button inside a `form`, the asset paths of a `model` (`W317`), `grow` only on a child of a `stack` (`W318`), a component whose content model is `text` or `mixed` takes its text from content or from the `text` prop, not both, and the context entries of §2.3 (their values, `status`, `for`, ids and limits).

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

Code ranges: `W1xx` syntax, `W2xx` schema, `W3xx` semantics, `W4xx` compatibility, `W5xx` patches, `W6xx` import (§9), `W7xx` projects (§10), `W8xx` fragments (§10.7). A code, once published, never changes meaning.

`path` addresses the element from the root: one segment per element, `kind#id`, or `kind[index]` when the element has no valid id (the index counts the parent's list, text included). A named slot adds `slot[name]`, a text child `#text[index]`, an attribute `@name` (`@on-press` for events, `@weft` for the version). The context block is `context` under the root, and an entry `entry#id`, or `entry[index]` (its position in the block) without a valid id: `/screen#login/context/entry#reset-where/@status`. Syntax diagnostics name the open elements only. For JSON that does not have the shape of §3, the path is a JSON Pointer prefixed with `#`, e.g. `#/root/children/0/kind`. A patch diagnostic (§7) about the patch itself points into the patch list the same way, e.g. `#/patches/2/parent`.

### 6.2 Codes

`mode` severity is a warning in lenient mode and an error in strict mode (§8). `W602`, `W702`, `W710` and `W714` are warnings. Every other code is an error.

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
| W120 | `<context>` misplaced or malformed (§2.3): not a direct child of the root (also inside `<each>`, `<slot>` or `insert` markup), a second one, an attribute on it, or content other than `<entry>` elements. |
| W121 | `<entry>` misplaced or malformed (§2.3): outside `<context>`, an element inside it, an attribute it does not take, or `kind`, `by` or `name` missing. |
| W200 | Document does not have the JSON shape of §3 (or nests too deep, or the root's `props` holds `weft` or `version`). |
| W201 | Root element is not the catalog's `root` kind (`screen`), or a project fragment file's root is not `<fragment>` (§10.7). |
| W202 | Element or context entry without `id`. |
| W203 | Value not one of the enum values or declared states, or an entry's `kind`, `by` or `status` not one of its values (§2.3). |
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
| W223 | Kind, prop, event or slot name invalid or reserved (`slot`, `context` or `entry` as a kind, `id` or `on-*` in `props`). |
| W224 | Number below `min`, above `max`, or not whole where the prop is `integer`. |
| W227 | `status` missing on a context entry of kind `question` or `todo`, or given on another kind (§2.3). |
| W228 | Context over a limit: more than 100 entries, an entry text over 500 characters, or more than 16,000 characters of entry text in the document (mode, §2.3). |
| W229 | Context entry text empty, or `name` empty, over 64 characters or holding a character outside its set (§2.3). |
| W230 | `version` is not a literal `MAJOR.MINOR.PATCH` (no leading zeros, pre-release or build metadata). |
| W301 | Duplicate id, among elements and context entries alike. |
| W302 | Child kind not in the parent's (or slot's) `allowedChildren`. |
| W303 | Parent kind not in the child's `allowedParents`. |
| W304 | Content breaks the content model: text where only elements go, elements where only text goes, anything in `none`. |
| W305 | Binding uses a loop variable that is not in scope. |
| W306 | Token not in the supplied token set. |
| W307 | Token `$type` differs from the prop's `tokenType`. |
| W308 | Action not in the supplied action list. |
| W309 | Id reference points at no suitable element (a `references` prop that names none of its kind, or an entry's `for` that names no element or names the root). |
| W310 | Text given twice: a `text` or `mixed` component has both content and the `text` prop. |
| W311 | Loop variable shadows an enclosing one. |
| W312 | The `root` kind (`screen`) below the root. |
| W313 | `button` with `submit="true"` outside a `form`. |
| W314 | `<each>` without an element child. |
| W315 | Binding path not declared in the supplied data schema (§10.5). |
| W316 | Data type at the binding path is not one the attribute takes (§10.5). |
| W317 | Asset path of a `model` (§5.1) is not a relative path or an `https` URL, has the wrong extension, or is too long. |
| W318 | `grow="true"` on an element whose parent is not a `stack` (§2.2): the root, a child of another kind, or the content of a named slot. |
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
| W510 | `add-context` uses an id that an element or entry already has. The hint suggests a free one. |
| W511 | `set-context`, `resolve-context` or `remove-context` names an id that no entry has. The hint names the nearest entry id, or says that the id is an element's. |
| W512 | A context patch the host does not allow: `add-context` whose `by` or `name` differs from the host's `author`, or any context patch when the host made context read-only. |
| W513 | `add-fragment` names an inline fragment the document already has (§10.7). |
| W601 | Imported input cannot be read: a snapshot that is neither Playwright aria snapshot YAML nor an accessibility tree, HTML that is not a string, or a catalog the importer cannot map with. |
| W602 | Imported input exceeds an import limit (length, element count or nesting depth); the rest is not imported. |
| W701 | Project file is not JSON or not an object, or a member or setting has the wrong type (§10.2, §10.6). |
| W702 | Unknown member or setting in the project file. |
| W703 | File name in the project file is absolute, leaves the project directory or is malformed; or a package name that is not an npm name. |
| W704 | File named by the project cannot be read or is not JSON; or a catalog package that is not installed or names no readable catalog. |
| W705 | Problem in the project's token files or resolver (§10.3). |
| W706 | Catalog extension, or one of its entries, is not a valid catalog or definition (§10.4). |
| W707 | Catalog extension narrows or changes the core catalog (§10.4). |
| W708 | Action name in the project file breaks the action grammar. |
| W709 | Data schema is malformed (§10.5). |
| W710 | Data schema uses a keyword Weft does not support; that part accepts any data (§10.5). |
| W711 | Two catalogs claim the same catalog name, prefix or kind; the later claim is ignored (§10.4). |
| W712 | Catalog prefix is malformed or reserved, or a second catalog has no prefix; the catalog is ignored (§5, §10.4). |
| W713 | Catalog defines or extends a kind it does not own: a library's kind outside its prefix, or a project catalog's new kind under a library's prefix (§10.4). |
| W714 | Catalog requirement names a catalog that is not loaded, or is loaded at an incompatible version (§5, §10.4). |
| W715 | Fragment name claimed outside its owner: a library's fragment not named `<prefix>-…`, or a fragment in `weft.json` named under a loaded library's prefix. That fragment is left out (§10.4). |
| W716 | Library fragment reaches the host directly: a binding reads `$.…`, or an `on-*` value names an action instead of reading an action parameter (§10.4). |
| W801 | `<use>` names no fragment of the project (mode, §10.7). |
| W802 | `<use>` gives an attribute, `on-*` or slot that its fragment does not declare (§10.7). |
| W803 | Malformed `<param>` or `<fragment>`: a bad declaration, a duplicate or misplaced `<param>`, a fragment without a body (§10.7). |
| W804 | `<outlet>` names no slot parameter, places one twice, has other attributes or content, or stands outside a fragment (§10.7). |
| W805 | Fragments use each other in a cycle; the use that closes it expands to nothing (§10.7). |
| W806 | Expanding the fragments gives more than 10,000 elements or nests deeper than 256 levels (§10.7). |
| W807 | Parameter read where its type cannot go: an action or slot parameter in a prop, a value parameter in `on-*`, a read with more after the name (§10.7). |
| W808 | Inline `<fragment>` misplaced or malformed: not a direct child of `<screen>`, after the body, no valid `name`, another attribute (`weft` included), a duplicate name, a name a project fragment already has, or inside a fragment file. That fragment is left out (§10.7). |
| W809 | Variant misdeclared: `variant` on a parameter that is not an enum or on a second one, `<variant>` without a variant parameter or outside a fragment, a value covered twice or not at all, a body beside the variants, another attribute (§10.7). |
| W810 | Reported by `weft version-check` only: the new version is lower than the changes require (§8). |

Diagnostics are written for a model that will repair the document: they name the exact location, the expectation and the nearest valid alternative.

### 6.3 Streaming

A model writes markup front to back, so a reader can show a screen while it is still being written. Flat formats do this with id lists; nested markup does it with a prefix. Parsing in **partial** mode (`parse` with `partial: true`) takes markup that stops anywhere and returns the longest finished prefix as a document, with the problems the cut itself causes kept apart from real ones:

- **Kept.** Every element whose start tag is complete, with its attributes, and the text read so far. An element still open at the end stays in the tree; its children so far are its content. A document appears as soon as the root start tag is complete; before that there is none.
- **Dropped.** What the cut interrupts and cannot yet mean anything: a start tag without its `>`, an end tag without its `>`, a reference without its `;` (`&am`), and a comment without its `-->`. A start tag is dropped whole, because attributes cut short would build a wrong element.
- **`pending`.** The result has `diagnostics` and `pending`. `pending` holds the diagnostics a later chunk can still fix: `W110` for each element, tag or comment not finished, `W114` while there is no root, `W115` for an unfinished comment, `W208` and `W314` on an element that is still open (its slot or repeated element may be the next thing written), `W803` when an inline `<fragment>` is still open and has no body yet, and `W309` on any reference (its target may come later). Everything else is reported in `diagnostics` exactly as for a complete document, so an error in the finished part (a mismatched closing tag, an unknown attribute value) still blocks the document. A document that stops cleanly after its closing tag has an empty `pending`.
- **Monotone.** Appending text never removes an element from the prefix; it only adds elements and text. The text of the last open element grows in place.

A reader keeps the text received so far and parses it again after each chunk; the result of the last chunk equals the result of `parse` on the whole text. No state is carried between calls, so any engine, runtime and catalog works the same way. A renderer shows `document` as it is and may mark the paths in `pending` as still being written.

## 7. Patches

Agents edit documents with patches addressed by `id`:

```ts
type Patch =
  | { op: "set"; id: string; fragment?: string; prop: string; value: Value | null }     // null removes the prop
  | { op: "insert"; parent: string; fragment?: string; slot?: string; index?: number; markup: string }
  | { op: "remove"; id: string; fragment?: string }
  | { op: "move"; id: string; fragment?: string; parent: string; slot?: string; index?: number }
  | { op: "add-fragment"; markup: string }
  | { op: "remove-fragment"; name: string }
  | { op: "add-context"; entry: Entry }                                // appends
  | { op: "set-context"; id: string; field: "text" | "kind" | "for"; value: string | null }
  | { op: "resolve-context"; id: string }
  | { op: "remove-context"; id: string }
  | { op: "set-version"; value: string | null };                  // null removes Document.version
```

`applyPatches(document, patches, { catalog, mode?, tokens?, actions?, author?, context? })` takes the patch list as untrusted input (any JSON value), never throws and never changes `document`. It returns `{ document?, diagnostics }`.

- **Atomic.** The patches apply in order to a copy, so a later patch sees the effects of earlier ones. The result is canonicalized and validated as in §6 with the given options. If any patch fails or the result has errors, nothing is applied: `document` is absent and `diagnostics` explain the first failing patch (or the validation errors). Otherwise `document` is the result and `diagnostics` holds its warnings. A patch list does not repair a document that was already invalid: its errors are reported.
- **Shape.** A patch has exactly the members shown and nothing else; `index` is a non-negative integer. A list that is not an array, or a patch that breaks the shape, gives `W501`, one diagnostic per problem, each pointing at `#/patches/<n>/…`. An empty list is valid and changes nothing.
- **Addressing.** `id`, `parent` name elements by id, including `<each>`. A name that no element has gives `W502` with the nearest id as the hint. `<slot>` has no id: it is addressed by `parent` and `slot`. Optional `fragment` on `set`, `insert`, `remove` and `move` names an inline fragment (§10.7); with it, `id` and `parent` address that fragment, its root included (the fragment's own name addresses the `<fragment>` element). A name that no inline fragment has is `W502`, with the nearest inline name as the hint. Without `fragment`, addressing is the screen, as before, and never reaches into a project fragment.
- **`set`.** `value` is a Value of §3 and is stored as given, so it is typed JSON, not markup text: `7`, `true`, `{ "bind": "$.name" }`, `{ "token": "space.md" }`. Strings are always literals, so a string that starts with `{` needs no escape. `null` removes the prop; removing an absent prop does nothing. The one exception to storing as given is `text` on an element that holds its text as content (**Text** below). A prop name starting with `on-` binds or unbinds the event after it: the value is an action name, or `null` to remove the binding. `id` cannot be set (ids are stable; to rename, remove the element and insert it again), nor the root's `weft`. The prop name must follow the name grammar (§2). Whether the prop, event or value is allowed is left to validation (`W203`, `W204`, `W206`, …).
- **`insert`.** `markup` is one or more sibling elements, parsed like a document (§2) but without a `<screen>` root; literals are typed by the catalog. It must hold elements only (`W508`), with ids that no element of the document has (`W509`; syntax errors keep their `W1xx` codes and point into `#/patches/<n>/markup`). They go into the default slot of `parent`, or into the slot named by `slot` (a name the parent's component declares, `W504`; `<each>` declares none, extension and unknown elements accept any name). `index` counts the entries of that list, text included, and defaults to the end; it must not exceed the length (`W505`).
- **`remove`.** Deletes the element and everything inside it. The root cannot be removed (`W507`).
- **`move`.** Takes the element out and puts it into `parent` as `insert` would, keeping its id and content. `index` counts the target list after the element has left it, so the final order is the one the index names, also when the target is the list it came from. The root cannot move (`W507`); an element cannot move into itself or its descendants (`W506`).
- **Context.** The last four operations edit the context block (§2.3) and never touch elements. `add-context` appends `entry`, typed JSON exactly as in `Document.context`, so its text needs no XML escaping; its id must be new to the document, elements and entries alike (`W510`). `set-context` changes one field: `value` is a string, and `null` is allowed only for `for`, which makes the entry about the screen. Changing `kind` to `question` or `todo` sets `status` to `open` when it is absent; changing it to any other kind drops `status`. `id`, `by` and `name` cannot be changed: to restate someone else's note, add an entry of your own. `resolve-context` sets `status` to `resolved`; resolving a resolved entry does nothing, and resolving a kind without status fails validation (`W227`). There is no reopen: a question that comes back is a new question. `remove-context` deletes the entry. An id that no entry has gives `W511`. Values (`kind`, `for`, the text) are left to validation (`W203`, `W309`, `W228`, `W229`), as `set` leaves them.
- **Element patches leave context alone.** `move` keeps the id, so `for` still holds. `insert` cannot bring entries (`W120`). `remove` of an element that an entry names, or of an ancestor of one, leaves a dangling `for`, so the list fails with `W309` and its hint names the entry (`remove-context reset-where, or set-context its for`): a `constraint` must not vanish silently with the element it protected.
- **Host options.** `author: { by, name? }` fixes what `add-context` may claim: an entry whose `by`, or `name` when given, differs is refused (`W512`). `context: "read-only"` refuses every context patch (`W512`); the default is `"read-write"`. Both are set by the host (the MCP server, a plugin), never by the model, so an agent's channel cannot write `by="human"`.
- **Inline fragments.** `add-fragment` adds one, given as `<fragment name="…">` markup. A name the screen already has is `W513`. A name the project already has is `W808` and the fragment is not added. `remove-fragment` removes one by `name`; a name that no inline fragment has is `W502`. A `<use>` that still names a removed fragment fails the result's validation (`W801`), so the list is not applied.
- **`set-version`.** Sets `Document.version` on the document root, which has no id to address. `value` is a string, stored as given; `null` removes the version. Whether the string is `MAJOR.MINOR.PATCH` is left to validation (`W230`), as `set` leaves its values. A value that is not a string or `null` is `W501`, and the patch is not applied.
- **Text.** Text is changed with `set` and `prop: "text"`, wherever the element keeps it (§5.1: content or the `text` prop, never both). When a `text` or `mixed` component holds its text as content and nothing else, the patch writes that content: a string replaces it and stays content, any other value replaces it with the `text` prop, and `null` removes it. Otherwise `text` is set like any other prop, so text already in the prop stays there, and a `mixed` component whose content holds elements keeps them and gets `W310` if it is given `text` as well. Text that shares a content list with elements (an `item` holding text beside a button) is not addressable by a patch: `remove` the element that holds it and `insert` it again with the same id.

## 8. Versioning and extensibility

- `weft` on `<screen>` is `major.minor`. A minor version only adds; a reader of `0.x` MUST accept any `0.y` document under the rules below. A major version may break.
- The format is `weft` 0.3 since the optional document `version` (§3), fragment variants and inline fragments (§10.7), minor additions: a 0.1 or 0.2 document is a valid 0.3 document and needs no migration, and writers write `weft="0.3"`. There is no per-feature gate. A 0.2 reader warns about the version (`W403`) and reads `version` as an unknown attribute and `<variant>` and an inline `<fragment>` as unknown elements, which it keeps. A 0.2 reader of canonical JSON rejects `version` and `fragments` (`W200`), which fails closed. Every 0.2 reader of this repository moves to 0.3 in the same change.
- The format moved to `weft` 0.2 with fragments (§10.7), a minor addition: a 0.1 document is a valid 0.2 document. There is no per-feature gate: a reader accepts a fragment file or a `<use>` marked 0.1. A 0.1 reader warns about the version (`W403`) and reads `<fragment>`, `<param>`, `<outlet>` and `<use>` as unknown elements, which it keeps.
- **Extensions** are elements or attributes whose name starts with `x-<vendor>-`. An extension element MUST carry `role` (its ARIA fallback) and follows `content: "mixed"`; it may have any attributes, slots and events, and its literals stay strings. A reader that does not know it renders its children inside a container with that role. Extension attributes are allowed on every element in both modes.
- **Unknown, non-extension** elements or attributes come from a newer minor version or another catalog. In *lenient* mode (default for readers) they produce a `W4xx` warning; an unknown element is treated as an extension with role `group`, an unknown attribute is kept in the model and ignored. In *strict* mode (default for writers and CI) they are errors. A newer minor `weft` version is reported the same way. Unknown and extension elements are opaque: parent/child rules skip them, but the parent's content model still applies. Undeclared events, slots, states and enum values of a known component are schema errors, not compatibility warnings.
- A reader MUST NOT drop unknown content when it round-trips a document.
- A catalog has its own semver. Removing a component, a prop, an enum value, a slot, a state or an event is a major change; adding one is minor.
- The rule does not spell out every case, so the following are fixed. A change is **major** when a document valid against the previous catalog can become invalid or mean something else, **minor** when it only admits more documents, and **none** when no document is affected. The version bump of a catalog is the highest level among its changes.
  - Major: a prop becomes required; a new prop or slot is required; a prop's `type` or `tokenType` changes; a prop's `default` changes, appears or disappears; a component's `role` changes; `requiresLabel` turns on; a component becomes the `root`; a prop gains or changes `references`; `bindable` turns off; `writable` turns off.
  - Content narrowing is major and widening is minor. For `content`, `mixed` accepts everything `text` and `nodes` accept, and `none` accepts nothing; a change to a model that does not accept everything the old one did is narrowing. For `allowedChildren` and `allowedParents` (component or slot), an absent list means any: adding a list or removing a kind narrows, removing the list or adding a kind widens.
  - Minor: a prop stops being required; a slot stops being required; `requiresLabel` turns off; a component stops being the `root`; a prop loses `references`; `bindable` or `writable` turns on; a numeric range widens.
  - A numeric range narrows when its lower bound rises, its upper bound falls, or a bound appears; the opposite is widening. A prop field the classifier does not know is major when it changes.
  - A change to a `description` only is none.
- The core catalog of §5.1 is `weft-core` 0.2.0. It moved from 0.1.0 when it gained `stack.justify`, `padding`, `max-width` and `grid.min-column-width`, all minor changes (a new optional prop with a default is not a default that appears); the format stays `weft` 0.1, as it did when the universal attributes of §2.2 grew.
- The format moved to `weft` 0.2 with the context block (§2.3), a minor addition: a 0.1 document is a valid 0.2 document. There is no per-feature gate: a reader accepts context in a document marked 0.1. A 0.1 reader of canonical JSON rejects `context` (`W200`), which fails closed; a 0.1 markup reader warns about the version (`W403`) and reads `<context>` and `<entry>` as unknown elements with role `group` (`W401`), so a 0.1 renderer would show the notes. No syntax avoids both; every 0.1 reader of this repository moved to 0.2 in the same change.
- **`weft version-check <old> <new>`** compares two versions of one file and says how far its `version` must be raised. It prints each change with its level and the least version the new file may declare. When the new file declares a lower version it reports `W810` at `@version` and exits with status 1. It reads both files as untrusted input and never throws. It reads no project setting of its own (§10.6).
  - A **fragment** is classified from its parameter signature (`weft_core::fragment::signature`) by the catalog rules above (`weft_catalog::diff_catalogs`). A use that was valid can become invalid or mean something else is major; only more uses becoming valid is minor; a body change in canonical JSON, including a change of `label` alone, is a patch; identical canonical JSON is none. The comparison ignores `version` and `weft`, so a format stamp or a version edit is not itself a change.
  - A **screen** is classified by its host contract. It is major when the screen names an action or reads a data path the previous version did not, reads a path at a type the previous did not take, gains a writable binding, or loses an element id. It is minor when it gains element ids and nothing above. Any other canonical JSON change is a patch. Identical canonical JSON, ignoring `version` and `weft`, is none.
  - Two **library catalogs** are classified together: their kinds by the rules above, and each fragment document they carry by the fragment rule. The library's `version` must be raised as far as its most changed part.
  - Below `1.0.0` the least version follows Cargo's rule, as `requires` does: an incompatible (major) change raises the minor number and anything else raises the patch number. From `1.0.0` a major change raises the major number, a minor change the minor number and a patch the patch number. When the older file declares no version, a major change requires at least `0.1.0` and any other change at least `0.0.1`; no change requires nothing.
- A host advertises `{ weft, catalogs: [{ name, version, prefix? }] }`; an agent writes only what the host advertises. A host that also checks design tokens, action names or a data schema (§10) adds `tokens`, `actions` and `data` to the advertisement, so a writer knows which of them are enforced: a category that is absent is not checked. The MCP server (`weft_capabilities`) does this; the catalog list names the core catalog first and then every catalog the project loaded (§10.4), in the order the project lists them, each library with its `prefix`. A kind under a library's prefix (`acme-button`) is an ordinary catalog kind, typed and checked; an `x-acme-button` element stays an opaque extension.

## 9. Mapping

- **To code:** a renderer maps each kind to a platform component. The reference renderer targets React and MUST produce an accessibility tree whose roles and names equal those the document declares. Text that is not part of a name appears in that tree as text runs; `stack`, `grid`, `text` and other role-`none` elements add no node of their own.
- **Material:** a `material` token (§10.3) on a `stack` or `grid` is a frosted-glass surface: the element's own background is the token's tint, with what is behind it blurred by the token's radius. The static page and the web components set `backdrop-filter: blur(<radius>)` and the tint over it, inside `@supports`, and show the opaque colour of the tint where the browser has no `backdrop-filter`; the token is three custom properties, `--weft-<path>-tint` (hex with alpha), `-solid` and `-blur`, and the element carries `data-weft-material`, which the base stylesheet selects. SwiftUI applies the token struct's `Surface` modifier (`.modifier(theme.material.glass)`): Liquid Glass (`glassEffect` with the tint) on iOS 26 and macOS 26 or later, and before that the tint over a system `Material` whose thickness the blur radius picks (below 10 `ultraThin`, 20 `thin`, 30 `regular`, 50 `thick`, else `ultraThick`); SwiftUI has no blur radius to set, and the radius is the light context's whatever the appearance, while the tint follows the appearance. Figma builds a fill of the tint colour at the tint's opacity and a `BACKGROUND_BLUR` effect of the radius; Penpot builds the same fill and a background blur of that value. The `material` prop comes back from the plugin data of the node, as every prop does; a node whose fill or blur was edited by hand is reported as a `tokens` loss, like any visual edit that has no prop. The importers read `data-weft-material` and `.modifier(theme.<path>)` back to the prop.
- **Layout:** `stack.justify`, `grow` (§2.2), `padding`, `max-width` and `grid.min-column-width` (§5.1) are not drawn by the web, Slint, Figma or Penpot targets yet. A generator keeps them as it keeps the other props it has no form for (`data-prop-*` attributes in the static page, the inert marker modifier in SwiftUI, the source comment in Slint), so a generated file with its source comment (**Provenance** below) reads back to them exactly; without the comment an importer reports what it cannot read back as a loss. SwiftUI draws them, and its importer reads each form back from the code alone: a row's `justify` is spacers (`end` one `Spacer(minLength: 0)` before the children, `center` one on each side, `space-between` an `HStack(spacing: 0)` with a `Spacer(minLength:)` of the gap between the children, and inside an `<each>` before every item but the row's first), `grow` on a row's child is `.frame(maxWidth: .infinity)`, `padding` is `.padding(theme.<token>)`, `max-width` is a full-width frame aligned as the stack's children followed by `.frame(maxWidth: theme.<token>)`, both inside the material, and a grid with `min-column-width` is the generated `WeftColumns` layout, which shows `columns` columns while each can be that wide and fewer down to one, judged by the grid's own width as on the web (an adaptive `GridItem` would not cap the count at `columns`); a `section` in it is wrapped in `WeftCell` so that its header and content stay one cell. A column's `justify`, a row's `justify` with a single child or a growing child, a bound `justify`, and `grow` outside a row stay markers. json-render keeps each one as the prop itself, so a round trip keeps it. A2UI writes `justify` on `Row` and `Column` (`start` is omitted, because it is the default; `space-between` is `spaceBetween`) and `grow` as `weight` 1. `padding`, `max-width` and `min-column-width` are `layout` losses there, as are `columns` and `wrap`.
- **Models:** the static page, the generated components and the reference renderer write a `model` as `<model-viewer role="img" aria-label="…" alt="…" src="…" ios-src="…" camera-controls interaction-prompt="none">` whose only child is `<img slot="poster" alt="" src="…">`, the `fallback`; `ios-src` is `usdz` and is left out without one. The page that shows it loads the `@google/model-viewer` script: Weft never injects a script, and where the script is missing the browser shows the child, so the still is what a page without the script and a screenshot show. SwiftUI shows the bundled USDZ with `Model3D` on visionOS and `RealityView` on iOS and macOS, and the fallback while it loads, when there is no `usdz`, and in a process that sets `WEFT_STILL_MODELS`. Design tools draw a grey 160 by 120 rectangle in place of the still; they load no file, and the paths stay in the plugin data.
- **Context in design tools:** a Figma or Penpot build keeps the document's context (§2.3) as its canonical JSON array in the shared plugin data key `weft.context` of the layer built from the root, beside the rest of the source, and never as a layer: the canvas never shows it, and copying an element's layer cannot copy entry ids. Reading the layer back gives the entries back as they were, except an entry whose `for` names an element whose layer the designer removed: it goes with the layer, as a `context` loss, instead of failing the read with `W309`. Elements a designer adds never take an entry's id. A stored value that does not have the shape of a context list is ignored, and the entries are validated with the rest of the document. The plugin panel lists the entries about the selected layer as plain text, read-only: the screen's own on the root frame, and on any other layer those about the nearest Weft element at or above it. `import.figma.context` (§10.6) leaves the context out of what the `figma-pull` script writes.
- **States:** a renderer maps a state to ARIA where an equivalent exists (`loading`, `busy`, `submitting` → `aria-busy`; field `invalid` or a non-empty `error` → `aria-invalid`) and exposes every state as `data-state` as well.
- **Trust:** a renderer never interprets document strings as markup or code. URLs in `link.href` and `image.src` are used only when they are `http`, `https`, `mailto` or relative; any other value is dropped. Every mapping in this section follows that allowlist, including A2UI `openUrl` and `Image.url`. The paths of a `model` are used only when they follow the rule of §5.1 (`W317`).
- **To HTML:** a document compiles to one static page with no script: semantic elements with the accessible structure of the reference renderer (a `text` is a block with no role, the caption of a control is hidden from the tree and the control carries its `aria-label`, a link with no usable `href` is `role="link"` with no `href`), written so that it looks the same as the generated components: no line break between inline pieces, and one `<style>` that holds the tokens as CSS custom properties (`--weft-<path with . as ->`), then the base stylesheet (below), then only the layout the components carry as inline styles, `data-weft-id` on every element, and what a static page cannot run written as inert data: `data-bind="prop:path; …"`, `data-action="event:action"`, `<template data-each data-as>` for `<each>` and `<template data-empty>` for an empty slot. A dimension in px or rem, a hex colour and a number are values as they are; a typography token is the `font` shorthand (`weight size/lineHeight families`, every family that is not a CSS generic family a quoted CSS string) plus `--weft-<path>-letter-spacing` when it has letter spacing; a token CSS has no safe form for is left out. With an appearance (§10.3) the page follows the system appearance: `:root` has `color-scheme: light dark` and the `light` context's values, and an `@media (prefers-color-scheme: dark)` block sets the properties whose `dark` value differs. `weft css-tokens` writes the same declarations as a stylesheet, `weft-tokens.css`, for the React and SolidJS components, whose token references read these properties. **Base stylesheet:** `weft css-base` writes `weft-base.css`, the one set of rules that every web target shares, which the static page inlines and a React or SolidJS host links after `weft-tokens.css`. It is a base, not a theme: it sets no page colour and no page font, only what a screen needs to read as a form. A screen, form and section are a column with a gap; a field is its caption above its control (an input, a textarea or a select), with the error under it; a checkbox, a switch (drawn as a track and a thumb) and a radio are a row, the control then its caption; a radio group is its caption above its rows; a link, an `a` or any `role="link"` element, has the accent colour and an underline; a button has an outline, `primary` and `danger` are filled, and a disabled one is dimmed; tabs, an alert and a `tone`, the `footer` and `actions` slots (one row), a table, a list and its items, and a menu have the few rules they need to be told apart. Every value is a token as a CSS custom property, with the default token's value as its fallback, so the sheet also works without `weft-tokens.css`: spacing `space.xs`, `space.sm` and `space.md`, `radius.sm` and `radius.pill`, `font-size.sm`, `color.white`, `color.action.primary` and `color.action.danger`, and the optional `color.success` and `color.warning`. Borders and muted text mix `currentColor`, so they follow the light and dark appearances of §10.3. The rules select only elements and attributes that the page, the components and the reference renderer all write (never a tag the page and the components spell differently), so the targets look the same, and they use `:has()` (Chromium 105, Safari 15.4, Firefox 121 or later). Event handler attributes, `javascript:` URLs and scripts never appear. With sample data the page shows that data instead: each binding is replaced by the value it reads (§2.1, own properties only; a value that does not fit the prop is dropped or clamped as in the components), `not` and truthiness read as in the components, `<each>` becomes one copy of its content per item with `id[index]` instance ids, and a list or table with no items shows its `empty` slot in place (`<li role="none">`, or a row with one cell spanning every column). Such a page carries no `data-bind` or `<template>`: it is a picture of the screen with that data, not a template.
- **Provenance:** a generator may keep the canonical form of the screen in a leading comment (`<!-- weft:source … -->` in HTML, `/* weft:source <framework> name=… */` in JSX, and in Swift, whose block comments nest, a `// weft:source swiftui` line followed by one `// ` line per markup line, split at every line break). An importer that finds the comment parses its markup and accepts it only if generating from it reproduces the input (HTML compared after parsing, JSX by syntax tree, ignoring comments and formatting); then the import is exact, with no losses. SwiftUI code also depends on tokens, appearance and sample data that its importer is not given, so the SwiftUI importer instead accepts the comment only if the code, read as below, gives the comment's screen with its uses expanded and without its context, with no loss and no diagnostic; its reading of generated code is exact, so this accepts what the generator printed. An edited file falls back to reading the code. The comment escapes with `\` every sequence that would end or nest it, and follows every `@` with a `\`, so no note can carry the `@license` or `@preserve` marker that makes a minifier keep a comment in a production bundle. The context block (§2.3) travels in the comment as part of the screen; `export.<target>.context` (§10.6) strips it first, by default for the static page only, since a deployed page shows its comments to anyone who opens its source, and `import.<target>.context` drops what an import reads back. React and SolidJS components also show each entry to developers as a readable `{/* <kind> (<by> <name>): <text> */}` comment, and SwiftUI files as a `// <kind> (<by> <name>): <text>` line, escaped the same way, with line breaks as spaces: above the element it names, or first inside the root (the view's `body` in SwiftUI) for an entry about the screen. These comments are derived from the context, so importers ignore them. Lit templates are strings that reach the DOM, so they carry none.
- **To JSX:** a document compiles to one self-contained React function component `({ data, actions, onChange })`, or SolidJS component `(props)` reading `props.data`, `props.actions` and `props.onChange`, as JSX or as TSX with typed props, with the accessible structure of the reference renderer. A binding becomes a read of `data` or of a loop variable that reads own properties only, `not` becomes `!`, `<each>` becomes `.map` with the item index as key and `id[index]` instance ids, an event calls `actions[name]` with `{ id, action, item }` when the host supplied that action, a writable prop becomes a controlled input that calls `onChange(path, value)` with the absolute data path, a token reference becomes `var(--weft-<path with . as ->)`, and named slots go where §5.1 places them. A SolidJS component also marks the chosen option of a select `selected`, so that server rendering shows it. Document strings appear in the output only as escaped string literals or as JSX text made of inert characters, so a document cannot inject code.
- **To Lit:** the same document compiles to one `LitElement` subclass, registered as a custom element named after the component (`weft-screen` for `WeftScreen`, a hyphen added when the name has none), that takes `data`, `actions` and `onChange` as properties and renders `html` templates with the accessible structure of the reference renderer. It renders into the light DOM, so the page's stylesheet (the base rules and tokens of the other web targets) and the ids that labels refer to reach its controls. The mapping is the React and SolidJS one: `<each>` becomes `.map`, a value becomes a property binding (`.value`, `.checked`), a boolean state a `?disabled`-style attribute and an event an `@click`-style listener. A template keeps the whitespace between its elements, so none is written there: line breaks stand only inside tags. Document strings reach the output only as string literals, as attribute or text runs of inert characters, or as values of a binding. The target is JavaScript only: it has no TypeScript flavour and no `weft:source` comment, and asking for either is an error.
- **To SwiftUI:** a document compiles to one Swift file for iOS 17 and macOS 14: an `@Observable` model class whose properties are the data the bindings read (types inferred from the props that read them), a `CaseIterable` action enum with one case per action the document names (its raw value is the action name), an event struct `{ action, id, item }` (`item` is the data path of the list item inside `<each>`), and a view that takes the model (`@Bindable`), the tokens and a `perform` handler. The tokens are either the shared `WeftTokens` struct, which a separate `WeftTokens.swift` holds once for the whole token set of a project, or a theme struct in the screen file with only the token values the document references, so a single screen builds on its own; `export.swiftui.sharedTokens` (§10.6) picks one. Colours become `Color`, dimensions points, durations seconds, cubic Béziers `UnitCurve`, typography `Font`, or, with `letterSpacing` or `lineHeight`, the token struct's `Typography` view modifier (`tracking`, and line spacing of `lineHeight` × size less the font's own line height, half of it padding the first and last line); with an appearance (§10.3) a colour whose `dark` value differs from its `light` one is built by the token struct's `adaptive(light:dark:)` and follows the system appearance, while other tokens take the default context; a token SwiftUI has no form for is left out of `WeftTokens.swift`, and a screen that references it is not generated. A kind of the project's catalog extension that the core catalog lacks becomes a call of a view the app writes, named after the kind (`promo-card` → `PromoCardView`): the props are labelled arguments in catalog order, then `state`, then one closure per event (`onDismiss`), then the content and every slot as view-builder closures. A missing view is a compile error. Ids become `.accessibilityIdentifier`, labels become `.accessibilityLabel` or the control's title, `<each>` becomes `ForEach` over the enumerated items, a writable prop becomes a binding into the model, and `hidden` removes the element from the view. Props SwiftUI has no form for (`required`, `sort`, `state`, `error`, …) are kept as an inert marker modifier so they read back. The number controls read and write a `Double` in the model, and a range is built so that it is never reversed (a `max` below `min` is `min`, as in §5.1); a `date-picker` or `color-picker` converts its text to a `Date` (in UTC) or a `Color` and back; a `segmented-control` is a `Picker` with the segmented style; SwiftUI has no combobox, so one becomes a text field beside a menu of its options. With sample data, the model also gets an initializer that takes every property (each defaulting to its declared value) and an extension with `static var sample`, the model built from the data with each value read as the property's type (§2.1), and `#Preview` shows `.sample`; the importer ignores both, so the file reads back to the same document. Document strings appear only as escaped string literals, and in the line comments of the source comment and the context notes (Provenance), which open the file and sit above the views entries name. A screen that strict validation rejects, or that has `x-` elements or attributes, is not generated.
- **From SwiftUI:** Swift source is parsed, never compiled or run; the first view no other view in the file uses is the screen, and views it uses from the same file are inlined. Source that the generator printed reads back to the same document with no losses, and with its `weft:source` comment (Provenance) to the screen as the comment holds it, context included. Other SwiftUI maps view by view (`VStack`/`HStack` → `stack`, `Text` → `text`, `Button` → `button`, `TextField` → `field`, `Toggle` → `switch`, `Picker` → `select`, `List`/`Form` → `list`/`form`, `TabView` → `tabs`, `.sheet` and `.alert` → `dialog`, `ForEach` over a data path → `<each>`, `if` on a data path → `hidden`, the view of a catalog-extension kind → that kind), with `{ document, losses, diagnostics }` as above. The input is bounded like the other importers (`W602`), a source with no view is `W601`, and the losses are:
  - `ids`: a view without a valid, unique `.accessibilityIdentifier` gets a generated id.
  - `bindings`, `actions`: an expression that is not a data path, and a handler that does more than name one action, are not kept.
  - `tokens`, `layout`: spacing that is not a theme token, a `.padding`, `.frame`, `Spacer` or `WeftColumns` width other than the forms the generator prints for the layout props (Layout above), and modifiers Weft has no prop for (fonts, colours), are dropped.
  - `repetition`, `hidden`: a `ForEach` or `if` over anything but a data path is imported once, as shown.
  - `slots`, `props`, `values`, `names`, `text`: as above; interpolated text keeps its literal parts.
  - `kinds`: a view with no Weft kind (a custom view from another file, a shape, a drawing) is dropped.
  - `structure`: content a kind cannot hold is dropped, and a source with syntax errors notes the parts the parser skipped.
- **To Slint:** a document compiles to one `.slint` file for Slint 1.x: an exported component `<Name>Screen` that inherits `Window` (its `title` is the screen's `label`), one `in-out property` per root data path a binding reads (`$.user.email` is `user-email`, typed `string`, `bool`, `float` or `int` from the props that read it; a boolean read of text or a number is its truthiness, §2.1, and `not` is `!`), and one callback `perform(action, id)` that every event calls, a submit button and Enter in a field of the form calling the form's `submit` action. Ids are the Slint element ids (`email := LineEdit`). Kinds map to the std widgets: `screen` → `VerticalBox`, `stack` → `VerticalLayout` or `HorizontalLayout` (a `gap` token is `spacing` in px), `grid` → `GridLayout` (children placed by `row` and `col`), `section` → `GroupBox`, `form` → `VerticalBox` with the `footer` slot in a `HorizontalBox`, `heading` and `text` → `Text`, `link` → a `TouchArea` around a `Text`, `button` → `Button`, `field` → a caption `Text` above a `LineEdit` (`TextEdit` when `multiline`), `checkbox` and `switch` → `CheckBox` and `Switch`, `select` → `ComboBox` (options as its model, `value` mapped to `current-index`), `slider` → `Slider`, `stepper` → `SpinBox`; writable props are two-way bindings (`<=>`), `disabled` is `enabled` and `hidden` is `visible`. Props Slint has no property for (`state`, `required`, `error`, the field types `email` and `search`, `variant="danger"`, `tone`, `stack.align`, `justify` and `wrap`, `padding`, `max-width`, `min-column-width`, `grow`, `step`, `href`, `material`) are kept only in the source comment below. Document strings appear only as Slint string literals in which every backslash is doubled, so no `\{…}` interpolation can form. A screen that strict validation rejects, or whose ids or property names Slint would read as one (it treats `-` and `_` alike) or as a reserved name, is not generated. A path whose name is a property `Window` already has (`$.title`, `$.width`) is declared with `-data` appended (`title-data`), so it is not that property. An `<each>` adds one `in-out property` for the array it repeats (`[{ field: type }]`, typed from the props read on that array, or `[string]` when a `select` or `combobox` repeats its options), and a path with an index into that array (`$.players.0.name`) reads `root.players[0].name`. An element id equal to the repeater's `as` is not written on the element, because Slint would read it as the repeater variable; `perform` still receives the id.

| Kind | Slint |
| --- | --- |
| `each` | `for <as> in root.<model> :` the element it repeats |
| `list` | `ListView` when its only element is one `<each>`; otherwise `VerticalLayout`. `ordered` stays in the comment. An `empty` slot is `if <model>.length == 0` beside the `ListView`, or the slot itself when the list does not repeat |
| `item` | the repeated element: `Text` when it is only text, `TouchArea` when it has `on-press`, otherwise `VerticalLayout` |
| `table` | `StandardTableView`. A repeated row is not projected onto `rows` (`[[StandardListViewItem]]`): Slint has no map from a struct model, so those cells stay in the comment. An `empty` slot is an `if` on the model length, or the slot itself when the table has no model |
| `column` | a `TableColumn` (`title`, and `sort-order` from `sort`). `sort-ascending` and `sort-descending` call the column's `press` |
| `row` | `current-row` when `selected` is true, and `row-pointer-event` on pointer up calls `press`. A bound `selected` stays in the comment |
| `cell` | `{ text }` of a literal row. Child widgets stay in the comment |
| `tabs` | `TabWidget`. `selected` is `current-index` by tab id, and `changed current-index` writes the id back. `label` is `accessible-label` together with `accessible-role: tab-list` |
| `tab` | `Tab` (`title` from `label`) around a `VerticalLayout` |
| `dialog` | `PopupWindow`. `open` calls `show` and `close`; the label is a `Text`. `modal` is not a property: unless it is false, the popup sets `close-policy: no-auto-close` |
| `menu` | `MenuBar` containing a `Menu` (`title` from `label`), as a direct child of the `Window`: a `MenuBar` cannot sit in a layout |
| `menu-item` | `MenuItem` (`title`, `activated` for `press`) |
| `image` | `Image` (`accessible-label` from `label`). `src` is a string and is not `@image-url`, which would load a file |
| `model` | `Image` (`accessible-label` from `label`). Slint 1.18 has no 3D widget, so `src`, `usdz`, `fallback` and the tilt props stay in the comment |
| `alert` | `VerticalLayout` of its text and children. `tone` stays in the comment |
| `date-picker` | `LineEdit` (`text` from `value`). `DatePickerPopup` and `TimePickerPopup` hold a `Date` or a `Time`, not the string; `type`, `min` and `max` stay in the comment |
| `color-picker` | `LineEdit`. Slint 1.18 has no color-picker widget; the string `value` is the text |
| `radio-group` | `RadioGroup` (`title` from `label`). `checked` compares `value` with each option's literal value, and `selected` writes that value back. `current-value` is an `out` property of the button text, so it is not the binding |
| `radio` | `RadioButton` (`text`, `enabled` from `disabled`) |
| `segmented-control` | the same `RadioGroup` with `orientation: Orientation.horizontal` |
| `segment` | `RadioButton`, as a `radio` |
| `combobox` | `ComboBox`, as a `select`. `placeholder` stays in the comment. Options from `<each>` are the `model` as `[string]`: `ComboBox.model` is `[string]`, and Slint cannot project a struct field |

- **From Slint:** the generator writes the canonical markup in a leading `// weft:source slint` line comment, one markup line per comment line. `import_slint` reads the document from that comment and accepts it only when generating from it reproduces the file, compared line by line with indentation and blank lines ignored. A file that fails that check, or that has no comment, is parsed with the syntax tree of `i-slint-compiler` (never compiled or run) and returned as `{ document, losses, diagnostics }`. The input is bounded like the other importers (`W602`); a source with no component is `W601`. A brace in a literal that would read as a binding or a token reference is replaced, as the other importers replace it. `Window` is the `screen` (a string `title` is `label`). A wrapping `VerticalBox` or `VerticalLayout` whose only property is `alignment: start`, and that has no callback, is the screen itself and its id is the screen id. `VerticalLayout` is a column `stack` and `HorizontalLayout` a row `stack`. `VerticalBox` is that same stack. `HorizontalBox` is a row `stack`. `GridLayout` is a `grid` whose `columns` is the number of children. `GroupBox` is a `section` (a string `title` is `label`). `Text` is `text`. A `TouchArea` around a `Text` that has no id is a `link`. `Button` is a `button`. `LineEdit` and `TextEdit` are a `field` (`TextEdit` is `multiline`; a string `text` is `value`). `CheckBox` and `Switch` are `checkbox` and `switch` (a string `text` is `label`). `ComboBox` is a `select`, or a `combobox` when the source comment gives that id the kind `combobox`. `Slider` and `SpinBox` are `slider` and `stepper`. A `for` is an `each`, still read once, and its variable is `as` when the name is a loop variable. A `ListView` is a `list`, as is a `VerticalLayout` whose element children include one id-less `ListView`; the element that `for` repeats in a list is an `item` when it is a `Text`, a `TouchArea` or a `VerticalLayout`. A `StandardTableView` is a `table` (a string `accessible-label` is `label`), as is a `VerticalLayout` whose element children include one id-less `StandardTableView`; its `columns` and `rows` are not read. A `TabWidget` is `tabs` and a `Tab` is a `tab` (a string `title` is `label`); a property-less `VerticalLayout` that only wraps a tab's body is not an element. A `PopupWindow` is a `dialog`, and a string `Text` with no id at the start of its body is the `label`. A `MenuBar` is a `menu` and stays a child of the `screen` when it sits beside the screen box; an id-less `Menu` that is its only child is that same menu (a string `title` is `label`), and a `MenuItem` is a `menu-item`. An `Image` is an `image` (a string `accessible-label` is `label`), or a `model` when the source comment gives that id the kind `model`. A `VerticalLayout` is an `alert` when the source comment gives that id the kind `alert`; `tone` stays in the comment. A `RadioGroup` is a `radio-group`, or a `segmented-control` when its `orientation` is `Orientation.horizontal`. A `RadioButton` is a `radio`, or a `segment` inside that horizontal group. An element id is the Weft id. An element whose type is not one of these is a `kinds` loss and its children are still read. Where the file also has a source comment, text that differs from that comment is a `text` loss and the file's text is kept. Expressions the generator prints are read back. A `root` property is that binding (`user-email` is `$.user.email`; a `-data` suffix is the `Window` property it was kept off, so `title-data` is `$.title`), including a two-way `<=>`. A boolean binding is the property, `!` it, or its truthiness (`!= ""` for a string, `!= 0` for a number; `==` is the negated form). `enabled` and `visible` are `disabled` and `hidden` with that binding read negated. `root.perform("action", "id")` is the event of the element with that id: `clicked` and `activated` are `press`, `edited`, `toggled`, `changed` and `selected` are `change`, and a call whose id is not the element that holds the callback is that id's `submit`. A `Text` with no id, alone above a `LineEdit`, `TextEdit`, `Slider`, `SpinBox` or `ComboBox` in a `VerticalLayout` with no id, is that control's `label`. A `Text` with `font-weight` 700 and `font-size` 32, 24, 19, 16, 13 or 11 px is a `heading` of level 1 to 6. A `ComboBox` `model` of string literals is `option` children, and `current-index` is `value` (a number is that option; a chain of `==` ternaries is the binding). A `spacing` in px that equals exactly one dimension token, a `rem` token counted at 16 px, is that token. Losses:
  - `ids`: an element without a valid, unique id gets a generated id.
  - `bindings`: an expression other than a string literal or one of those reads is not kept.
  - `actions`: a callback that does not only call `root.perform` for an event named above is not kept. The `selected` write-back `root.<property> = [<values>][self.current-index]` is part of the `value` binding, not an action.
  - `tokens`: a `spacing` that is not a px length, or that equals no dimension token or more than one, is not a token.
  - `layout`: a `font-size` that is not one of those sizes at weight 700 is not a heading level; a link colour other than the accent is not kept; a grid's `row` and `col` are not placed.
  - `repetition`: a `for` is an `each` read once; the model it walks is not kept.
  - `slots`: a `GroupBox` has no header slot, and a `HorizontalBox` is a row stack rather than a form footer.
  - `hidden`: an `if` is read as its element, shown.
  - `props`: a property the generator keeps only in the source comment is not in the tree.
  - `values`: a `ComboBox` model that is not a list of string literals is not read as options.
  - `names`: a required `label` the file omits is `""`.
  - `kinds`: an element that is not a mapped widget is dropped and its children are kept; a `VerticalBox` that is not the screen is a `stack`, so a `form` is not told apart from one.
  - `text`: a brace that would read as a binding or a token reference is replaced; text that differs from the source comment keeps the file's wording.
  - `structure`: the source has syntax errors, or a link holds children other than its text (those children are kept beside it).
- **From HTML, React and SolidJS source:** a page or a component is parsed, never run; a component's JSX is evaluated statically from its props (`data`, `actions`, `onChange`, destructured or not): property reads of `data` are data paths, `.map` and `<For>`/`<Index>` over a data path are `<each>`, `&&`, `?:` and `<Show>` on a data path are `hidden` (`open` for a dialog), a test of a list's `length` places the other branch in the list's `empty` slot, and handlers that call one action of `actions` are events. The conventions of the HTML generator (`data-weft-id`, `data-bind`, `data-action`, `<template data-each>`, the layout classes and custom properties) read back as such, so generated code without its source comment comes back too, except the ids of `<each>`, which JSX does not carry. Everything else is read as rendered HTML (below), with `{ document, losses, diagnostics }`: values from hooks, local state, template strings and other computations (`bindings`, `props`), spread attributes (`props`), inline style Weft has no prop for (`layout`), text written beside a bound value (`text`), components from other files (`kinds`, their children kept), handlers that do more than call an action (`actions`), and conditions on anything but a data path (`hidden`, every branch kept) are losses. A source that does not parse or returns no JSX is `W601`; one longer or deeper than the importer allows is `W602`.
- **From a Custom Elements Manifest:** a manifest of schema 2.x (`custom-elements-manifest` 2.1.0, https://github.com/webcomponents/custom-elements-manifest) is read as data, never evaluated, into a catalog extension (§10.4) with `{ catalog, losses, diagnostics }`; the caller names the catalog and gives the catalog it extends. Each custom element (a class with `customElement: true` and a tag, from `tagName` or from a `custom-element-definition` export of its module) is a kind; `attributes` and the public, non-static, writable `fields` are props, merged by attribute name or `fieldName`; `slots` are slots (the unnamed slot makes the content `mixed`, a manifest that documents no slots also gives `mixed`, otherwise `none`); `events` are events. A type text maps to `boolean`, `number`, `string`, or an `enum` of a union of string literals (`undefined` and `null` are ignored); any other type is a `string` for an attribute and not a prop for a property alone. A `default` is kept when it is a literal of the prop's type (and in the `enum`), a property or event name in camelCase becomes kebab-case, and every kind has the role `generic`, since a manifest has no roles. The manifest is untrusted: input over 10 MB, more than 2000 elements or more than 1000 members in one list of an element is cut (`W602`); a manifest that is not JSON, has no `modules` or has another schema version is `W601`; and a name, enum value, default or description that breaks the grammars of §2 and §5 never reaches the catalog (descriptions are the first sentence, at most 200 characters), so the catalog always loads without diagnostics. Losses (kinds as above): `kinds`: an element without a tag or with a tag that is not a Weft name, starts with `x-`, is already a kind of the base catalog or repeats an earlier tag, and a custom element mixin; `names`: an attribute, property, event or slot name that is not a Weft name (or is `id`, `role`, `weft` or starts with `on-`), and each rename; `props`: methods, private, static and read-only members, properties without an attribute, a type no prop holds; `values`: a default that is not a literal of the type; `slots`: a slot that is not a Weft name, and an element that documents no slots; `structure`, one entry per element: `cssParts`, `cssProperties`, `cssStates`, `demos`, `mixins`, `superclass`, `source`, `deprecated`, `reflects`, `inheritedFrom` and the payload `type` of events, and one for the package: `readme`, module descriptions, `js` exports and declarations that are not elements (functions, variables, plain classes), plus the role. `weft import-cem` reads a manifest file of at most 10 MB (a larger file is not read) and prints the catalog as JSON, or writes `<file>.catalog.json` (§10.6), with the losses and diagnostics on stderr; it extends the project's catalog unless `--catalog` names another, and the catalog is named by `--name` and `--version`, else `import.cem.name` and `import.cem.version`, else the manifest's file name without its extension and `0.0.0`. With `--prefix`, else `import.cem.prefix`, the catalog is a library (§5, §10.4): it gets that `prefix` and `requires: { "weft-core": "<the core catalog's version>" }`, and an element whose tag does not start with `<prefix>-` is a `kinds` loss. A prefix that is not one lowercase name segment stops the command. Without a prefix, when every kind starts with the same first segment, `weft import-cem` prints on stderr that `--prefix` with that segment makes the catalog a library. `importCem` of `@weft/core/cem` returns the same `{ catalog, losses, diagnostics }`.
- **From a running UI:** an accessibility snapshot (Playwright aria snapshot YAML or the same tree as objects) or rendered HTML maps back to Weft with losses. The importer maps each role to the first catalog kind with that role, except for the refinements and inversions below and for the two roles several kinds share, which map to `field` (`textbox`) and `select` (`combobox`) unless the markup says more; a role no kind has becomes the extension `x-aria-<role>` with `role` set. It returns `{ document, losses, diagnostics }`, where each loss is `{ kind, path, note }` and the document validates in lenient mode without errors.
  - Refinements: `spinbutton` and `searchbox` are a `field` with `type` `number` or `search`, `paragraph` is `text`, an exposed `dialog` has `open` true. `generic`, `none`, `presentation` and `rowgroup` add no element; their children take their place.
  - Inversions: a `tablist` and the `tabpanel`s after it become one `tabs` whose `tab`s hold their panels, and the selected tab becomes `tabs.selected`; a header row of `columnheader`s becomes the table's `column`s; a checked `radio` or selected `option` becomes the `value` of its group or select.
  - Markup says more than a role in HTML: an `<input>` of type `date`, `time` or `datetime-local` is a `date-picker` (the `type` prop is left out when it is `date`), `color` a `color-picker`, `range` a `slider`; an `<input type=number>` inside an element marked `data-weft-stepper` is a `stepper` and elsewhere a `field`; an `<input>` whose `list` names a `<datalist>` is a `combobox` with that list's options; an element with `role=radiogroup` marked `data-weft-segmented` is a `segmented-control`, and a `radio` inside a `segmented-control` is a `segment`. `min`, `max` and `step` equal to their defaults are left out, as the renderers write them out whatever the document said.
  - HTML is read with the implicit roles of HTML-AAM (a `form` and a `section` always count as `form` and `region`) and a simplified accessible name: `aria-labelledby`, `aria-label`, an image's `alt`, a control's `<label>`, a button input's `value`, a table's `<caption>`, then `title`, else the text content. Elements carrying `data-weft-id` keep it as their id; a stack or grid is recognised from its `display` style.
  - A `<model-viewer>` is a `model`: `src`, `ios-src` (`usdz`) and the `src` of its `slot="poster"` child (`fallback`) are read back, and a `WeftModel` call does the same in SwiftUI. A model with no poster or no usable `src` is not a `model` but an `image` or a loss, since `fallback` and `src` are required. An accessibility snapshot has only the role `img`, so a `model` comes back from it as an `image`.
  - Ids are generated from the kind and the name (`button-save`, `list-1`) when the input has none, and `id[index]` instance ids become `id-index`.
  - Input is untrusted: it is never evaluated, a literal that would read as a binding or token reference has its brace replaced, and length, element count and depth are bounded (`W602`).
- **To A2UI:** a document compiles to the messages of one A2UI v0.9 surface that uses the basic catalog (`https://a2ui.org/specification/v0_9/catalogs/basic/catalog.json`): a `createSurface` whose `surfaceId` is the id of the screen, then one `updateComponents` with a flat list of components whose ids are the Weft ids (the screen is the component `root`, a `Column` carrying the screen's `label` as `accessibility.label`). The messages are checked against the A2UI JSON Schemas, and the importer below reads them back. Kinds map as follows: `stack` and `grid` → `Row` or `Column` (`direction`, `align`, and on a stack `justify`; a child with `grow` has `weight` 1), `section` and `alert` → `Card` (the section's `header` slot and the alert's text become its content), `form` → `Column` (its `on-submit` action goes to the submit buttons inside it), `heading` → `Text` (`h1`–`h5`; level 6 is `h5`), `text` → `Text` (`caption` when `tone` is `muted`), `image` → `Image`, `button`, `link` and `menu-item` → `Button` (`borderless` for a link, `primary` for a primary button; a `disabled` bound to a path becomes a `required` or `not` check on that path), `field`, `stepper` and `color-picker` → `TextField` (`required` and `type="email"` become `required` and `email` checks), `checkbox` and `switch` → `CheckBox`, `radio-group`, `select`, `combobox` and `segmented-control` → `ChoicePicker` (`filterable` for `select` and `combobox`, `chips` for the segmented control), `slider` → `Slider`, `date-picker` → `DateTimeInput`, `list` and `menu` → `List` (the `empty` slot follows it), `table` → a `Column` of a header `Row` and a `List` of rows, `tabs` → `Tabs`, `dialog` → `Modal` with a generated trigger `Button` whose event is `<id>.open`; `item` and `cell` add no component. Any other kind is dropped and its content kept. A `<each>` is a template: `children: { componentId, path }` where the path is absolute (`$.todos` → `/todos`) or, inside the repetition, relative to the item (`$todo.title` → `title`). A bound value is `{ "path": … }`. An event is `{ "event": { "name": <action> } }`; a button with no action names its own id; a `link` whose `href` is an `http`, `https` or `mailto` URL is the function call `openUrl` (a relative or other-scheme `href` is a loss). The data is not part of a document, so no `updateDataModel` is written, and `deleteSurface` is never used.
- **From A2UI:** messages (a JSON array, one object, or JSON Lines) are parsed, never run. Only the first surface is read; its components are looked up by id from `root`. A `Column` or `Row` at the root is the screen itself, as the exporter writes it. The inverse of the mapping above applies: `Row` and `Column` → `stack`, `Card` → `section` when it has an `accessibility.label` and otherwise `stack`, `Text` → `heading` or `text`, `Button` → `button` (`link` when `borderless`), `TextField` → `field`, `CheckBox` → `checkbox`, `ChoicePicker` → `select` when filterable, `segmented-control` as `chips`, otherwise `radio-group`, `Slider`, `DateTimeInput`, `Tabs`, `List` (each child is an `item`), and `Modal` → its trigger and a `dialog` (the trigger the exporter generates is not read back). A template is an `<each>` whose variable is `item`, `item2`, … by depth; a `path` becomes a binding, and a path that is not a Weft data path is a loss. An event name is the action when it is a valid action name. The ids of the components are the ids of the elements. The input is bounded as the other importers are (`W602`); input that is not JSON, or has no `root`, is `W601`. The result is `{ document, losses, diagnostics }` as above, and a document exported and read back exports to the same messages after one such round trip.

| Loss kind | Accessibility snapshot | HTML |
| --- | --- | --- |
| `ids` | Always: the tree has no ids; all are generated. | Elements without a valid, unique `data-weft-id` get a generated id. |
| `bindings`, `actions` | Always: values are the ones shown; handlers are not in the tree. | Unless the page carries them as `data-bind` and `data-action`. |
| `tokens` | Always. | A token-backed `gap` is noted; token values are not recovered. |
| `layout` | Always: `stack` and `grid` are not in the tree. | Only a `gap` that is not a token variable. |
| `repetition` | Always: repeated content is static siblings, not `<each>`. | Each `id[index]` instance becomes a static sibling; `<template data-each>` is an `<each>`. |
| `slots` | Always; content a kind does not take by default goes to the first slot that takes it. | Only content outside a `data-weft-slot` wrapper (the renderers write one around a slot's content): it goes to the first slot that takes it. |
| `hidden` | Always: hidden elements, closed dialogs and unselected tab panels are absent. | Hidden elements and closed dialogs; all tab panels are kept. |
| `props` | Always: props and states without an ARIA equivalent (variant, tone, placeholder, required, sort, modal, `data-state`); also invalid values. | Invalid values only. |
| `values` | A required prop missing from the input is filled with a stand-in. | Likewise. |
| `names` | A required `label` missing from the input is set to `""`. | Likewise. |
| `kinds` | A role without a kind becomes `x-aria-<role>`. | Likewise. |
| `text` | Text with no place in the content model is dropped; a reference-like brace is replaced. | Likewise. |
| `context` | Always: the tree has no context. Nothing is reported, as there is nothing to name. | Unless the page carries its source comment (Provenance): comments are never read as context. Nothing is reported. |
| `structure` | No single `main` landmark: a `screen` root is added. | Likewise. |

Losses of the A2UI conversions (A2UI v0.9, `specification/v0_9` of `a2ui-project/a2ui` at commit `4787774`, 2026-10-05):

| Loss kind | To A2UI | From A2UI |
| --- | --- | --- |
| `ids` | Never: a component id is the Weft id, made unique with a number where one kind produced two components. | A component id that is not a valid Weft id is replaced by a generated one. |
| `bindings` | A binding on a prop the component has no data-bound property for (`disabled` other than on a button, `selected`, `open`, `error`), and one that reads a whole item or an outer repetition. | A path that is not a Weft data path; a function call (`formatString`, `length`, …) used as a value. |
| `actions` | An event other than `press`, `submit` and a dialog's open (`on-change`, `on-close`); a button with no action gets its id as the event; a form whose submit button is missing. | An event name that is not an action name, an event `context`, a local function call other than `openUrl`. |
| `tokens` | Every token reference (`gap`, `material`, …): A2UI themes are set on `createSurface`, not per element. | The `theme` of `createSurface`. |
| `layout` | `columns`, `wrap`, `padding`, `max-width` and `min-column-width`. | `justify` of `spaceAround`, `spaceEvenly` or `stretch`; unequal weights; a negative or non-numeric weight; a weight on a component that is not a child of a `Row` or `Column`; a horizontal `List`. |
| `repetition` | Options that repeat over data (a `ChoicePicker`'s options are literal). | The `path` of a template is not a Weft data path. |
| `slots` | A slot the mapping above does not use is left out. | None: A2UI has no slots. |
| `hidden` | `hidden="true"` elements are not written; a bound `hidden` is always shown. | None: A2UI has no hidden state. |
| `props` | Props with no A2UI property (`state`, `tone`, `sort`, `placeholder`, `step`, `ordered`, `min` and `max` of a stepper, the tilts, `modal`, …), a field `type` other than text, email, password, number and multiline, and a `disabled` on anything but a button. | `accessibility.description`, `fit`, `validationRegexp`, a `ChoicePicker` that allows several values (one is kept), a check with no Weft prop. |
| `values` | None. | `updateDataModel` messages (a document holds no data); a selected value beyond the first. |
| `names` | None. | A required `label` missing from the input is set to `""`. |
| `kinds` | `switch`, `stepper` and `color-picker` (written as `CheckBox` and `TextField`), and any kind without a component, such as `model` (A2UI has no 3D or model component, so its paths and still are lost as `props`), which is dropped with its content kept. | `Icon`, `Video`, `AudioPlayer`, `Divider` and any component of another catalog are dropped; a `Card` with no label is a `stack`. |
| `text` | Text with no place in the content model is dropped. | Likewise. |
| `structure` | A `dialog` is a `Modal` with a generated trigger; a `tabs` with no tab, which A2UI refuses. | Other surfaces than the first, a component that is used but not defined, a `Button` whose child is not a `Text`. |

- **To json-render:** a document compiles to one json-render spec (`vercel-labs/json-render` at commit `fc2a696`, 2026-10-01, `@json-render/core` 0.21.0): `{ "root", "elements" }`, plus `"state"` when the caller gives sample data (a JSON object), written as it is. The catalog the spec is checked against is the Weft catalog itself, so the mapping is one element per element: the key and the `type` are the Weft id and kind, and `props` holds the Weft props in canonical JSON (§3; a token reference stays `{ "token": … }`), except that a binding is a json-render expression: `{ "$state": "/user/email" }` for a path from `$.`, `{ "$item": "title" }` for a field of the innermost loop variable (`""` for the whole item), `$bindState` and `$bindItem` instead on a prop the catalog marks `writable`, and a negated binding `{ "$cond": { …, "not": true }, "$then": true, "$else": false }`. `hidden` is the element's `visible`: `true` is `false`, a binding is a condition on the same path with `not` inverted. The text content of a `text` or `mixed` component that holds text only is its `text` prop; in a `mixed` component that also holds elements, each text run is a generated `text` element. Named slots are `slots`, the default slot `children`. An event is `on: { <event>: { "action": <action>, "params": { "id": <element id> } } }`, inside a repetition with `"item": { "$item": "" }` as well, which json-render resolves to the item's absolute path: what a Weft host receives with an action (§2.2). An `<each>` is an element of type `each` (a component of the exported catalog that renders its children and adds no element of its own) with props `{ "as": … }`, `repeat: { "statePath": … }` (the pointer of `in`, `{ "$item": … }` inside another repetition) and its content as `children`. A literal `href` or `src` is written only when it is `http`, `https`, `mailto` or relative (Trust, above); any other literal is left out. A binding of either prop is still an expression.
- **From json-render:** a spec is parsed, never run. The elements are read from `root`, each at most once; a `type` that names a kind of the catalog is that kind, with its props, `visible`, `on`, `children` and `slots` read back by the inverse of the mapping above. An element of type `each` with `repeat` is an `<each>` with its id and `visible`; an element with `repeat` whose type is not `each` (json-render's own way of repeating a container's children) holds an `<each>` around its children. The loop variable of a repetition is its `as` prop, else the first of `item`, `item2`, … that no outer repetition uses. A literal `label` is the accessible name and a literal `text` the content, through the builder every importer shares, so ids, required props and content rules hold as for the other importers. The input is bounded as the other importers are (at most 2,000,000 bytes, 200 levels and 20,000 elements; `W602`); input that is not JSON, or has no `root` naming an element, is `W601`. The result is `{ document, losses, diagnostics }`, and a document exported with no losses imports back to the same document, except that a literal `text` prop comes back as content, which json-render cannot tell apart from it. A literal `href` or `src` is kept only when it is `http`, `https`, `mailto` or relative; any other literal is left out.

Losses of the json-render conversions (`vercel-labs/json-render` at commit `fc2a696`, 2026-10-01; `@json-render/core` 0.21.0). Ids, kinds, token references, slots, events, `hidden` and repetitions keep their meaning on export, and so does every prop except a literal `href` or `src` that is not `http`, `https`, `mailto` or relative; only these change:

| Loss kind | To json-render | From json-render |
| --- | --- | --- |
| `ids` | None: a generated text element takes a free id. | An element key that is not a Weft id; a generated id stands in. |
| `bindings` | A binding that reads an outer repetition, or a whole outer item: json-render reads only the innermost item. The prop is left out (for `hidden`, the element is always shown). | A pointer that is not a Weft data path; a binding on a prop the kind cannot bind. |
| `actions` | None. | A second action bound to one event, an event the kind does not declare, an action name that is not a Weft action name, `params` other than `id` and `item`, `confirm`, `onSuccess`, `onError`, `preventDefault`, `watch`, and any event of an `each` element. |
| `tokens` | None: a token reference is written as it is. | A token reference on a prop that takes no token, or a path that is not a token path. |
| `hidden` | None. | A `visible` condition other than truthiness of one path (`eq`, `gt`, …, `$and`, `$or`, `$index`); the element is always shown. |
| `values` | None. | `state` (a document holds no data); a list value; `$template`, `$computed`, `$index` and any other `$cond`. |
| `props` | A literal `href` or `src` that is not `http`, `https`, `mailto` or relative; it is left out. | A prop the kind does not declare, a value it does not take, or a literal `href` or `src` that is not `http`, `https`, `mailto` or relative. |
| `kinds` | None. | A `type` that is not a kind of the catalog; its children are kept in its place. |
| `repetition` | None. | A `repeat.statePath` that is not a Weft data path; `repeat.key`; an `as` that is not a loop variable name or is already an outer one; an `each` element without `repeat`, whose children are kept in its place. |
| `slots` | None. | A slot the kind does not have; its elements are placed as content. |
| `names` | None. | A required `label` missing from the input is set to `""`. |
| `text` | A text run beside elements in a `mixed` component (or in an extension element) becomes a generated `text` element with a free id. | None. |
| `structure` | None. | An element that is used but not defined, or used a second time. |

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
| `tokens` | array of file names, at most 64; or one file name | DTCG token files, in layer order (§10.3), or one DTCG resolver document that orders the sets and modifiers itself (§10.3). More than 64 is `W701`. Project content (§10.1) gives the resolver document itself. |
| `catalog` | file name; or an array of at most 32 entries, each a file name or `{ "package": "<npm name>" }` | Catalogs merged over the core catalog (§10.4). One file name is an array of one. More than 32 is `W701`. A package entry names an installed npm package (below). Project content (§10.1) gives a catalog, or an array of catalogs. |
| `actions` | array of action names | The host's actions: `on-*` values are checked against them (`W308`). |
| `data` | file name | A JSON Schema of the host data model (§10.5). |
| `fragments` | object of name → file name | Fragments the screens place with `<use>` (§10.7). |
| `$schema` | string | Ignored; for editors, which can point it at `schemas/weft.schema.json` (§10.6). |
| `validate`, `format`, `explain`, `render`, `export`, `import`, `mcp`, `plugins` | objects | Tool settings (§10.6). |

- Every member is optional. Without `catalog` the project's catalog is the core catalog.
- On the command line `--catalog` may be repeated. The files it names replace the project's whole `catalog` list, in their order, and merge over the core catalog as §10.4 says; their diagnostics point into each file (`acme-ui.catalog.json:#/name`), and an error stops the command. A file whose catalog is named `weft-core` replaces the core catalog as the base; alone, it is used as it is.
- A file name is relative to the directory of the project file, uses `/` as separator and stays inside that directory: it is non-empty and has no empty or `..` segment, no leading `/`, no `\`, no `:` and no NUL. Any other name is `W703` and the file is not read.
- After the name is accepted, the file is read only if its resolved path (symbolic links followed) stays inside the project directory; otherwise it is `W704` and is not read.
- A package entry is the one exception to reading inside the project directory. Its `package` is an npm package name (at most 214 characters; lowercase letters, digits, `-`, `.`, `_` and `~`, not starting with `.` or `_`; optionally scoped as `@scope/name`); any other value is `W703`. The package is the directory `node_modules/<name>` in the project directory or, failing that, in the nearest parent directory that has it, as Node resolves packages. Only two files are read: the package's `package.json`, and the file its `weft.catalog` member names, a file name with the rules above relative to the package directory (`W703` otherwise), whose resolved path stays inside the package directory. A package that is not found, a `package.json` that cannot be read or has no string `weft.catalog`, and a catalog file that cannot be read or is not JSON are `W704`, at `#/catalog/<i>/package`. Nothing is installed, fetched or run. The catalog's `source` is the package and the version its `package.json` gives, `<name>@<version>` (`<name>` without a version). Only tools that read files from disk (the `weft` command and the Node tools) look packages up; project content gives catalogs themselves, so there an object entry is a catalog.
- Loading never stops at a problem. Each problem is a diagnostic and the rest of the project still applies: a project file that is not JSON or not an object is `W701` and the project is empty; a member of the wrong type is `W701` and is ignored, and so is an entry of `tokens` or `actions` of the wrong type; an unknown member is `W702`, a warning in both modes so that an older tool still reads a newer file; an action name that breaks the action grammar (§2.2) is `W708` and is left out; a file that cannot be read or is not JSON is `W704` and is left out.
- Project diagnostics point into the project file with a JSON Pointer prefixed with `#`, e.g. `#/tokens/1`; for project content passed as a tool argument the pointer starts at that argument (`#/project/tokens/1`). A pointer continues into a named file as if its content stood in the project file: `#/catalog/components/rating`, `#/catalog/1/components/acme-chip`, `#/data/properties/user/type`. A problem of the merged token tree points at `#/tokens`. Diagnostics of a screen keep the paths of §6.1.

### 10.3 Token layers and the resolver

- Each file is a DTCG 2025.10 token tree. The files merge in order into one tree, which then loads as one token file. Groups merge member by member; a token (an object with `$value`) replaces whatever the earlier files have at its path, as a whole; any other clash is won by the later file. So a later file overrides a token by declaring it again, and a `$type` an earlier file sets on a group still applies to tokens a later file adds to that group.
- **Material tokens.** DTCG 2025.10 has no material type, so a material is a `color` token (the tint, whose `alpha` is the material's opacity) with the vendor extension `dev.weft.material` (`$extensions`, which tools must preserve): `{ "blur": { "value": 20, "unit": "px" } }`, the background blur as a dimension in px or rem. Every other DTCG tool reads the file as an ordinary colour and keeps the blur. Weft loads the token as the kind `material` with the value `{ tint, blur }`; the prop's `tokenType` is `material`, so `W306` and `W307` check a reference as for any other token. The file is untrusted: an `alpha` outside 0 to 1, a blur that is not a dimension in px or rem or is outside 0 to 100 px, a tint that is not a colour, or no `blur` is `W705`, and the token is left out. Modes (below) work as for a colour: a context may override the whole token, so its dark value has its own tint and blur.
- Aliases resolve after the merge, as the DTCG resolver module orders its sets: an alias in the base file to a token the brand file overrides reads the brand value.
- A file that is not a JSON object, and every problem of the merged tree (a bad name, a token without a type, an alias to nothing, an alias cycle), is `W705`. The token in question is left out.

**Resolver.** When `tokens` is one file name, that file is a resolver document of the DTCG Resolver Module 2025.10 (by convention `*.resolver.json`): `version` `"2025.10"`, `sets` of token sources, `modifiers` whose `contexts` each list token sources, and a `resolutionOrder` of set and modifier entries, each a `$ref` to `#/sets/<name>` or `#/modifiers/<name>` or an inline entry with `type` and `name`. Sources are token trees or `$ref`s to a token file, a JSON Pointer inside one (`file.json#/brand`), or a set; members beside a `$ref` replace the referenced ones shallowly. Weft reads it so:

- **Contexts.** The tree of each input merges in `resolutionOrder` as the token files of a list do, and aliases resolve after the merge, so an alias in a set reads the value of the active context. The *default input* picks, for every modifier, its `default`, or else its first context in document order; that tree is the project's token set, used for validation and by every tool that knows one context. The project also keeps, for each modifier, the tree of every one of its contexts with the other modifiers at their defaults; generators that support modes read those.
- **Appearance.** The first modifier in `resolutionOrder` that has contexts named `light` and `dark` (compared without case) is the project's appearance: generators map its `light` and `dark` contexts to the platform's light and dark appearance (§9). Without one, generators emit one appearance as before.
- **Design tools.** A Figma or Penpot build may be given one modifier (the plugin request's `modifier`: its name, default and every context's tokens, at most 64 contexts). In Figma each context is a mode of the token collection, the default context its default mode; in Penpot the modifier is a token theme group and each context a theme that turns on the base token set and, for a context other than the default, a set of the tokens that differ, and the default context is recorded as shared plugin data on the library because a theme has none. A context the file cannot hold (a Figma plan that limits modes) is skipped and reported in the reply's `notes`, never an error. An export reads the modes back into a resolver document in the reply's `resolver` field: a set with the default context's tokens and the modifier with, per context, the tokens that differ, so loading it gives each context the values the file holds. A value the file holds that is an alias or does not parse is left out.
- **Files.** File references are relative to the directory of the resolver, follow the file name rules of §10.2 after `..` steps are taken inside that directory, and are never URLs: any other reference is `W703`. A file that cannot be read, or any file reference in project content, is `W704`. At most 64 files and 64 contexts per modifier are read.
- **Problems.** Every rule of the module is checked and every breach is `W705` with a pointer into the resolver (`#/tokens/modifiers/theme/default`), and the rest still loads: a missing or other `version`; an entry without `type` or `name`, or a repeated name; a `$ref` to nothing, into `resolutionOrder`, or to a modifier from anywhere but `resolutionOrder`; a reference cycle; a modifier with no context (left out) or one context (kept); a `default` that names no context (the first context applies). A problem of one context's tree is `W705` with the message prefixed by `<modifier>=<context>: `, and a token whose type differs between contexts is `W705`.
- A token tree can hold no array, so a document whose `resolutionOrder` is an array is a resolver and anything else is a token file; tools that take one token file argument (`--tokens`) accept a resolver by that test.

### 10.4 Catalog extension

Each `catalog` file is a catalog (§5): `weft`, `name`, `version` and `components`, and optionally `prefix`, `requires` and, for a library, `fragments`. A catalog with a `prefix` is a *library*; the one catalog without a prefix is the *project catalog*. Screens never say which catalog a kind comes from: the kind's name is enough.

- **Claims.** In list order, a catalog whose `name` is the core's or an earlier catalog's, or whose `prefix` an earlier library declares, is `W711` and is ignored. A `prefix` that breaks the rules of §5 is `W712`, and so is a second catalog without a prefix; that catalog is ignored.
- **Requirements.** A `requires` entry that names no loaded catalog, or one loaded at a version its rule does not admit, is `W714`, a warning; the catalog still loads.
- **Merge.** The project's catalog starts from the core catalog. The libraries merge next, in list order, and the project catalog last, wherever it is listed, so the order changes listings and diagnostics but never the result. The merged catalog takes the project catalog's `weft`, `name` and `version`, or else the last library's.
- **Libraries** define only new kinds named `<prefix>-…`; an entry for any other kind, including a core kind or another library's, is `W713` and is ignored. A kind already defined by another catalog is `W711` and is ignored.
- **The project catalog** may define new kinds outside every loaded library's prefix (a new kind under one is `W713` and is ignored), and extend any kind already merged, core or library.
- A new kind needs a whole definition (`description`, `role`, `content`, …). Its name follows the name grammar, is not a structural element (`context`, `each`, `entry`, `slot`, `use`, `fragment`, `param` or `outlet`), and does not start with `x-`, because `x-` elements are opaque to every catalog (§8).
- An extended kind's entry may leave out `description`, `role` and `content` to keep the ones it extends. `props` and `slots` merge by name, an entry replacing the definition of that name (to add a variant, restate the prop with the longer `values` list). `states`, `events`, `allowedChildren` and `allowedParents` are joined: the existing values, then the new ones. Every other field replaces the existing one.
- An extension may only widen. The merged definition is compared with the definition it extends (the core's, or the library's) by the rules of §8: a kind whose merged definition makes a change those rules call major (a changed role or type, a new required prop, a narrowed content model, …) is `W707`, and that kind keeps the definition it had.
- A catalog that is not a catalog, including a `prefix` that is not a string or a `requires` that is not an object of versions, is `W706` and is ignored; an entry that is not a valid definition, or names a kind that breaks the rules above, is `W706` and only that entry is ignored.
- `null` is not a value in a catalog: a member written `null` makes its entry invalid.
- **Library fragments.** A library's `fragments` maps at most 256 fragment names to fragment files (§10.7), each relative to the directory of the catalog file under the rules of §10.2; for a library found as a package, the fragment files are read inside the package directory under the same rules as its catalog file, and nothing outside it. In project content (§10.1) a value is the fragment's markup. A `fragments` member on the project catalog, one that is not an object or one with more entries is `W706`, and the catalog is ignored. An entry with a name that breaks the name grammar or a value that is not a string is `W706`, a bad file name `W703`, a file that cannot be read `W704`; that fragment is left out.
- **Fragment ownership.** A library's fragments are named `<prefix>-…`, and a fragment in `weft.json` is named under no loaded library's prefix. Any other name is `W715`, and that fragment is left out. So one fragment name has one owner.
- **What a library fragment may use.** It is written once for projects its author does not know, so it is checked against what the library declares: the core kinds, the library's own kinds and the kinds of the catalogs in its `requires`, as they define them (the project catalog's widenings do not count); its own fragments and those of the libraries in its `requires`. Token references are checked against the consuming project's tokens (`W306`). It reaches the host only through its parameters: a binding that reads `$.…`, or an `on-*` value that names an action instead of reading an action parameter, is `W716`. Its problems point into the catalog: `#/catalog/0/fragments/acme-promo/fragment/stack#row/@gap`.
- The libraries' fragments join the merged catalog's `fragments` before the project's own, so screens and the project's fragments place them with `<use>` like any other. A library's `version` covers its fragments (§8).
- A loader records, for each kind of the merged catalog, the catalog that defined it and the catalogs that extended it, and the list of loaded catalogs with their `name`, `version`, `prefix` and source file.

### 10.5 Data schema

The `data` file is a JSON Schema (2020-12) of the host data model. Validation checks every binding against it: the path must be declared, and the data there must have a type the attribute takes.

- **Subset.** Weft reads `type` (a type name or a list of them), `properties`, `additionalProperties` and `items` (one schema), and the boolean schemas `true` (any data) and `false` (no data). Keywords that only annotate or constrain values (`$schema`, `$id`, `$comment`, `$defs`, `title`, `description`, `default`, `examples`, `required`, `enum`, `const`, `format`, `minimum`, …) are ignored. Keywords that combine or reference schemas (`$ref`, `$dynamicRef`, `allOf`, `anyOf`, `oneOf`, `not`, `if`, `then`, `else`, `dependentSchemas`, `prefixItems`, `contains`, `patternProperties`, `propertyNames`, `unevaluatedItems`, `unevaluatedProperties`) are not supported: `W710`, and the schema they sit in accepts any data. A schema that is not an object or a boolean, a `type` that names no JSON Schema type, `properties` that is not an object, or nesting deeper than 256 schemas, is `W709`, and that schema accepts any data.
- **Closed objects.** One rule differs from JSON Schema: a schema with `properties` and without `additionalProperties` declares every property the object has, as if `additionalProperties` were `false`. An object with open keys (a map) says so with `"additionalProperties": true` or a schema for its values. A schema with neither keyword declares any name.
- **Paths.** A path from `$.` starts at the root schema; a path from `$item` starts at the `items` schema of the `in` of the `<each>` that names `item`. A name segment steps into `properties`, then `additionalProperties`; an index segment steps into `items` when the schema may be an array. A step the schema does not declare, into `false`, or into a value that is neither an object nor an array, is `W315`, with the nearest declared name as the hint. A schema without `type` may be anything, so every step from it is declared.
- **Types.** The schema at the end of a plain binding must allow a type the attribute takes: a `string` prop (`text`, `label` and `state` included) takes `string`, `number` and `integer`, because text shows numbers; a `number` prop takes `number` and `integer`; a `boolean` prop (and `hidden`) takes `boolean`; an `enum` or `token` prop takes `string`; the `in` of `<each>` takes `array`. Otherwise it is `W316`. A schema without `type` allows every type. A negated binding tests whether a value is empty and so takes any type; an attribute the catalog does not declare is not type-checked.

### 10.6 Tool settings

Everything a Weft tool lets its user configure can also be set in the project file, so every screen of a project is checked, formatted, rendered and converted the same way. Settings sit in one section per tool; every section and every key is optional.

Precedence: an argument given to a tool (a command-line flag, a tool argument) overrides the project file, which overrides the tool's default. A project passed to an MCP tool as an argument brings its resources but never changes the server's own settings (`mcp`).

| Key | Type | Default | Used by |
| --- | --- | --- | --- |
| `validate.mode` | `"strict"` or `"lenient"` | `"lenient"` | `weft validate`; the default of `weft_validate`'s `strict` when the MCP server is started with the project. Writers' tools (`weft_patch`, render, export) always check strictly. |
| `format.write` | boolean | `false` | `weft fmt` rewrites the file in place instead of printing it. The canonical form itself has no options (§3). |
| `explain.context` | boolean | `false` | `weft explain` also lists each element's context entries after its readbacks, as `--context` does (§2.3). `--against` always lists context changes. |
| `render.data` | file name | no data | Sample data the bindings read when a screen is rendered to a static page. |
| `render.tokens` | array of file names, or one file name | the project's `tokens` | Token files to render with, layered as in §10.3, or one resolver document (§10.3). |
| `render.outDir` | file name | next to the screen | Where rendered pages go. |
| `render.appearance` | `"light"` or `"dark"` | the default context | Which context of the resolver's light and dark modifier (§10.3) the reference renderer draws with, which the page declares as its `color-scheme`; `--appearance` overrides it. Without an appearance in the tokens it changes nothing. |
| `export.html.outDir` | file name | standard output | Where `weft html` writes `<screen>.html`. |
| `export.html.source` | boolean | `false` | The page keeps the canonical screen in a leading comment, so `weft import-html` gives it back exactly (§9). |
| `export.html.data`, `export.swiftui.data` | file name | no data | Sample data (JSON) the generated page shows, or the SwiftUI model's `sample` is built from (§9). Without it the page is a template for a host to fill and the preview shows the model's defaults. |
| `export.react.outDir` | file name | next to the screen | Where generated React components go (`weft react` prints when absent). |
| `export.react.typescript`, `export.solid.typescript` | boolean | `false` | Write TSX with typed props instead of JSX. |
| `export.react.source`, `export.solid.source` | boolean | `false` | The component keeps the canonical screen in a leading comment, so `weft import-react`/`import-solid` give it back exactly (§9). |
| `export.react.context`, `export.solid.context`, `export.html.context`, `export.slint.context`, `export.swiftui.context` | `"keep"` or `"strip"` | `"strip"` for `html`, `"keep"` for the others | Whether the source comment (§9, Provenance) carries the screen's context (§2.3). `--context` overrides it. Output without a source comment carries no context either way. |
| `export.solid.outDir` | file name | standard output | Where `weft solid` writes `<screen>.jsx` or `.tsx`. |
| `export.lit.outDir` | file name | standard output | Where `weft lit` writes `<screen>.js`. |
| `import.html.outDir` | file name | next to the page | Where screens imported from HTML go (`weft import-html` prints when absent). |
| `import.react.outDir`, `import.solid.outDir` | file name | standard output | Where `weft import-react` and `weft import-solid` write `<file>.weft`. |
| `export.swiftui.outDir` | file name | standard output | Where `weft swiftui` writes `<screen>.swift` and `weft swiftui-tokens` writes `WeftTokens.swift`, and where the Xcode command plugin's `export` does (next to the screen when absent). The build tool plugin ignores it: it writes into the build folder. The target (iOS 17, macOS 14) is fixed: `@Observable` needs it. |
| `export.css.outDir` | file name | standard output | Where `weft css-tokens` writes `weft-tokens.css`, the token stylesheet of §9 that React, SolidJS and Lit components read, and `weft css-base` writes `weft-base.css`, their base stylesheet. |
| `export.swiftui.sharedTokens` | boolean | `true` | Screens read the tokens from the shared `WeftTokens.swift` instead of each carrying a theme struct with the tokens it uses (§9). A screen generated without a project carries its own. |
| `import.swiftui.outDir` | file name | standard output | Where `weft import-swiftui` writes `<file>.weft`, and where the Xcode command plugin's `import` does (next to the view when absent). |
| `export.a2ui.outDir`, `import.a2ui.outDir` | file name | standard output | Where `weft a2ui` writes `<screen>.a2ui.json` and `weft import-a2ui` writes `<file>.weft` (§9). |
| `export.slint.outDir`, `import.slint.outDir` | file name | standard output | Where `weft slint` writes `<screen>.slint` and `weft import-slint` writes `<file>.weft` (§9). |
| `import.react.context`, `import.solid.context`, `import.html.context`, `import.slint.context`, `import.swiftui.context` | `"keep"` or `"drop"` | `"keep"` | Whether context read back from a generated file's source comment enters the document. `drop` suits files from outside the team. `--context` overrides it. |
| `export.schema.outDir` | file name | standard output | Where `weft schema` writes `document.schema.json`, the JSON Schema of the canonical documents the project's catalog admits (§3.1). |
| `import.cem.outDir` | file name | standard output | Where `weft import-cem` writes `<file>.catalog.json`, the catalog imported from a Custom Elements Manifest (§9). |
| `import.cem.name`, `import.cem.version` | non-empty string | the manifest's file name without its extension; `"0.0.0"` | The `name` and `version` of the catalog `weft import-cem` writes. |
| `import.cem.prefix` | one lowercase name segment | none | The `prefix` of the library catalog `weft import-cem` writes (§9); without it the catalog is a project catalog. |
| `import.figma.outDir` | file name | the working directory | Where the `figma-pull` script of the plugins writes `<screen id>.weft`, a frame read through the Figma REST API. |
| `import.figma.context` | `"keep"` or `"drop"` | `"keep"` | Whether the context the Figma plugin kept in the frame's plugin data (§9) enters the screen `figma-pull` writes. `drop` suits files from outside the team. |
| `mcp.limits.markupChars`, `dataChars`, `patchesChars`, `projectChars` | whole number ≥ 1 | 200,000; 200,000; 200,000; 500,000 | Bounds, in UTF-16 code units, on the arguments of one MCP call. |
| `mcp.limits.patches`, `diagnostics`, `inputElements` | whole number ≥ 1 | 100; 40; 20,000 | Patches per call, diagnostics listed per result, JSON values per call. |
| `mcp.context` | `"read-write"` or `"read-only"` | `"read-write"` | Whether `weft_patch` may change a screen's context (§2.3, §7): `"read-only"` refuses every context patch with `W512`; `weft_context` still reads it. |
| `plugins.open-design.tokensDir` | file name | next to the design system | Where the Open Design plugin's `design-md` script writes the tokens it maps from a `DESIGN.md` or `tokens.css`. |
| `plugins.<name>` | object | — | Settings of a plugin or tool Weft does not know, under its own name. Weft checks only that each is an object. |

- File names follow §10.2 and are relative to the project file; a directory name has no trailing `/`.
- A setting of the wrong type is `W701` and the default applies; a bad file name is `W703`; an unknown key in a section is `W702`, a warning. Tools read only the settings that passed these checks.
- `schemas/weft.schema.json` is the JSON Schema (2020-12) of the project file. It is generated from the same table the loader checks against, so the two cannot disagree; editors that follow `$schema` complete and check every key.
- **Adding a setting.** A tool option that a user can set gets a key here in the same change: a row in this table, an entry in the loader's table (`crates/weft-catalog/src/settings.rs`), which regenerates the schema, and the tool reading it with the precedence above. A new export or import target adds its section under `export.<target>` or `import.<target>`. The names reserved for targets in progress are `export.figma` (T14, where a rem base and the token strategy belong), `export.penpot` and `import.penpot`. Until a target's section lands, its key is unknown and warns.


### 10.7 Fragments

A fragment is a block that several screens of a project share, such as a page header, a footer or a product card. It is kept once, in its own file, and screens place it by reference.

A fragment adds elements to the format, not a new model, and it is an addition of format 0.2 (§8).

**Where fragments live.** The project file maps fragment names to files:

```json
{ "fragments": { "page-header": "fragments/page-header.weft" } }
```

- A key is the fragment's name and follows the name grammar of §2. The name lives only here. A value is a file name under the rules of §10.2. In project content (§10.1) a value is the fragment's markup as a string.
- A `fragments` member that is not an object, a key that breaks the name grammar, or a value that is not a string is `W701`. A bad file name is `W703`, and a file that cannot be read is `W704`. A file whose root is not `<fragment>` is `W201`. In each of these cases the fragment is left out.
- Each fragment is parsed against the merged catalog, with the project's tokens and actions and with the other fragments. Its markup problems keep their codes and their positions in its file. Their path starts at `#/fragments/<name>`, as in `#/fragments/page-header/fragment/stack#bar`.
- The project's merged catalog (§10.4) carries the fragments in a `fragments` member, which maps each name to its fragment document in canonical JSON. So every tool that is given the project's catalog knows the fragments. A library may ship fragments too (§10.4); a project catalog file that has `fragments` is `W706`. A screen's own inline fragments (below) do not need a project.

**A fragment file.**

```xml
<fragment label="Page header" version="1.0.0" weft="0.3">
  <param name="title" required="true" type="string"/>
  <param default="muted" name="tone" type="enum" values="default muted"/>
  <param name="back" type="action"/>
  <param allowed-children="button link" name="actions" type="slot"/>
  <stack id="bar" direction="row">
    <button id="back" on-press="{$back}">Back</button>
    <heading id="title" level="1" text="{$title}"/>
    <text id="note" tone="{$tone}">Signed in</text>
    <outlet name="actions"/>
  </stack>
</fragment>
```

- The root is `<fragment>`. It takes `weft`, an optional `version` (§3, `W230`) and an optional `label`, a description for tools rather than an accessible name. It has no `id`. A change of `label` alone is a patch (§8).
- The `<param>` elements come first, one per parameter. A parameter has the vocabulary of a catalog prop (§5): `name`, which matches `[a-z][a-z0-9]*` (so that it is both an attribute name and a loop-variable name) and is not `fragment` or `id`; and `type`, which is one of `string`, `number`, `boolean`, `enum`, `token`, `action` or `slot`. As the type allows, it also takes `values` (space-separated, for `enum` only), `token-type` (for `token`), `min`, `max` and `integer` (for `number`), `required`, and `default` (for value types, not together with `required`). A `slot` parameter takes `required`, `allowed-children` (space-separated kinds) and `content`, whose only value is `nodes` because a slot holds elements. `required` and `integer` are booleans, `min` and `max` are numbers, and `default` is a value of the parameter's type: a literal, or a token reference for a `token` parameter.
- Without a variant parameter, the body comes after the parameters. It holds one or more elements, as they could stand inside a `<screen>`.
- `<outlet name="…"/>` marks where the content of a `slot` parameter goes. It is structural, like `<slot>` and `<each>`: it has no id, no other attribute and no content.
- A `<param>` of type `enum` may take `variant="true"`. A fragment has at most one. `variant` is a boolean. A fragment with a variant parameter has, after its parameters, only `<variant when="…">` elements. `when` lists one or more of that parameter's values, separated by spaces. Each value is covered by exactly one `<variant>`. A `<variant>` holds one or more elements, outlets included. It is structural: no id and no attribute but `when`. The same rules apply to an inline fragment when the reader has that form; this reader applies them to a fragment file.
- `variant="true"` on a parameter that is not an enum or on a second parameter, a `<variant>` without a variant parameter or outside a fragment, a `when` value the enum does not have or that two variants cover, an enum value no variant covers, a body element beside the variants, or another attribute on `<variant>`, is `W809`.
- Adding a value of the variant parameter widens the enum (minor), removing one narrows it (major), and changing a variant's body is a patch. That classification belongs to the document-version check. This reader does not implement a second classifier.
- The body reads a value parameter with the loop-variable form of a binding, `{$title}`, in any prop value. It reads a boolean parameter negated as `{!$busy}`. A read names the parameter and nothing after it. An action parameter is read as the whole value of an `on-*` attribute, `on-press="{$back}"`. This is the one place where an `on-*` value is braced. Bindings of `$.…` in the body read the host data model of the screen that uses the fragment.

In canonical JSON a fragment file is a `Document` whose root has the kind `fragment`. `param`, `outlet` and `variant` are nodes without an id, and their attributes sit in `props`, typed as above. A variant's `when` is that string. A parameter read is an ordinary binding value, `{ "bind": "$title" }`. An action parameter in `on` is the string `"{$back}"`. The rules of §3 apply unchanged, and the `<param>` elements keep their document order before the variants or the body.

**A use.**

```xml
<screen id="cart" label="Cart" weft="0.3">
  <use id="header" fragment="page-header" title="Your cart" on-back="nav.back">
    <slot name="actions">
      <button id="clear" on-press="cart.clear">Clear</button>
    </slot>
  </use>
</screen>
```

```json
{
  "kind": "use",
  "id": "header",
  "props": { "fragment": "page-header", "title": "Your cart" },
  "on": { "back": "nav.back" },
  "slots": { "actions": [{ "kind": "button", "id": "clear", "on": { "press": "cart.clear" }, "children": ["Clear"] }] }
}
```

- `<use>` is structural. It takes `id` (required, as on `<each>`), `fragment` (the fragment's name, a literal), one attribute per value parameter, `on-<name>` per action parameter, and one `<slot name="…">` per slot parameter. It has no default content and none of the universal attributes of §2.2: a header that can be hidden declares a `hidden` boolean parameter and passes it on. Extension attributes (`x-…`) are allowed.
- A value parameter takes every value form of §2.1: a literal typed by the parameter, a binding, a negated binding, or a token reference. A binding is read where the `<use>` stands, so `{$line.name}` works inside an `<each as="line">`.
- The variant parameter takes a literal. The fragment's signature declares it with `bindable: false`, so a binding is `W217`. Without the attribute the parameter's `default` applies, or the use is `W205` when the parameter is `required`. The body may also read the parameter as a value, `{$emphasis}`.
- The canonical form keeps a `<use>` as it is written. Expansion (below) is something a renderer or a checker does, never a rewrite of the document.

**Ids.**

- Ids inside a fragment body are local. They are unique within the fragment (`W301` there) and may repeat the ids of any screen. Ids are local to each variant: the same id may appear in several variants and means the same part, so an instance path stays valid when the variant changes. Within one variant they are still unique.
- The id of a `<use>` is a screen id like any other.
- An expanded element is addressed by its *instance path*: the id of the `<use>` and the local id, joined by `/`, as in `header/title`. Nested uses join further: `header/crumbs/home`. `/` is outside the id grammar, so an instance path never collides with a document id. Inside `<each>` the index suffix of §4.3 follows the whole instance path: `card/price[2]`.
- Renderers write the instance path to `data-weft-id`.

**Meaning: expansion.** A use means its fragment's body, placed where the `<use>` stands. When the fragment has a variant parameter, that body is the `<variant>` whose `when` contains the use's literal, or the parameter's `default` when the attribute is absent. A use that does not resolve to one variant expands to nothing. Expansion then:

1. every parameter read replaced by the use's value. A negated read of a binding toggles its negation, and a negated read of a boolean literal is its opposite. An absent optional parameter takes its `default`; without one, the attribute is left out;
2. every `<outlet name="x">` replaced by the content of the use's `<slot name="x">`, or by nothing;
3. every id replaced by its instance path.

Evaluation is scoped, not textual. The loop variables of the body never capture names from the use site, and the reverse. An implementation that expands by rewriting renames the body's loop variables that would clash.

`<use>` is transparent for structure, as `<each>` is. The top-level elements of the chosen body are checked against the parent's (or slot's) `allowedChildren` and against their own `allowedParents` where the `<use>` stands (`W302`, `W303`, `W304`). `<use>` itself adds no node to the accessibility tree.

A fragment file is as untrusted as a screen, so expansion is bounded. A fragment may use other fragments, but a cycle (`a` uses `b`, which uses `a`) is `W805`, and nothing in it expands. A screen whose expansion holds more than 10,000 elements, or nests deeper than the 256 levels of §2 (a `<use>` counts as a level), is `W806`, and its expansion keeps only the root. Both are reported at the outermost `<use>` that leads to them.

**Validation.** A fragment is checked once on its own, and each use is checked against the fragment's parameters. Expanded content is not checked again element by element.

A fragment is checked as a screen body, with the project's catalog, tokens, actions and data schema. Each variant body is checked on its own, the same way. An outlet may appear in some variants and not in others; a variant without the outlet of a slot parameter drops that slot's content, and `weft explain` says which variant a use chose and when a slot is dropped. Each parameter read counts as a value of the parameter's type. A `string` prop takes `string` and `enum` parameters, a `number` prop takes `number` parameters, a `boolean` prop takes `boolean` parameters, an `enum` prop takes an `enum` parameter whose values it allows (`W203`), and a `token` prop takes a `token` parameter with the same `token-type` (`W307`). Any other pairing is `W204`. A literal-only prop takes no read (`W217`), and a negated read needs a boolean parameter in a boolean prop (`W218`). Reads of `$.…` are checked against the data schema (`W315`, `W316`). A `submit` button in the body may rely on a `form` around the use, so `W313` is not reported there, and `grow` on a top-level element is checked where the fragment is used.

At a use:

| Situation | Code |
| --- | --- |
| `fragment` names no inline fragment of the screen and no fragment of the project | `W801` (mode severity; the hint is the nearest name) |
| An attribute, `on-*` or slot that the fragment does not declare | `W802` |
| A required parameter or slot is missing | `W205`, `W208` |
| A value is not of the parameter's type, enum values or range | `W204`, `W203`, `W224` |
| A binding or token problem in a value | the codes of §6, such as `W214`, `W306`, `W315` |
| A top-level body element is not allowed where the `<use>` stands | `W302`, `W303`, `W304` |
| Slot content against the slot parameter's `allowed-children` | `W302`, `W304` |

In a fragment:

| Situation | Code |
| --- | --- |
| A read of a name that is no parameter or loop variable | `W305` |
| A body `as` that reuses a parameter name | `W311` |
| A `<param>` after the body or outside a fragment, a duplicate parameter, a bad declaration (a bad name or type, `values` on a `string`, a `default` of the wrong type), or a fragment without a body | `W803` |
| An `<outlet>` for a name that is no `slot` parameter, twice for one name, with other attributes or content, or outside a fragment | `W804` |
| A read where its type cannot go: an `action` or `slot` parameter in a prop, a value parameter in `on-*`, a read with more after the name | `W807` |
| A variant parameter or `<variant>` misdeclared (§ above) | `W809` |

A reader without fragments (no project and no inline fragment) reads `<use>` as content it does not know (§8). In lenient mode it warns with `W801` and treats the use as an element with role `group` that holds its slots' content. In strict mode `W801` is an error.

**Inline fragments.** A screen may define a fragment in place, for a block that screen repeats and no other screen uses. It needs no project.

```xml
<screen id="cart" label="Cart" weft="0.3">
  <fragment label="Price row" name="price-row">
    <param name="label" required="true" type="string"/>
    <param name="amount" required="true" type="string"/>
    <stack id="row" direction="row">
      <text id="name" text="{$label}"/>
      <text id="value" text="{$amount}"/>
    </stack>
  </fragment>
  <use id="subtotal" amount="{$.cart.subtotal}" fragment="price-row" label="Subtotal"/>
  <use id="total" amount="{$.cart.total}" fragment="price-row" label="Total"/>
</screen>
```

- `<fragment name="…">` is a direct child of `<screen>`, after the `<context>` block when there is one and before the first body element. `name` is required and follows the name grammar of §2. `label` and `version` are optional. It has no `id` and no `weft`: it is written in the screen's format version. `version`, when present, is a literal `MAJOR.MINOR.PATCH` (`W230`), the same rule as on a fragment file.
- Its content is that of a fragment file: `<param>` elements, then a body. In canonical JSON the inline fragments are `Document.fragments`, the name mapping to the node. Canonical markup writes them after the context block, sorted by name, with `name` among the attributes.
- An inline fragment anywhere else (deeper in the tree, inside `<each>`, a `<slot>`, another fragment or `insert` markup), with no valid `name`, with `weft` or another attribute, after a body element, with a name another inline fragment already has, with a name a project fragment already has, or inside a fragment file, is `W808`. That fragment is left out.
- An inline fragment is known only in its screen. A screen's `<use>` finds the name among the screen's inline fragments, then among the project's. An inline fragment may use the project's fragments and the screen's other inline fragments. Cycles are `W805` and expansion limits are `W806`, unchanged. Helpers shared by fragment files are fragments of the project, not inline fragments.
- Everything else in this section applies unchanged. Ids inside an inline fragment are local and may repeat the screen's ids. A context entry's `for` names a screen id, which for a fragment instance is the id of its `<use>`.

**Patches.** Patches address the screen and, with `fragment`, one of its inline fragments (§7). They never reach into a project fragment. `set-version` sets the document root's `version`, and on a fragment file that is the fragment's own version, because its root has no id.

- `set` on the id of a `<use>` changes a parameter (`"prop": "title"`), an action parameter (`"prop": "on-back"`), or the fragment itself (`"prop": "fragment"`). The result is validated as every patch result is.
- `insert` and `move` with `"parent": "header", "slot": "actions"` fill a slot parameter. The slot must be one the fragment declares (`W504`).
- `remove` and `move` of a `<use>` take the whole instance.
- An id inside a project fragment body (`title`, or the instance path `header/title`) is not a screen id. It is `W502`, and the hint names the fragment to edit. A fragment file is edited with the same patches as a screen, with `<fragment>` as its root.
- An id inside an inline fragment is addressed with `fragment` set to that fragment's name (§7). The fragment's own name, as `id`, addresses the `<fragment>` element, so `label` and `version` are set there. `add-fragment` and `remove-fragment` add and remove inline fragments. Removing one that a `<use>` still names fails as `W801` and applies nothing.

**Tools.** Renderers and generators expand uses before they draw (as above) and write instance paths to `data-weft-id`; the Slint generator, whose element names cannot hold `/`, names an element by its instance path with `--` for `/`. `to-jsx`, which emits a component per fragment, emits one function per inline fragment and a call at each `<use>`. The accessibility tree of the reference renderer equals that of the expanded document. Importers never produce `<use>`, an inline `<fragment>` or `<variant>`, because they cannot tell a fragment from a copy. The core exports `expand(document, catalog)`, which returns the expanded document and its `W805`/`W806` diagnostics. The CLI, the MCP server and `write-page` load fragments with the rest of the project. `weft_catalog` lists the project's fragments with their parameters after the components, and, when given a screen's markup, that screen's inline fragments too. `weft explain` names the variant each use chose.
