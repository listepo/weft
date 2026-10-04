# Fragments: design proposal

Status: proposal for the creator (T31, part B). Nothing here is built. Once approved it lands in `SPEC.md`, `AGENT-SPEC.md`, the Rust core (which `@weft/core` runs through WebAssembly) and the differential fixtures together.

## 1. Problem

Screens of one project repeat blocks: a page header, a footer, a product card. Today each screen copies them, so a change to the header is an edit to every screen, and nothing checks that the copies stay alike. A fragment is such a block kept once, in its own file, and placed in screens by reference.

Goals:

- One definition, many uses, checked at every use.
- Screens stay readable on their own: a `<use>` says which fragment and with what values.
- Canonical JSON keeps the `Node` shape of SPEC §3. A fragment adds elements, not a new model.
- Ids stay stable and addressable by patches.
- Fragment bodies cannot run code or blow up when expanded.

Not goals: namespaces or sharing fragments across projects (that is T17), fragments that take arbitrary markup as arguments, conditional logic.

## 2. Where fragments live

The project file gets a fifth member:

```json
{
  "fragments": {
    "page-header": "fragments/page-header.weft",
    "product-card": "fragments/product-card.weft"
  }
}
```

- Keys are fragment names (the name grammar of SPEC §2: `[a-z][a-z0-9]*(-[a-z0-9]+)*`). The name lives only here, so a file cannot disagree with the project about what it is called.
- Values are file names under the rules of SPEC §10.2 (relative, inside the project directory, `/` separators). In project content (the MCP `project` argument) a value is the fragment's markup as a string.
- Problems with the member or a file use the existing project codes: `W701` wrong shape, `W703` bad file name, `W704` unreadable file. Markup errors in a fragment file keep their `W1xx`/`W2xx`/`W3xx` codes, with positions in that file and a path that starts at `#/fragments/<name>`.
- Without a project there are no fragments.

## 3. Syntax

### 3.1 A fragment file

```xml
<fragment weft="0.2" label="Page header">
  <param name="title" type="string" required="true"/>
  <param name="tone" type="enum" values="default muted" default="default"/>
  <param name="back" type="action"/>
  <param name="actions" type="slot" content="nodes"/>
  <toolbar id="bar" label="{$title}">
    <button id="back" on-press="{$back}" variant="ghost">Back</button>
    <heading id="title" level="1" text="{$title}" tone="{$tone}"/>
    <outlet name="actions"/>
  </toolbar>
</fragment>
```

- The root is `<fragment>`, with `weft` and an optional `label` (a description for tools and the catalog listing, not an accessible name). It has no `id`.
- `<param>` elements come first. Each declares one parameter in the vocabulary of a catalog prop (SPEC §5): `name`, `type` (`string`, `number`, `boolean`, `enum`, `token`, plus `action` and `slot`), and as the type needs: `values` (space-separated, for `enum`), `token-type`, `min`, `max`, `integer`, `required`, `default`. A `slot` parameter takes `content` and optionally `allowed-children`, as a catalog slot does.
- After the parameters comes the body: one or more elements, exactly as they could stand inside a `<screen>`.
- `<outlet name="…"/>` marks where a `slot` parameter's content goes. It is a structural element like `<slot>` and `<each>`: no id, one attribute.

Inside the body a parameter is read with the existing loop-variable form of a binding, `{$title}` (SPEC §2.1: `$` name), so the binding grammar does not change. A negated read `{!$busy}` works for boolean parameters. An `action` parameter is read in an `on-*` attribute as `on-press="{$back}"`; this is the one place where an `on-*` value may be braced, and only inside a fragment.

Reads of the host data model (`{$.cart.total}`) are allowed in a body and mean the same as in the screen that uses it.

### 3.2 A use

```xml
<screen id="cart" label="Cart" weft="0.2">
  <use id="header" fragment="page-header" title="Your cart" on-back="nav.back">
    <slot name="actions">
      <button id="clear" on-press="cart.clear">Clear</button>
    </slot>
  </use>
  …
</screen>
```

- `<use>` is a structural element. It takes `id` (required, like `<each>`), `fragment` (the name), one attribute per value parameter, `on-<name>` per action parameter, and `<slot name="…">` children per slot parameter. It has no default content and takes none of the other universal attributes (`label`, `hidden`, `state`, `role`): a header that can be hidden declares a `hidden` boolean parameter and passes it on.
- Value parameters take every value form of SPEC §2.1: a literal typed by the parameter, a binding, a negated binding, a token reference. A binding is read where the `<use>` stands, so `{$line.name}` inside an `<each as="line">` works.
- `fragment` is reserved: no parameter may be called `fragment` or `id`.

