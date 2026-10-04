# Weft for agents

How an AI agent reads, writes, edits and repairs Weft. This is the agent's side of the format: [`SPEC.md`](SPEC.md) is the contract and wins on any conflict; this document says what an agent must do to stay inside it. The MCP primer (`packages/mcp/src/primer.ts`) is a condensed version of it.

The examples are valid in strict mode against the core catalog `weft-core` 0.1 and in canonical form; `bench/test/agent-spec.test.ts` checks them.

## 1. What the host gives you

| Input | Use it for |
| --- | --- |
| The catalog (`weft_catalog`, or `catalog.json`) | The only elements, attributes, slots, states and events that exist. |
| The data model (paths such as `$.user.email`) | The targets of bindings. Do not invent paths the host does not have. |
| The action names (`auth.submit`, `nav.back`) | The values of `on-<event>`. When the host lists actions, use only those (`W308`). |
| The design tokens (`space.md`, `color.accent`) | The values of token props. When the host lists tokens, use only those (`W306`). |
| The format version and catalogs it reads | `weft="0.1"` on the root; write nothing a host does not advertise. |

Validate in **strict** mode before you answer: unknown elements and attributes are warnings for readers but errors for writers.

## 2. Writing a screen

```xml
<screen id="todos" label="Todos" weft="0.1">
  <form id="add" label="Add a todo" on-submit="todo.add">
    <stack id="add-row" direction="row" gap="{token.space.sm}">
      <field id="title" label="Title" required="true" value="{$.draft}"/>
      <button id="add-go" disabled="{!$.draft}" submit="true" variant="primary">Add</button>
    </stack>
  </form>
  <list id="items" label="Todos">
    <each id="todo-each" as="todo" in="{$.todos}">
      <item id="todo">
        <checkbox id="todo-done" checked="{$todo.done}" label="Done" on-change="todo.toggle"/>
        <text id="todo-title" text="{$todo.title}"/>
      </item>
    </each>
    <slot name="empty">
      <text id="none" tone="muted">Nothing to do.</text>
    </slot>
  </list>
</screen>
```

### 2.1 Syntax

- One root `<screen id="…" weft="0.1">`, with a `label` naming the screen. Nothing outside it but comments and whitespace.
- Names are lowercase with hyphens: `radio-group`, `on-press`. No `class`, `style`, `onClick`, `aria-*` or `data-*`: they are not Weft.
- Every attribute has a value in double quotes. No bare attributes: `required="true"`, not `required`.
- An element without content is self-closing: `<field id="f" label="Name"/>`.
- Escape `&` as `&amp;` and `<` as `&lt;` in text and values. Only the five XML entities and numeric references exist (`&nbsp;` does not; write the character itself).
- No XML declaration, DOCTYPE, CDATA or processing instructions.

### 2.2 Ids

- Every element has an `id`, unique in the document: a letter, then letters, digits, `_` or `-`.
- Ids are stable. Keep the ids of elements you did not change; give new elements new ids. A patch addresses elements by id.
- `<slot>` has no id. `<each>` has one, and ids inside `<each>` are templates, still unique in the document.

### 2.3 Values

An attribute value is exactly one of:

| Form | Example | Use |
| --- | --- | --- |
| Literal | `label="Email"`, `level="2"`, `required="true"` | A constant. Numbers and booleans are written as JSON (`2`, `true`). |
| Binding | `value="{$.user.email}"` | Data from the host. On a writable prop (`field.value`, `checkbox.checked`) input writes back. |
| Negated binding | `disabled="{!$.busy}"` | Boolean props only, never on a writable prop. |
| Token | `gap="{token.space.md}"` | Every design value: spacing, color, size. Never `gap="16px"`. |

- Never mix text and a reference: `text="Hello {$.name}"` is an error. Bind the whole value, and put fixed text in its own element.
- A literal that starts with `{` is written `{{`: `text="{{curly}"` reads as `{curly}`.
- Binding paths: `$.a.b.0` from the data model, `$item.field` inside `<each as="item">`.
- `id`, `on-*` and the slot `name` are plain text, never references.

### 2.4 Text

