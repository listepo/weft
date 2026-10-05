---
name: weft-render
description: Render a Weft screen (.weft) to an HTML page and preview it in the browser. Use when the user wants to see, preview or open a Weft screen.
---

Render a Weft screen to a static HTML page with the plugin's script, then show it. Take the screen path and the optional output path, `--data` and `--tokens` from the user's request (`/weft-render <screen.weft> [out.html] [--data data.json] [--tokens tokens.json]`).

1. Run, from the user's project directory. The script is `dist/render.js` of the plugin folder, which is two levels above this skill's folder, so the command is:

   ```bash
   node "<this skill's folder>/../../dist/render.js" <screen.weft> [out.html] [--data data.json] [--tokens tokens.json]
   ```

   `--data` is the sample data the bindings read; `--tokens` is a DTCG tokens file or resolver (default: the catalog's default tokens); `--appearance light` or `dark` picks a side of a resolver's light and dark themes (default: its default context). Without an output path the page is written next to the screen as `<name>.html`. The script refuses to replace an existing file; add `--force` only when the user said to replace it, or when you are re-rendering a page you wrote earlier in this session.

   If a `weft.json` sits in the screen's folder or above it, the screen is rendered with that project's catalog, and its settings stand in for missing arguments: `render.data` for `--data`, `render.tokens` for `--tokens` (else the project's own tokens), `render.appearance` for `--appearance`, `render.outDir` for the output path. `--project <weft.json>` names another project file; `--no-project` ignores it. A project with errors stops the script with exit code 1 and its problems printed as `weft.json:#/pointer code message`.

2. Exit code 0: the script prints the path and a `file://` URL. Open that URL in Cursor's built-in browser (its Navigate tool). If the browser does not open a `file://` URL, serve the page's folder on localhost instead (`python3 -m http.server --bind 127.0.0.1 <port>` in the background, then `http://127.0.0.1:<port>/<name>.html`) and stop the server when you are done. If no browser tool is available, give the user the path to open.
3. Exit code 1: the screen is not strictly valid and the diagnostics were printed; nothing was written. Report them and offer to fix the screen with the `weft_*` tools (see the `weft-spec` skill).
4. Exit code 2: a usage or file problem. Report the message.
5. After the page is open, look at it before you answer: if something is clearly off, say what and offer a fix instead of declaring success.
