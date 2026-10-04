# Catalog and tokens

Two things decide what a Weft screen is allowed to say. The **catalog** is the vocabulary: which elements exist and what each may contain. **Design tokens** are the values of design: spacing, sizes, colors. Both are plain data files that you can read, extend and version.

The examples use the scratch folder from the [tour](tour.md); this block makes it again if you skipped that page.

```console
$ mkdir -p weft-tour plugins/shared/scratch
$ cp corpus/login/screen.weft weft-tour/login.weft
```

## The catalog

A catalog is a JSON file. It lists components; each component has a one-sentence description (written for a model), an ARIA role, what its content may be (nothing, text, elements, or both), and its props, slots, states and events. The core catalog, `weft-core` 0.1, has 29 components and lives in `packages/catalog/catalog.json`. [SPEC §5](../SPEC.md#5-catalog) defines the file format and [§5.1](../SPEC.md#51-core-catalog-weft-core-01) lists every component.

| Group | Components |
| --- | --- |
| Structure | `screen`, `stack`, `grid`, `section`, `tabs`, `tab`, `dialog` |
| Text and media | `heading`, `text`, `image`, `link`, `alert` |
| Actions | `button`, `menu`, `menu-item` |
| Input | `form`, `field`, `checkbox`, `switch`, `radio-group`, `radio`, `select`, `option` |
| Collections | `list`, `item`, `table`, `column`, `row`, `cell` |

You can read the catalog from a terminal with the MCP tool (see [MCP server](mcp.md)):

```console
$ node docs/examples/mcp-call.mjs weft_catalog kind=checkbox
{"kind":"checkbox","description":"A box the user ticks or clears to turn one independent option on or off.","role":"checkbox","content":"none","requiresLabel":true,"props":{"checked":{"description":"Whether the control is on; bind it to a boolean to read and write it.","type":"boolean","writable":true},"disabled":{"description":"Set to true to make the checkbox non-interactive.","type":"boolean"}},"events":["change"]}
```

Read the `props` carefully: `type` is a value type, `values` lists the allowed words of an `enum`, `writable` marks an input that writes back to your data, `bindable: false` means a literal only, and `required` means the validator will insist. Every component also takes the universal attributes of [SPEC §2.2](../SPEC.md#22-universal-attributes): `id`, `label`, `hidden`, `state` and `on-<event>`.

### Extending it

There are two ways to go beyond the core catalog.

**Vendor extensions.** An element or attribute whose name starts with `x-<vendor>-` is allowed everywhere. An extension element must say which ARIA role it falls back to, so a renderer that does not know it can still show its children in a sensible container. This file has a chart element with a role, extra attributes and a slot, and it is valid even in strict mode:

```console
$ weft validate compat/extensions.weft --catalog packages/catalog/catalog.json --strict; echo "exit $?"
exit 0
```

An unknown element that is not an extension is treated differently by readers and writers. A reader (lenient mode) warns and keeps it; a writer (strict mode) must not write it. See [The `weft` command](cli.md#weft-validate) for the two modes and [SPEC §8](../SPEC.md#8-versioning-and-extensibility) for the rules.

**Your own catalog.** Copy `catalog.json`, change it and pass it with `--catalog`. This example adds a `ghost` value to the button's `variant`:

```console
$ node -e 'const fs=require("fs");const c=JSON.parse(fs.readFileSync("packages/catalog/catalog.json","utf8"));c.name="acme";c.version="0.2.0";c.components.button.props.variant.values.push("ghost");fs.writeFileSync("weft-tour/acme.catalog.json",JSON.stringify(c,null,2))'
$ sed 's/variant="primary"/variant="ghost"/' weft-tour/login.weft > weft-tour/login-ghost.weft
$ weft validate weft-tour/login-ghost.weft --catalog packages/catalog/catalog.json --strict; echo "exit $?"
weft-tour/login-ghost.weft:8:61 W203 "ghost" is not an allowed value.
exit 1
$ weft validate weft-tour/login-ghost.weft --catalog weft-tour/acme.catalog.json --strict; echo "exit $?"
exit 0
```

An agent that is told which catalog the host uses (`weft_catalog` reports it) writes only what that catalog allows. There is no shared registry of extension catalogs yet, and the MCP server and the plugin use the core catalog.

### Versioning a catalog

A catalog has its own version. Adding something is a minor change; removing or tightening something is a major change, because a screen that was valid could stop being valid. `diffCatalogs` classifies a change by the rule in [SPEC §8](../SPEC.md#8-versioning-and-extensibility). From a file inside a workspace package (see the note in the [index](README.md)):

```console
$ cat > plugins/shared/scratch/diff.ts <<'EOF'
import { readFileSync } from "node:fs";
import { coreCatalog, diffCatalogs } from "@weft/catalog";

const acme = JSON.parse(readFileSync("weft-tour/acme.catalog.json", "utf8"));
const { level, changes } = diffCatalogs(coreCatalog, acme);
console.log(level);
for (const c of changes) console.log(`${c.level} ${c.path}: ${c.message}`);
EOF
$ node plugins/shared/scratch/diff.ts
minor
minor components.button.props.variant.values: Enum value "ghost" was added.
```

## Design tokens

Tokens are a file in the W3C Design Tokens format (DTCG 2025.10), the format design tools export. A screen never holds a color or a size; it holds a reference such as `gap="{token.space.md}"`, so changing `space.md` in one place changes every screen. A designer can own that file.

`packages/catalog/tokens/default.tokens.json` is the default set: spacing (`space.xs` to `space.xl`), radii, font sizes and a few colors, with aliases such as `color.action.danger` pointing at `color.red`. `loadTokens` reads a file, resolves aliases and returns the tokens and any problems it found, never throwing on bad input. Props that take a token say which type they accept (`gap` takes a `dimension`), and the validator checks both the name and the type.

The `weft` command and the MCP server do not know your tokens, so they accept any well-formed reference. To check names, give the validator the token map. It does the same for the list of action names your app has. This script checks the login screen four ways:

```console
$ cat > plugins/shared/scratch/tokens.ts <<'EOF'
import { readFileSync } from "node:fs";
import { coreCatalog, loadTokens, tokenTypes } from "@weft/catalog";
import { parse } from "@weft/core";

const tokenFile = JSON.parse(readFileSync("packages/catalog/tokens/default.tokens.json", "utf8"));
const { tokens, problems } = loadTokens(tokenFile);
console.log(`${tokens.size} tokens, ${problems.length} problems`);
console.log(tokens.get("space.md"));

const login = readFileSync("weft-tour/login.weft", "utf8");
const check = (name: string, markup: string, actions?: string[]) => {
  const { diagnostics } = parse(markup, {
    catalog: coreCatalog,
    mode: "strict",
    tokens: tokenTypes(tokens),
    ...(actions === undefined ? {} : { actions }),
  });
  console.log(`${name}: ${diagnostics.length} diagnostics`);
  for (const d of diagnostics) console.log(`  ${[d.code, d.message, d.hint].filter(Boolean).join(" ")}`);
};

check("as written", login);
check("unknown token", login.replace("space.md", "space.huge"));
check("wrong token type", login.replace("space.md", "color.ink"));
check("unknown action", login, ["auth.submit", "nav.signup"]);
EOF
$ node plugins/shared/scratch/tokens.ts
18 tokens, 0 problems
{ type: 'dimension', value: { value: 16, unit: 'px' } }
as written: 0 diagnostics
unknown token: 1 diagnostics
  W306 Token "space.huge" does not exist. did you mean "space.lg"?
wrong token type: 1 diagnostics
  W307 Token "color.ink" has type color.
unknown action: 1 diagnostics
  W308 Action "nav.reset" is not provided by the host.
```

`W306` (unknown token), `W307` (wrong type) and `W308` (unknown action) only appear when you supply the lists. The [renderer](rendering.md) and the plugin scripts use the default tokens unless you pass `--tokens`, so they enforce `W306` against that set.

### Where tokens turn into values

| Where | What happens to `{token.space.md}` |
| --- | --- |
| [Renderer](rendering.md) | Replaced with the token's value from the set you gave, for example `16px`. |
| [React export](exporting-jsx.md) | `var(--weft-space-md)`: you define the CSS variable. |
| [Import](importing.md) | Not recovered. A page holds only the resolved size. |

## Limits

- The loader reads the DTCG subset Weft needs: groups, `$type`, `$value`, aliases and `$root`. It does not read `$extends`, `$ref` or the resolver module, and it ignores `$description`, `$extensions` and `$deprecated`.
- There is one set of tokens per run.
- The catalog format has no inheritance. A custom catalog is a full copy that you keep in step with the core one; `diffCatalogs` tells you what changed.
