---
description: Convert an HTML file into a Weft screen (.weft) and show what the import lost. Use when the user wants to import, convert or bring a web page or HTML file into Weft.
argument-hint: <page.html> [out.weft]
---

Import an HTML file into Weft with the plugin's script. Arguments: `$ARGUMENTS`

1. Run, from the user's project directory:

   ```bash
   node "${CLAUDE_PLUGIN_ROOT}/scripts/import.ts" <page.html> [out.weft]
   ```

   Without an output path the screen is written next to the page as `<name>.weft`. The script refuses to replace an existing file; add `--force` only when the user said to replace it.
2. Exit code 0: tell the user where the file is and show the loss table the script printed. Do not shorten it: the table lists what an HTML page cannot carry (bindings, actions, tokens, slots, hidden elements), and the user needs it to know what to add back.
3. Exit code 1: the diagnostics on stderr say what is wrong with the import. Report them; do not edit the generated markup by hand to hide them.
4. Exit code 2: a usage or file problem (missing file, file too large, output exists). Report the message.
5. Offer the next step: validate and refine the screen with the `weft_*` tools of the weft MCP server, following the `weft:spec` skill.
