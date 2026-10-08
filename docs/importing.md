# Importing HTML and component libraries

Importing turns something that already exists, a web page or an accessibility snapshot, into a Weft screen, so you can start from the real thing and refine it with the [MCP tools](mcp.md) instead of writing from nothing.

Be clear about what an import is. A web page keeps no ids, no data bindings, no action names and no design tokens, so a converted screen has only the structure, the roles, the names and the text. The importer tells you everything it could not carry over, in a loss table, and you add those parts back by hand or with an agent.

## Import an HTML file

The plugin's `import.ts` script (the `/weft:import` command in Claude Code) reads an HTML file and writes a `.weft` file next to it. It prints the loss table on standard output.

The examples use the scratch folder from the [tour](tour.md); this block makes it again if you skipped that page, and writes a small page to import.

```console
$ mkdir -p weft-tour
$ cat > weft-tour/pricing.html <<'EOF'
<main aria-label="Pricing">
  <h1>Plans</h1>
  <p>Pick the plan that fits your team.</p>
  <table>
    <caption>Plans and prices</caption>
    <tr><th>Plan</th><th>Price</th></tr>
    <tr><td>Free</td><td>$0</td></tr>
    <tr><td>Team</td><td>$12</td></tr>
  </table>
  <form aria-label="Contact sales">
    <label for="work-email">Work email</label>
    <input id="work-email" type="email" required>
    <button type="submit">Contact sales</button>
  </form>
  <a href="/docs">Read the docs</a>
</main>
EOF
$ node plugins/shared/scripts/import.ts weft-tour/pricing.html
Wrote weft-tour/pricing.weft

Import losses:

| Kind | Path | Note |
| --- | --- | --- |
| bindings | /screen#screen-pricing | values are the resolved values the page shows, not bindings |
| actions | /screen#screen-pricing | event handlers and their action names are not in the HTML |
| tokens | /screen#screen-pricing | design token references are rendered as CSS and cannot be mapped back |
| hidden | /screen#screen-pricing | elements a renderer leaves out (hidden, closed dialogs) are not in the HTML |
$ cat weft-tour/pricing.weft
<screen id="screen-pricing" label="Pricing" weft="0.1">
  <heading id="heading-plans" level="1">Plans</heading>
  <text id="text-pick-the-plan-that-fits-your-tea">Pick the plan that fits your team.</text>
  <table id="table-plans-and-prices" label="Plans and prices">
    <column id="column-plan">Plan</column>
    <column id="column-price">Price</column>
    <row id="row-1">
      <cell id="cell-1">Free</cell>
      <cell id="cell-2">$0</cell>
    </row>
    <row id="row-2">
      <cell id="cell-3">Team</cell>
      <cell id="cell-4">$12</cell>
    </row>
  </table>
  <form id="form-contact-sales" label="Contact sales">
    <field id="field-work-email" label="Work email" required="true" type="email"/>
    <button id="button-contact-sales" submit="true">Contact sales</button>
  </form>
  <link id="link-read-the-docs" href="/docs">Read the docs</link>
</screen>
```

Look at what happened. The roles decided the components: `h1` became a `heading` with `level="1"`, the `<th>` row became the table's `column`s, the `<label>` became the field's `label`, the `submit` button became a `button` with `submit="true"`. Ids were made up from the kind and the name (`heading-plans`, `field-work-email`). The result always validates, in lenient mode, with no errors.

The script refuses to overwrite an existing file; add `--force` to replace it. Exit code 0 means done, 1 means the input could not be imported (the diagnostics are printed), 2 means a usage or file problem.

```console
$ node plugins/shared/scripts/import.ts weft-tour/pricing.html; echo "exit $?"
weft: weft-tour/pricing.weft already exists; pass --force to replace it
exit 2
```

## What a page cannot tell you

