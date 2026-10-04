# Rendering

Rendering turns a screen and some data into something you can look at. The reference renderer is `@weft/render-react`. It maps every component to plain semantic HTML with the right ARIA role and name, so the accessibility tree of the result equals what the document declares. That, rather than visual design, is its job: it shows that a screen is structured correctly, and gives a model or a person something to check an edit against.

You can get three things out of it.

| You want | Use |
| --- | --- |
| A page to open in a browser | The plugin's `render.ts` script (`/weft:render` in Claude Code) |
| The accessibility tree, as text | The `weft_render` tool of the [MCP server](mcp.md) |
| A React element tree inside your own app | `render` or `WeftView` from `@weft/render-react` |

The examples use the scratch folder from the [tour](tour.md); this block makes it again if you skipped that page.

```console
$ mkdir -p weft-tour
$ cp corpus/login/screen.weft weft-tour/login.weft
```

## A page for the browser

```console
$ node plugins/shared/scripts/render.ts weft-tour/login.weft weft-tour/rendered.html --data corpus/login/data.json
Wrote weft-tour/rendered.html
file:///path/to/weft/weft-tour/rendered.html
```

The script checks the screen strictly first and writes nothing if it has errors. The page is one static HTML file with no scripts: it shows the screen, it does not run actions. Layout comes from a few inline styles (the direction and gap of a `stack`, the columns of a `grid`); everything else is the browser's default look.

To see every reference screen at once, write the gallery:

```console
$ node packages/render-react/src/gallery.ts
12 screens and index.html written to /path/to/weft/packages/render-react/gallery/
```

Open `packages/render-react/gallery/index.html`. The folder is ignored by git.

## The accessibility tree

`weft_render` shows what a screen reader or a browsing agent gets, in the notation of a Playwright aria snapshot. It needs no browser, so it is cheap to run after every edit. Bindings resolve against the data you give it. The login screen without data has no email, so the button is disabled:

```console
$ node docs/examples/mcp-call.mjs weft_render markup=@weft-tour/login.weft
- main "Sign in":
  - heading "Sign in" [level=1]
  - textbox "Email"
  - textbox "Password"
  - button "Sign in" [disabled]
  - link "Forgot password?"
  - link "Create an account"
$ node docs/examples/mcp-call.mjs weft_render markup=@weft-tour/login.weft data=@corpus/login/data.json
- main "Sign in":
  - heading "Sign in" [level=1]
  - textbox "Email": ada@example.com
  - textbox "Password"
  - button "Sign in"
  - link "Forgot password?"
  - link "Create an account"
```

Loops expand against the data too. The todo list repeats an `<each>` once per item of `$.todos`:

```console
$ node docs/examples/mcp-call.mjs weft_render markup=@corpus/todo-list/screen.weft data=@corpus/todo-list/data.json | sed -n '1,11p'
- main "Tasks":
  - heading "Tasks" [level=1]
  - textbox "New task"
  - button "Add" [disabled]
  - list:
    - listitem:
      - checkbox "Write spec" [checked]
      - button "Remove"
    - listitem:
      - checkbox "Build parser"
      - button "Remove"
```

## Data

The data is a JSON file shaped like the paths your bindings use: `{$.email}` reads the `email` member of the top level. `corpus/login/data.json` is the smallest example; every corpus screen has one. A path that does not exist reads as empty, and a binding read as a boolean is false for `false`, `0`, an empty string, `null` and a missing path.

## Tokens

A token reference such as `{token.space.md}` turns into a real value when the page is built. Without `--tokens`, the default set in `packages/catalog/tokens/default.tokens.json` applies, and a screen that names a token the set does not have is not rendered:

```console
$ sed 's/space.md/space.huge/' weft-tour/login.weft > weft-tour/login-token.weft
$ node plugins/shared/scripts/render.ts weft-tour/login-token.weft; echo "exit $?"
weft-tour/login-token.weft:4:24 W306 Token "space.huge" does not exist. — did you mean "space.lg"?
exit 1
```

Give your own file in the same format (W3C Design Tokens, DTCG 2025.10) with `--tokens`. Here a copy of the default set has `space.md` widened from 16 to 40 pixels:

```console
$ node -e 'const fs=require("fs");const t=JSON.parse(fs.readFileSync("packages/catalog/tokens/default.tokens.json","utf8"));t.space.md.$value={value:40,unit:"px"};fs.writeFileSync("weft-tour/wide.tokens.json",JSON.stringify(t,null,2))'
$ node plugins/shared/scripts/render.ts weft-tour/login.weft weft-tour/wide.html --tokens weft-tour/wide.tokens.json
Wrote weft-tour/wide.html
file:///path/to/weft/weft-tour/wide.html
$ grep -o 'gap:[0-9a-z]*' weft-tour/wide.html
gap:40px
```

More in [Catalog and tokens](catalog-and-tokens.md).

## Inside a React app

For a live screen that reacts to input, call the renderer from your own code:

```ts
import { WeftView } from "@weft/render-react";

<WeftView
  document={document}          // a validated Document
  catalog={coreCatalog}
  data={data}                  // what bindings read
  tokens={tokens}              // a map from loadTokens, optional
  actions={{ "auth.submit": (event) => submit(event) }}
  onChange={(path, value) => setData(update(data, path, value))}
/>;
```

- `actions` maps an action name to a function. It receives `{ id, action, item }`: the element, the action name and, inside a loop, the path of the current item (such as `$.todos.2`). An action the host did not list does nothing.
- `onChange` is called when the user changes a field, checkbox, switch, select, radio group or tab, with the absolute data path and the new value. The renderer never keeps the value itself, so your data stays the single source of truth.
- Pass a validated document. The renderer expects what the validator accepted.

The test files `packages/render-react/test/behaviour.test.ts` and `corpus.test.ts` show every interaction in use. [SPEC §9](../SPEC.md#9-mapping) states what a renderer must do.

## Safe by construction

The renderer never treats a document string as markup or code. A `link.href` or `image.src` is used only when it starts with `http`, `https` or `mailto`, or is a relative address; anything else is dropped. States are exposed twice: as ARIA where an equivalent exists (`loading` becomes `aria-busy`) and always as `data-state`.

## Limits

- The output is plain HTML. There is no theme, no CSS beyond inline layout, and no animation.
- One target: React and the web. Nothing renders to native UI.
- The static page does not run actions. Use `WeftView` for that.
