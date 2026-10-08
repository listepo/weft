---
name: weft-figma-pull
description: Read a frame of a Figma file back into a Weft screen (.weft) through the Figma REST API, and show what the read lost. Use when the user wants to pull, import or bring a Figma frame or a figma.com link into Weft.
---

Pull a Figma frame into Weft with the plugin's script. Take the link and an optional output path from the user's request (`/weft-figma-pull <figma link> [out.weft]`).

1. The script needs a Figma access token with the `file_content:read` scope in the `FIGMA_TOKEN` environment variable. Never ask the user to paste the token into the chat, never put it on a command line, and never write it to a file: if `FIGMA_TOKEN` is not set, tell the user to set it in their shell and start the session again.
2. Run, from the user's project directory. The script is `dist/figma-pull.js` of the plugin folder, which is two levels above this skill's folder, so the command is:

   ```bash
   node "<this skill's folder>/../../dist/figma-pull.js" <figma link> [out.weft]
   ```

   The link is the frame's `figma.com/design/…?node-id=…` link (Copy link to selection in Figma). A bare file key works with `--node <id>`. Without an output path the screen is written to the working directory as `<screen id>.weft`. The script refuses to replace an existing file; add `--force` only when the user said to replace it.

   The frame comes back exactly when it was built by the Weft Figma plugin: the plugin keeps each layer's Weft source in shared plugin data, which the script reads. Only shared plugin data is supported: a file built by the plugin's first versions, which kept the source in private plugin data, reads as foreign layers until it is rebuilt with the current plugin. If a `weft.json` sits in the working directory or above it, its catalog and tokens are used, and `import.figma.outDir` places the screen. `--project <weft.json>` names another project file; `--no-project` ignores it.

3. Exit code 0: tell the user where the file is and show the loss table the script printed, unshortened: it lists the designer's edits Weft cannot hold (fills, strokes, radii, padding) and the layers that did not come from Weft.
4. Exit code 1: the diagnostics on stderr say what is wrong with the screen. Report them; do not edit the markup by hand to hide them.
5. Exit code 2: a usage, token, network or file problem (refused token, file or node not found, rate limit, output exists). Report the message.
6. Offer the next step: validate and refine the screen with the `weft_*` tools of the weft MCP server, following the `weft-spec` skill.