### 3.3 Canonical JSON

No new types. A use is a `Node`:

```json
{
  "kind": "use",
  "id": "header",
  "props": { "fragment": "page-header", "title": "Your cart" },
  "on": { "back": "nav.back" },
  "slots": { "actions": [{ "kind": "button", "id": "clear", "on": { "press": "cart.clear" }, "children": ["Clear"] }] }
}
```

A fragment file is a `Document` whose root has kind `fragment`; `param` and `outlet` nodes are structural nodes without id, with their attributes in `props` (literals typed as the parameter grammar says: `required` boolean, `min` number, …). A parameter read is an ordinary binding value, `{ "bind": "$title" }`; an action parameter in `on` is the string `"{$back}"`. The canonical form rules of SPEC §3 apply unchanged; `<param>` elements keep document order, before the body.

The canonical form keeps `<use>` unexpanded. Expansion (§5) is something a renderer or checker does, never a rewrite of the document.

## 4. Ids

- Ids inside a fragment body are local: unique within the fragment (`W301` there), free to repeat the ids of any screen.
- The `<use>` id is a screen id like any other.
- An expanded element is addressed by its instance path, the use id and the local id joined by `/`: `header/title`. Inside `<each>` the index rules of SPEC §4.3 apply first, so a fragment used in a loop gives `card[2]/price`. Nested uses join further: `header/crumbs/home`. `/` is outside the id grammar, so an instance path can never collide with a document id.
- Renderers write the instance path to `data-weft-id`; diagnostics about expanded content use it in their path (`/screen#cart/use#header/toolbar#bar`, with the fragment file's own path after it when the problem is in the body).

## 5. Meaning: expansion

A use means its fragment's body, placed where the `<use>` stands, with:

1. every parameter read replaced by the use's value, evaluated in the use's scope (a negated read of a negated binding is the plain binding; a negated read of a boolean literal is its opposite; an absent optional parameter takes its `default`, or makes the attribute absent when there is none);
2. every `<outlet name="x">` replaced by the content of the use's `<slot name="x">`, or by nothing;
3. every id replaced by its instance path (§4).

Evaluation is scoped, not textual: loop variables of the body never capture names from the use site, and the reverse. An implementation that expands by rewriting must rename the body's loop variables to keep this.

`<use>` is transparent for structure, like `<each>`: the body's top-level elements are checked against the parent's (or slot's) `allowedChildren` and their own `allowedParents` where the `<use>` stands, and `<use>` itself adds no node to the accessibility tree.

Bounds, because a fragment file is as untrusted as a screen:

- A fragment may use other fragments. A cycle (`a` uses `b` uses `a`) is an error (`W805`) and nothing in it expands.
- Expansion counts toward the 256-level depth limit of SPEC §2 and toward a limit on expanded elements per screen (proposal: 10,000; `W806`), so a few short files cannot expand into millions of nodes.

## 6. Validation

A fragment is checked once on its own, and each use is checked against the fragment's parameters. Expanded content is not re-validated element by element; what the body declares is enough to judge it.

On its own, a fragment body is validated as a screen body with the project's catalog, tokens, actions and data schema, where each parameter read counts as a value of the parameter's type: a `string` parameter read in a `number` prop is `W204`, an `enum` parameter whose values are not all allowed by the prop is `W203`, a `token` parameter whose `token-type` differs from the prop's is `W307`, and so on. Binding reads of `$.…` are checked against the data schema (`W315`, `W316`).

At a use:

| Situation | Code |
| --- | --- |
| `fragment` names no fragment of the project | `W801` (hint: nearest name) |
| Attribute, `on-*` or slot that the fragment does not declare | `W802` |
| Required parameter missing | `W205` |
| Value not of the parameter's type, enum value or range | `W204`, `W203`, `W224` |
| Binding or token problems in a value | the existing `W214`, `W305`, `W306`, `W315`, … |
| Body element not allowed where the `<use>` stands | `W302`, `W303` |
| Slot content against the slot parameter's `content` and `allowed-children` | `W304`, `W302` |

In a fragment:

| Situation | Code |
| --- | --- |
| Parameter read with no such parameter | `W305`, as an unknown loop variable is |
| A body `as` that reuses a parameter name | `W311` |
| `<param>` after the body, duplicate parameter, bad declaration (`values` on a `string`, `default` of the wrong type) | `W803` |
| `<outlet>` for a name that is no `slot` parameter, or twice for one name | `W804` |
| Parameter read where its type cannot go (an `action` in a prop, a value parameter in `on-*`, a `slot` outside `<outlet>`) | `W807` |
| Root not `<fragment>`, or a `<fragment>` used as a screen | `W201` |

Cross-file:

| Situation | Code |
| --- | --- |
| Use cycle | `W805` |
| Expansion over the element limit | `W806` |

Without a project (so with no fragments), `<use>` follows SPEC §8 for content a reader does not know: a lenient reader warns (`W801` as a warning) and treats it as an element with role `group` holding its slots' content; a strict reader rejects it.

## 7. Patches

Patches address the screen; they never reach into a fragment.

- `set` on a `<use>` id changes a parameter (`"prop": "title"`), an action parameter (`"prop": "on-back"`), or the fragment itself (`"prop": "fragment"`). The result is validated as any patch result is, so a value of the wrong type is reported and nothing applied.
- `insert` and `move` with `"parent": "header", "slot": "actions"` fill a slot parameter; the slot must be one the fragment declares (`W504`).
- `remove` and `move` of a `<use>` take the whole instance.
- An id inside a fragment body (`title`, or the instance path `header/title`) is not a screen id: `W502`, with a hint that names the fragment to edit. A fragment file is edited with the same patches as a screen, its root being `<fragment>`; the MCP tools take it through the `project` argument.

## 8. Tools

- **Renderers** expand uses before rendering (§5) and write instance paths to `data-weft-id`. The reference renderer's accessibility tree must equal that of the expanded document.
- **to-jsx** turns each fragment used by the screen into its own function component in the same output module, `function PageHeader({ data, actions, onChange, params, slots })`, and each `<use>` into a call that passes parameter values evaluated at the use site, action names, and slot content. Scoping then falls out of JavaScript closures. Instance ids follow §4.
- **Importers** from accessibility snapshots and DOM never produce `<use>`: they cannot tell a fragment from a copy. Given `data-weft-id` instance paths, the DOM importer turns `header/title` into the id `header-title` and records a loss of kind `fragments`. A later importer with the project at hand may fold a subtree that matches a fragment back into a `<use>`; that is out of scope here.
- **Figma** (T14): a fragment maps naturally to a Figma component and a `<use>` to an instance whose component properties are the parameters and whose overrides are the values. Figma's API keeps the list of fields an instance overrides (`InstanceNode.overrides`), which is what a round trip needs to tell a parameter from a local edit.
- **CLI, MCP, write-page** load fragments with the rest of the project (SPEC §10); `weft_catalog` lists the project's fragments with their parameters after the components.

## 9. Versioning

`<use>`, `<fragment>`, `<param>` and `<outlet>` are additions, so they come with format version 0.2 (SPEC §8: a minor version only adds). A 0.1 reader sees `<use>` as an unknown element: it warns and keeps it, which is the behaviour SPEC §8 already promises.

## 10. Alternatives considered

| Alternative | Why not |
| --- | --- |
| Use the fragment name as an element (`<page-header>`) | Collides with catalog kinds and extension rules; a reader cannot tell a fragment from a component it does not know. |
| Expand at load time and store the expansion | Loses the single definition; patches and diffs would touch every copy. |
| Textual include (XInclude, SSI-style) without parameters | No way to vary a title or an action, so every slight variant becomes its own file. |
| Prefix ids (`header-title`) instead of instance paths | Can collide with real ids and needs a uniqueness check across files; `/` cannot collide. |
| Parameters as JSON Merge Patch over the body | Addresses by structure, not by name, and RFC 7396 replaces arrays wholesale, so slot content could not be merged. |

## 11. Open questions for the creator

1. `fragments` as a name → file map (proposed) or a list of files whose names come from the file?
2. Is the braced `on-press="{$back}"` acceptable as the one place an `on-*` value is braced, or should action parameters be forwarded some other way (for example a fragment declaring `events` that bubble to the `<use>`)?
3. Should `<use>` have a default slot (`<outlet/>` without a name), or named slots only (proposed)?
4. Element limit for expansion: 10,000 per screen?
5. Does this wait for format version 0.2, or is it acceptable as a 0.1 addition behind the project file?

## Sources

| Fact | Source (checked) |
| --- | --- |
| Figma instances expose component properties, set them with `setProperties`, and list the fields they override directly (`overrides`) | https://developers.figma.com/docs/plugins/api/InstanceNode/ (2026-10-05) |
| JSON Merge Patch: `null` removes a member; an array in a patch replaces the target array | RFC 7396, https://www.rfc-editor.org/rfc/rfc7396 (2026-10-05) |
| Shadow DOM slots: content is assigned to slots by name inside a shadow tree | DOM Standard §4.2.2, https://dom.spec.whatwg.org/#shadow-tree-slots (2026-10-05) |
| Weft structure, values, patches, versioning and projects | `SPEC.md` §2–§4, §7, §8, §10 of this repository |
