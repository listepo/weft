# Exporting to React

Exporting turns a Weft screen into a React component you can drop into an app. The component is plain JavaScript with JSX, in one file, with no dependency on Weft at run time. Use it when a screen has been designed and checked in Weft and you now want it in a codebase, or to see exactly what a screen means in code.

The generated component has the same structure as the [reference renderer](rendering.md) produces: the same elements, roles, names and states. A test compares the two on every corpus screen.

## Export a file

The plugin's `export.ts` script (the `/weft:export` command in Claude Code) reads a `.weft` file, validates it strictly and writes a `.jsx` file next to it. `--name` sets the component's name; it must start with a capital letter.

The examples use the scratch folder from the [tour](tour.md); this block makes it again if you skipped that page.

```console
$ mkdir -p weft-tour
$ cp corpus/login/screen.weft weft-tour/login.weft
$ node plugins/shared/scripts/export.ts weft-tour/login.weft weft-tour/LoginScreen.jsx --name LoginScreen
weft: weft-tour/LoginScreen.jsx already exists; pass --force to replace it
$ node plugins/shared/scripts/export.ts weft-tour/login.weft weft-tour/Default.jsx
Wrote weft-tour/Default.jsx
$ grep -n "export default" weft-tour/LoginScreen.jsx weft-tour/Default.jsx
weft-tour/LoginScreen.jsx:28:export default function LoginScreen({ data, actions, onChange }) {
weft-tour/Default.jsx:28:export default function WeftScreen({ data, actions, onChange }) {
```

The script does not replace an existing file without `--force`, and it does not write anything for a screen that has errors. Exit code 0 means done, 1 that the screen has errors (printed as `file:line:column code message`), 2 a usage or file problem such as a component name that is not a valid name:

```console
$ node plugins/shared/scripts/export.ts weft-tour/login.weft weft-tour/Bad.jsx --name login; echo "exit $?"
weft: componentName must match /^[A-Z][A-Za-z0-9]*$/
exit 2
```

## What the component looks like

The screen's structure is in the function body. Take the part of the login screen that holds the submit button:

```console
$ sed -n '28,31p;41,45p' weft-tour/LoginScreen.jsx | cut -c1-150
export default function LoginScreen({ data, actions, onChange }) {
  return (
    <main data-weft-id="login" aria-label="Sign in">
      <form data-weft-id="form" data-state="idle" onSubmit={(_e) => { _e.preventDefault(); _act(actions, { id: "form", action: "auth.submit" }); }}>
            <label>
              <span aria-hidden="true">Password</span>
              <input data-weft-id="password" aria-label="Password" type="password" required value={_text(data?.password)} onChange={(_e) => onChange?.
            </label>
          </div>
```

The file starts with a few small helper functions, so it needs nothing else. It uses JSX syntax with no `import` lines, so your build must use React's automatic JSX runtime, which current React setups do by default.

## Using the component

```jsx
import LoginScreen from "./LoginScreen.jsx";

function App() {
  const [data, setData] = useState({ email: "", password: "" });
  return (
    <LoginScreen
      data={data}
      actions={{ "auth.submit": (event) => signIn(data) }}
      onChange={(path, value) => setData(setPath(data, path, value))}
    />
  );
}
```

The component takes three props, and every Weft idea maps to one of them:

| In the screen | In the component |
| --- | --- |
| A binding, `value="{$.email}"` | A read of `data.email`. A path that does not exist reads as empty. |
| A negated binding, `disabled="{!$.email}"` | The same read with `!` in front. |
| An input that writes, such as a `field`, `checkbox`, `switch`, `select` or `tabs` | A controlled input. When the user changes it, the component calls `onChange(path, value)` with the absolute path, for example `onChange("$.email", "ada@")`. It does not keep the value itself: put it in `data` and render again. |
| An event, `on-press="nav.reset"` | A call of `actions["nav.reset"]` with `{ id, action, item }`: the element id, the action name and, inside a loop, the path of the current item. Nothing happens when your app did not supply that action. |
| An `<each>` loop | `.map` over the array in `data`, with the item index as the key. |
| A token, `gap="{token.space.md}"` | The CSS variable `var(--weft-space-md)` |

**Tokens need a stylesheet.** The component refers to a CSS variable named after the token path, with dots turned into hyphens. It does not define the variable. Put the values in your own CSS (`:root { --weft-space-md: 16px; }`), ideally generated from the same tokens file the designer owns ([Catalog and tokens](catalog-and-tokens.md)). `weft css-tokens` writes them as `weft-tokens.css`.

**A base stylesheet for the screen.** The component carries only its layout, so unstyled it shows a caption flush against its field and a link that does not look like one. `weft css-base` writes `weft-base.css`, the minimal rules that `weft html` also puts in its page; link it after `weft-tokens.css` ([the base stylesheet](cli.md#the-base-stylesheet)). It sets no page colour or font and does not change what the component renders.

## Safe by construction

A document is untrusted, so the generator never copies a document string into code. Strings appear only as escaped literals or as JSX text made of harmless characters; link and image addresses are used only for `http`, `https`, `mailto` or relative values. A screen cannot make the generated file run something you did not write. [SPEC §9](../SPEC.md#9-mapping) states the rules.

## Limits

- One screen becomes one component. Nothing is shared between components, and there is no way to tell the generator how to use your own design-system components.
- The output is JavaScript, not TypeScript, and is regenerated rather than edited. Change the screen and export again.
- Styling comes only from the inline layout of `stack` and `grid` and from your CSS variables.
