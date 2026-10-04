---
name: weft-export
description: Convert a Weft screen (.weft) into a React component (.jsx). Use when the user wants React or JSX code from a Weft file.
---

Export a Weft screen to a React component with the plugin's script. Take the screen path, an optional output path and an optional component name from the user's request (`/weft-export <screen.weft> [out.jsx] [--name Component]`).

1. Run, from the user's project directory. The script is `dist/export.js` of the plugin folder, which is two levels above this skill's folder, so the command is:

   ```bash
   node "<this skill's folder>/../../dist/export.js" <screen.weft> [out.jsx] [--name Component]
   ```

   Without an output path the component is written next to the screen as `<name>.jsx`; `--name` sets the component's name (default `WeftScreen`). The script refuses to replace an existing file; add `--force` only when the user said to replace it.

2. Exit code 0: tell the user where the file is. The component is `export default function Name({ data, actions, onChange })`: values are read from `data`, two-way fields call `onChange(path, value)`, host actions are called through `actions`. Say so in one line.
3. Exit code 1: the screen is not strictly valid and the script printed its diagnostics. Report them and offer to fix the screen with the `weft_*` tools (see the `weft-spec` skill); nothing was written.
4. Exit code 2: a usage or file problem. Report the message.