Every import ends with this table. The `Kind` column says what is missing; [SPEC §9](../SPEC.md#9-mapping) has the full list with the precise rules.

| Kind | What it means for you |
| --- | --- |
| `ids` | A `data-weft-id` on the page that was not a usable id was replaced by a generated one. |
| `bindings` | Values on the page are what it showed. You have to turn the ones that come from data into `{$.path}` bindings. |
| `actions` | Event handlers are not in HTML. Add `on-press`, `on-submit` and the like with your app's action names. |
| `tokens` | Colors and sizes became CSS and cannot be traced back to tokens. |
| `layout` | The gap of a flex or grid container is a plain size on the page and cannot be mapped back to a token. |
| `repetition` | A repeated list is imported as separate static items, not an `<each>`. |
| `slots` | A kind's content that the page does not mark with `data-weft-slot` lands in the default content, or in the first slot that takes it; move it into `<slot>` by hand. Pages that the renderers wrote carry the marker, so their slots come back. |
| `hidden` | Hidden elements and closed dialogs are not in the result. |
| `props` | Details with no equivalent in HTML are lost, or values that the catalog rejects. |
| `values`, `names` | A required value or label that was missing got a stand-in (an empty label, a placeholder value). |
| `kinds` | An element with a role that the catalog has no component for becomes an extension, `x-aria-<role>`, which keeps its role as a fallback. |
| `text` | Text with nowhere to go was dropped. |
| `structure` | The page had no single `main` landmark, so a `screen` was added around it. |

A short page with a landmark the catalog has no component for shows `kinds` at work: the `<nav>` has role `navigation`, so it becomes an extension element that carries its role, and the hidden `<div>` is not imported.

```console
$ cat > weft-tour/misc.html <<'EOF'
<main aria-label="Misc">
  <nav aria-label="Site"><a href="/a">A</a></nav>
  <ul><li>One</li><li>Two</li></ul>
  <img src="/logo.png" alt="Logo">
  <label>Plan <select><option>Free</option><option selected>Team</option></select></label>
  <input type="checkbox" aria-label="Subscribe" checked>
  <div hidden>Secret</div>
</main>
EOF
$ node plugins/shared/scripts/import.ts weft-tour/misc.html | tail -n 2
| hidden | /screen#screen-misc | elements a renderer leaves out (hidden, closed dialogs) are not in the HTML |
| kinds | /screen#screen-misc/x-aria-navigation#navigation-site | role navigation has no kind in the catalog |
$ cat weft-tour/misc.weft
<screen id="screen-misc" label="Misc" weft="0.1">
  <x-aria-navigation id="navigation-site" label="Site" role="navigation">
    <link id="link-a" href="/a">A</link>
  </x-aria-navigation>
  <list id="list-1">
    <item id="item-1">One</item>
    <item id="item-2">Two</item>
  </list>
  <image id="image-logo" label="Logo" src="/logo.png"/>
  <select id="select-plan" label="Plan" value="Team">
    <option id="option-free" value="Free">Free</option>
    <option id="option-team" value="Team">Team</option>
  </select>
  <checkbox id="checkbox-subscribe" checked="true" label="Subscribe"/>
</screen>
```

## Better input, better import

- **Semantic HTML imports best.** Use real `<button>`, `<label>`, `<table>`, `<h1>`…, and ARIA roles where the element has none. The importer reads implicit HTML roles and the accessible name the way a browser would (`aria-labelledby`, `aria-label`, `alt`, `<label>`, then `title`, then the text).
- **A page the Weft renderer wrote comes back almost whole.** The renderer puts every id in `data-weft-id`, and the importer keeps it, so ids and layout containers survive. Bindings, actions and tokens still do not. The [tour](tour.md) does this round trip.
- **Layout comes back only from Weft's own pages.** A `stack` or `grid` is recognised from `display: flex` or `display: grid` on an element that carries a `data-weft-id`. On any other page a plain `<div>` has no role, so it disappears and its children take its place.

## From an accessibility snapshot

The importer also reads an accessibility snapshot: the YAML a Playwright aria snapshot gives, or the same tree as objects. That is what `weft_render` prints too, so a screen can be imported from a live browser session. The plugin has no command for it; use the library from a file inside a workspace package (see the note in the [index](README.md)):

```console
$ mkdir -p plugins/shared/scratch
$ cat > plugins/shared/scratch/snapshot.ts <<'EOF'
import { coreCatalog } from "@weft/catalog";
import { serialize } from "@weft/core";
import { fromAriaSnapshot } from "@weft/from-aria";

const snapshot = `- main "Sign in":
  - heading "Sign in" [level=1]
  - textbox "Email"
  - textbox "Password"
  - button "Sign in" [disabled]
`;
const { document, losses } = fromAriaSnapshot(snapshot, { catalog: coreCatalog });
process.stdout.write(serialize(document));
console.log(losses.map((l) => l.kind).join(", "));
EOF
$ node plugins/shared/scratch/snapshot.ts
<screen id="screen-sign-in" label="Sign in" weft="0.1">
  <heading id="heading-sign-in" level="1">Sign in</heading>
  <field id="field-email" label="Email"/>
  <field id="field-password" label="Password"/>
  <button id="button-sign-in" disabled="true">Sign in</button>
</screen>
ids, bindings, actions, tokens, layout, repetition, slots, hidden, props
```

`fromDom` does the same for HTML in a string. Both return `{ document, losses, diagnostics }`.

## A catalog from a Custom Elements Manifest

A page becomes a screen; a component library becomes a catalog. A web component library that publishes a [Custom Elements Manifest](https://github.com/webcomponents/custom-elements-manifest) (`custom-elements.json`, schema 2.x) can bring its elements into Weft as kinds, so screens use `<acme-button>` with checked props and events:

```console
$ weft import-cem node_modules/acme-ui/custom-elements.json --name acme-ui --version 2.0.0 --out-dir catalogs
```

That writes `catalogs/custom-elements.catalog.json`, a catalog extension: name it as the `catalog` of your `weft.json` ([Projects](projects.md)) and the kinds sit beside the core ones. Each custom element is a kind with the role `generic` (a manifest has no roles); attributes and public, writable fields are props (`boolean`, `number`, `string`, or an `enum` of string literals); slots are slots; events are events, camelCase names becoming kebab-case. The losses on stderr list the rest: methods, private, static and read-only members, CSS parts and properties, event payload types, and any name that is not a Weft name. An element whose tag the project's catalog already has is left out, so importing again after adding the result to the project gives nothing new.

From TypeScript:

```ts
import { importCem } from "@weft/core/cem";

const { catalog, losses, diagnostics } = importCem(manifestText, {
  catalog: coreCatalog, // the catalog the result extends
  name: "acme-ui",
  version: "2.0.0",
});
```

The manifest is parsed as data and never run. A file over 10 MB is not read; more than 2000 elements, or more than 1000 members in one list, is cut with `W602`; input that is not a manifest of schema 2.x is `W601`. The full mapping is in SPEC §9.

## Safe input

An import never runs the page. Scripts are not executed, a text that would read as a binding or token reference has its brace replaced, and the page length, number of elements and nesting depth are bounded (`W602` says a limit was hit and the rest was not imported).

## Limits

- It converts one page or fragment, not an app. Pages with several `main` regions get one screen around them.
- The result is a draft. Add bindings, actions and tokens before it does anything in your app.
- The plugin script takes a file, not a URL. Save the page's HTML first.