- A component whose content is text (`heading`, `text`, `link`, `button`, `radio`, `option`, `column`, `menu-item`) or mixed (`item`, `cell`, `alert`) takes its text as content: `<button id="b">Save</button>`.
- Bound text goes in the `text` attribute: `<text id="t" text="{$.greeting}"/>`. Content or `text`, never both.
- `label` is the accessible name. Components marked "label" below need one; on a component that shows its text, leave `label` out.

### 2.5 Events and actions

- `on-<event>="action.name"`, for an event the component declares: `on-press` on `button`, `on-submit` on `form`.
- An action is a name, not code: `auth.submit`, not `submit()`, `press:auth.submit` or `actions.auth.submit()`. It takes no arguments; the host receives the element id and, inside `<each>`, the current item.
- A `button` with `submit="true"` fires its enclosing `form`'s `submit` and needs no `on-press`. It must be inside a `form`.

### 2.6 Structure

- Content placed inside an element is its default slot; the component's content model decides what it takes: `none`, `text`, `nodes` (elements) or `mixed`.
- A named slot is `<slot name="…">` as a direct child of a component that declares it. It holds elements only, at most once per parent. Where it stands among the content does not matter; the component places it.
- `<each id="…" in="{$.items}" as="item">` repeats its element children once per array item. `in` is a binding to an array; `as` is a new name (`[a-z][A-Za-z0-9]*`) that no enclosing `<each>` uses. `<each>` is transparent: its children follow the rules of its parent.
- Kinds with a fixed parent stay in it: `radio` in `radio-group`, `option` in `select`, `item` in `list`, `column` and `row` in `table`, `cell` in `row`, `tab` in `tabs`, `menu-item` in `menu`.
- `<screen>` is the root only. Nest at most 256 levels.

### 2.7 Components of `weft-core` 0.1

Props are strings unless a type is given; `*` marks a required prop; "label" means the accessible `label` is required; events follow `;`. Every `text` or `mixed` component also takes `text`.

- Layout: `stack` (`direction` column|row, `gap` token, `align` start|center|end|stretch, `wrap` boolean), `grid` (`columns`* integer ≥ 1, `gap` token), `section` (label; slot `header`).
- Text: `heading` (`level`* integer 1–6), `text` (`tone` default|muted|success|warning|danger), `image` (`src`*, label), `link` (`href`; press), `alert` (`tone` info|success|warning|danger; mixed).
- Actions: `button` (`variant` primary|secondary|danger, `disabled` boolean, `submit` boolean literal; states idle|busy; press), `menu` (label) holding `menu-item` (`disabled`; press).
- Forms: `form` (slot `footer`; states idle|submitting|invalid; submit), `field` (label, `type` text|email|password|number|search|multiline, `value` writable, `placeholder`, `required`, `disabled`, `error`; states valid|invalid; change), `checkbox` and `switch` (label, `checked` writable, `disabled`; change), `radio-group` (label, `value` writable; change) holding `radio` (`value`*, `disabled`), `select` (label, `value` writable, `disabled`; change) holding `option` (`value`*).
- Collections: `list` (`ordered`; slot `empty`; states ready|loading|empty) holding `item` (mixed; press), `table` (label; slot `empty`; states ready|loading|empty) holding `column` (`sort` none|ascending|descending; press) and `row` (`selected`; press) holding `cell` (mixed).
- Containers: `tabs` (`selected` writable, the id of a `tab`; change) holding `tab` (label; holds its panel content), `dialog` (label, `modal`, `open` writable; slot `actions`; close).
- Root: `screen` (states ready|loading|error).

`state` takes only the states listed for the component. The catalog the host serves is the source of truth; ask for a component in full when unsure.

### 2.8 Extensions

When nothing in the catalog fits, an extension element `x-<vendor>-<name>` with a `role` (its ARIA fallback) is allowed, and extension attributes `x-<vendor>-<name>` go on any element. Use them only when the host knows them; never as a way around a catalog rule.

## 3. Editing a screen

Prefer patches to rewriting. A patch list is JSON, addresses elements by id, applies in order and is all-or-nothing: if one patch fails or the result is invalid, nothing changes and the diagnostics explain why.

