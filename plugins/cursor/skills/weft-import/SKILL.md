---
name: weft-import
description: Convert an HTML file into a Weft screen (.weft) and show what the import lost. Use when the user wants to import, convert or bring a web page or HTML file into Weft.
---

Import an HTML file into Weft with the plugin's script. Take the page path and an optional output path from the user's request (`/weft-import <page.html> [out.weft]`).

1. Run, from the user's project directory. The script is `dist/import.js` of the plugin folder, which is two levels above this skill's folder, so the command is:

   ```bash
   node "<this skill's folder>/../../dist/import.js" <page.html> [out.weft]
   ```

   Without an output path the screen is written next to the page as `<name>.weft`. The script refuses to replace an existing file; add `--force` only when the user said to replace it.

   If a `weft.json` sits in the page's folder or above it, the page is mapped onto that project's catalog, and its `import.html.outDir` setting places the screen when no output path is given. `--project <weft.json>` names another project file; `--no-project` ignores it. A project with errors stops the script with exit code 1 and its problems printed as `weft.json:#/pointer code message`.

2. Exit code 0: tell the user where the file is and show the loss table the script printed. Do not shorten it: the table lists what an HTML page cannot carry (bindings, actions, tokens, slots, hidden elements), and the user needs it to know what to add back.
3. Exit code 1: the diagnostics on stderr say what is wrong with the import. Report them; do not edit the generated markup by hand to hide them.
4. Exit code 2: a usage or file problem (missing file, file too large, output exists). Report the message.
5. Offer the next step: validate and refine the screen with the `weft_*` tools of the weft MCP server, following the `weft-spec` skill.
