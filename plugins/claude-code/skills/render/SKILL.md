---
description: Render a Weft screen (.weft) to an HTML page and preview it in the browser. Use when the user wants to see, preview or open a Weft screen.
argument-hint: <screen.weft> [out.html] [--data data.json] [--tokens tokens.json]
---

Render a Weft screen to a static HTML page with the plugin's script, then show it. Arguments: `$ARGUMENTS`

1. Run, from the user's project directory:

   ```bash
   node "${CLAUDE_PLUGIN_ROOT}/scripts/render.ts" <screen.weft> [out.html] [--data data.json] [--tokens tokens.json]
   ```

   `--data` is the sample data the bindings read; `--tokens` is a DTCG tokens file (default: the catalog's default tokens). Without an output path the page is written next to the screen as `<name>.html`. The script refuses to replace an existing file; add `--force` only when the user said to replace it, or when you are re-rendering a page you wrote earlier in this session.
2. Exit code 0: the script prints the path and a `file://` URL. Open that URL in the built-in browser of Claude Code Desktop (the Browser pane: use its `navigate` tool with the URL, or `preview_start` with `url`). If no browser tool is available, give the user the path to open.
3. Exit code 1: the screen is not strictly valid and the diagnostics were printed; nothing was written. Report them and offer to fix the screen with the `weft_*` tools (see the `weft:spec` skill).
4. Exit code 2: a usage or file problem. Report the message.
5. After the page is open, look at it before you answer: if something is clearly off, say what and offer a fix instead of declaring success.