```json
[
  { "op": "set", "id": "add-go", "prop": "text", "value": "Add todo" },
  {
    "op": "insert",
    "parent": "todo",
    "markup": "<button id=\"todo-delete\" on-press=\"todo.remove\" variant=\"danger\">Delete</button>"
  },
  { "op": "set", "id": "todo-done", "prop": "on-change", "value": "todo.check" },
  { "op": "move", "id": "items", "parent": "todos", "index": 0 },
  { "op": "remove", "id": "none" }
]
```

- **`set`** `{ op, id, prop, value }` changes one prop. `value` is typed JSON, not markup: `"text"`, `7`, `true`, `{ "bind": "$.busy" }`, `{ "bind": "$.busy", "not": true }`, `{ "token": "space.md" }`. `null` removes the prop. A prop `on-<event>` sets or (with `null`) removes an action. `prop: "text"` changes an element's text wherever it is kept. `id` and the root's `weft` cannot be set.
- **`insert`** `{ op, parent, slot?, index?, markup }` adds one or more elements written as markup, with ids the document does not have yet. Without `slot` they go into the default content; `index` counts that list, text included, and defaults to the end.
- **`remove`** `{ op, id }` deletes an element and everything in it.
- **`move`** `{ op, id, parent, slot?, index? }` moves an element with its content; `index` counts the target list after the element has left it.
- The root cannot be removed or moved. Text that shares a list with elements is not addressable: remove the element that holds it and insert it again.

Rewrite the whole screen only when most of it changes. Then keep every id that still names the same thing.

## 4. Repairing with diagnostics

Every problem comes back as a diagnostic:

```text
{ code: "W203", severity: "error", message: '"submiting" is not an allowed value.',
  path: "/screen#login/form#f1/@state", line: 2, column: 15,
  expected: 'one of: "idle", "submitting", "invalid"', got: '"submiting"',
  hint: 'did you mean "submitting"?' }
```

- `path` names the element (`kind#id`, or `kind[index]` without an id), then `slot[name]`, `#text[index]` or `@attribute`. Patch problems point into the list: `#/patches/2/parent`. JSON shape problems are JSON Pointers: `#/root/children/0`.
- `expected` says what would be valid; `hint`, when present, is the smallest fix. Apply it.
- Fix every error, then validate again. Fix the first syntax error first: later ones may follow from it. Warnings (`W401`–`W403` in lenient mode) do not block a reader but are errors in strict mode.
- Change only what the diagnostics name. Do not rewrite unrelated parts while repairing.

What each code asks of you:

| Code | Fix |
| --- | --- |
| W101 | Repair the broken tag: a stray `<` (write `&lt;`), a missing `>`, attributes without a space between them. |
| W102 | Remove the `<?xml …?>` declaration or processing instruction. |
| W103 | Remove the DOCTYPE or declaration. |
| W104 | Replace the CDATA section with escaped text. |
| W105 | Rename to lowercase with hyphens: `Button` → `button`, `onPress` → `on-press`. |
| W106 | Put the value in double quotes. |
| W107 | Give the attribute a value: `required="true"`. |
| W108 | Keep one of the duplicate attributes. |
| W109 | Close the innermost open element first. |
| W110 | Close the element, tag or quoted value. |
| W111 | Remove the closing tag that has no open element. |
| W112 | Use one of `&lt; &gt; &amp; &quot; &apos;`, a numeric reference, or the character itself; write `&amp;` for `&`. |
| W113 | Remove the character, or escape it (`&lt;` in values, `]]&gt;` in text). |
| W114 | Keep exactly one root; move stray text inside an element. |
| W115 | Close the comment with `-->` and remove `--` from inside it. |
| W116 | Write a whole reference (`{$…}`, `{!$…}`, `{token.…}`) or escape the brace: `{{`. |
| W117 | Flatten the nesting. |
| W118 | Make `<slot name="…">` a direct child of a component, with only the `name` attribute. |
| W119 | Merge the two slots of the same name. |
| W200 | Give the JSON the shape of SPEC §3: `{ weft, root }`, nodes with `kind`, `id`, `props`, `on`, `slots`, `children` and nothing else. |
| W201 | Make `<screen>` the root. |
| W202 | Add a unique `id`. |
| W203 | Use one of the values in `expected`. |
| W204 | Use the type in `expected`: `level="2"`, not `level="two"`; a token, not a raw size. |
| W205 | Add the required prop, `label` or the root's `weft="0.1"`. |
| W206 | Use an event the component declares, or move the action to a component that has it. |
| W207 | Use a slot the component declares, or put the elements in the default content. |
| W208 | Add the required slot. |
| W209 | Remove `role` from the catalog component. |
| W210 | Add `role` to the extension element. |
| W211 | Use a WAI-ARIA 1.2 role. |
| W212 | Change the id to a letter followed by letters, digits, `_` or `-`. |
| W213 | Bind the whole value; put fixed text in its own element. |
| W214 | Fix the binding path: `$.name(.name)*` or `$item(.name)*`. |
| W215 | Fix the token path: segments joined by dots. |
| W216 | Fix the action name: `group.name`, lowercase start, no spaces or calls. |
| W217 | Use a literal: this prop takes no binding. |
| W218 | Use a plain binding; negation is for read-only boolean props. |
| W219 | Write the version as `major.minor`: `weft="0.1"`. |
| W220 | Name the extension `x-<vendor>-<name>`. |
| W221 | Remove control characters from the string. |
| W222 | Give `<each>` `in="{$.items}"` and `as="item"`. |
| W223 | Rename the kind, prop, event or slot to a valid name; ids go in `id`, events in `on-*`. |
| W224 | Use a number within the range (and whole, if required); `hint` names the nearest one. |
| W301 | Give one of the two elements another id. |
| W302 | Use a kind the parent (or slot) accepts, or move the element. |
| W303 | Put the element inside its required parent. |
| W304 | Respect the content model: wrap loose text in `<text id="…">`, move elements out of text-only components. |
| W305 | Use a loop variable an enclosing `<each>` defines, or a `$.` path. |
| W306 | Use a token from the host's token set. |
| W307 | Use a token of the type the prop expects. |
| W308 | Use an action the host provides. |
| W309 | Point the reference at an existing element of the right kind (`tabs.selected` at a `tab` id). |
| W310 | Keep the text in content or in `text`, not both. |
| W311 | Give the inner `<each>` another `as` name. |
| W312 | Use `section` or `stack` below the root. |
| W313 | Move the submit button into a `form`, or drop `submit` and give it `on-press`. |
| W314 | Put the element to repeat inside `<each>`, or remove the `<each>`. |
| W401 | Use a catalog component (see `hint`), or an extension the host knows. |
| W402 | Use an attribute the component declares, or remove it. |
| W403 | Write `weft="0.1"`: the reader is older than the version you wrote. |
| W404 | Write `weft="0.1"`: the reader cannot read that major version. |
| W501 | Send an array of patches, each with exactly the members shown in section 3. |
| W502 | Use an id that exists in the current document; `hint` names the nearest. |
| W503 | Do not set `id` or `weft`; use a valid prop name and an action name for `on-*`. |
| W504 | Use a slot the parent declares, or omit `slot`. |
| W505 | Use an index from 0 to the list length, or omit it to append. |
| W506 | Move the element somewhere outside itself. |
| W507 | Do not remove or move the root; send a whole new screen instead. |
| W508 | Insert elements only: no loose text, no `<slot>`. |
| W509 | Give the inserted elements ids the document does not have; `hint` suggests one. |
| W601 | The importer could not read its input; nothing to repair in a document. |
| W602 | The import was cut at a limit; the rest of the input is missing. |

## 5. Reading a screen

- An element's purpose comes from its kind and the catalog's role, its name from `label` or its text.
- What it does is its `on-*` action; a `submit` button triggers its form's `on-submit` action.
- What it shows or edits is its bindings: `{$.path}` reads, and on a writable prop also writes.
- Content inside `<each>` appears once per item; `<slot name="empty">` shows only when a list or table has nothing to show.
- Name things the way the document does: an action as `todo.add`, a path as `$.draft`, an element by its id.

## 6. Checklist

Before you answer, every one of these holds:

1. One `<screen>` root with `weft="0.1"` and a `label`.
2. Every element has a unique id; untouched elements keep theirs.
3. Only catalog kinds, props, states, slots and events, or extensions the host knows.
4. Every value is one literal or one whole reference; tokens for design values.
5. Required props, labels and slots are there; kinds sit in the parents they need.
6. Actions are names the host provides; submit buttons are inside a form.
7. The validator, in strict mode, reports no errors.

Keep this document current: see `AGENTS.md`.
