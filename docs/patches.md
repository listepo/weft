# Patches

A patch is how a screen is changed without being rewritten. It is a short list of operations, each addressed by element `id`. Because ids stay the same through edits, "set the placeholder of `password`" means the same thing before and after any other change, and nothing else in the file is touched.

Patches suit agents, but a person or a program can send them just as well. The result of a patch is a screen in canonical form, so a diff shows exactly the change.

The examples use the scratch folder from the [tour](tour.md); this block makes it again if you skipped that page.

```console
$ mkdir -p weft-tour plugins/shared/scratch
$ cp corpus/login/screen.weft weft-tour/login.weft
```

## The four operations

| Operation | Members | What it does |
| --- | --- | --- |
| `set` | `id`, `prop`, `value` | Sets a prop, or removes it when `value` is `null`. A `prop` starting with `on-` binds or unbinds an event: its value is an action name. |
| `insert` | `parent`, `markup`, `slot` and `index` (optional) | Puts one or more new elements into a parent, at the end unless `index` says where. `slot` names a named slot. |
| `remove` | `id` | Deletes the element and everything inside it. |
| `move` | `id`, `parent`, `slot` and `index` (optional) | Takes an element out and puts it somewhere else, keeping its id and content. |

A patch has exactly these members and nothing else. [SPEC §7](../SPEC.md#7-patches) has the full rules.

**`value` is typed JSON, not markup.** `7` is the number seven, `true` a boolean, `"Hello"` a string that stays a literal even if it starts with a brace, `{ "bind": "$.name" }` a binding, `{ "bind": "$.busy", "not": true }` a negated binding and `{ "token": "space.md" }` a token. `id` and the root's `weft` cannot be set; to rename an element, remove it and insert it again.

**`insert` takes markup.** It is one or more elements written as in a screen but with no `<screen>` root, and their ids must be new.

**Text** is changed with `set` and `prop: "text"`, whether the element keeps it as content or in a `text` prop.

## Apply a list

This list exercises every operation on the login screen: it changes the heading text, adds a placeholder, drops `required` from the email field, adds a Help link at the start of the footer, moves the sign-up link in front of it and removes the reset link.

```console
$ cat > weft-tour/edit.json <<'EOF'
[
  { "op": "set", "id": "title", "prop": "text", "value": "Welcome back" },
  { "op": "set", "id": "password", "prop": "placeholder", "value": "At least 8 characters" },
  { "op": "set", "id": "email", "prop": "required", "value": null },
  {
    "op": "insert",
    "parent": "form",
    "slot": "footer",
    "index": 0,
    "markup": "<link id=\"help\" on-press=\"nav.help\">Help</link>"
  },
  { "op": "move", "id": "signup", "parent": "form", "slot": "footer", "index": 0 },
  { "op": "remove", "id": "reset" }
]
EOF
$ node docs/examples/mcp-call.mjs weft_patch markup=@weft-tour/login.weft patches=@weft-tour/edit.json > weft-tour/edited.weft; echo "exit $?"
exit 0
$ diff weft-tour/login.weft weft-tour/edited.weft
3c3
<     <heading id="title" level="1">Sign in</heading>
---
>     <heading id="title" level="1">Welcome back</heading>
5,6c5,6
<       <field id="email" label="Email" required="true" type="email" value="{$.email}"/>
<       <field id="password" label="Password" required="true" type="password" value="{$.password}"/>
---
>       <field id="email" label="Email" type="email" value="{$.email}"/>
>       <field id="password" label="Password" placeholder="At least 8 characters" required="true" type="password" value="{$.password}"/>
10d9
<       <link id="reset" on-press="nav.reset">Forgot password?</link>
11a11
>       <link id="help" on-press="nav.help">Help</link>
```

`weft_patch` is a tool of the [MCP server](mcp.md); `mcp-call.mjs` is the small client used throughout this guide. The patches are applied in order to a copy, so a later one sees the effects of an earlier one (the `move` above finds the link where it is after the `insert`). `index` counts entries of the target list, and for a `move` it counts after the element has left it.

## All or nothing

If any patch fails, or the result has errors, nothing is applied. You get diagnostics that point at the patch (`#/patches/0/markup`), and the first failing patch is the one reported. A patch list does not repair a screen that was already invalid; its errors come back instead. Three common failures:

```console
$ echo '[{"op":"insert","parent":"fields","markup":"<checkbox id=\"email\" label=\"Remember me\"/>"}]' > weft-tour/dup.json
$ node docs/examples/mcp-call.mjs weft_patch markup=@weft-tour/login.weft patches=@weft-tour/dup.json; echo "exit $?"
{"diagnostics":[{"code":"W509","severity":"error","message":"Id \"email\" is already used in the document.","path":"#/patches/0/markup","expected":"ids that no element of the document has","got":"email","hint":"use \"email-2\""}]}
exit 1
$ echo '[{"op":"insert","parent":"fields","index":9,"markup":"<text id=\"t\">Hi</text>"}]' > weft-tour/idx.json
$ node docs/examples/mcp-call.mjs weft_patch markup=@weft-tour/login.weft patches=@weft-tour/idx.json; echo "exit $?"
{"diagnostics":[{"code":"W505","severity":"error","message":"Index 9 is past the end of a list with 2 entries.","path":"#/patches/0/index","expected":"an integer from 0 to 2 (text counts as an entry)","got":"9","hint":"omit `index` to append"}]}
exit 1
$ echo '[{"op":"set","id":"title","prop":"label","value":5},{"op":"delete","id":"x"}]' > weft-tour/shape.json
$ node docs/examples/mcp-call.mjs weft_patch markup=@weft-tour/login.weft patches=@weft-tour/shape.json; echo "exit $?"
{"diagnostics":[{"code":"W501","severity":"error","message":"Patch 1 is malformed: Invalid discriminator value. Expected 'set' | 'insert' | 'remove' | 'move'.","path":"#/patches/1/op","expected":"one of: \"set\", \"insert\", \"remove\", \"move\"","hint":"\"op\" must be \"set\", \"insert\", \"remove\" or \"move\""}]}
exit 1
```

Each diagnostic has a stable code. `W502` is an id that no element has (with the nearest id as a hint), `W504` a slot the parent does not declare, `W506` a move into itself, `W507` removing or moving the root, `W508` bad insert markup, `W509` a reused id and `W501` a patch with the wrong shape. [SPEC §6.2](../SPEC.md#62-codes) lists them all.

The result is also validated strictly, so a patch that produces something the catalog rejects fails too: setting `variant` to a word the button does not have gives the usual `W203`, with the allowed words listed.

## Check that a patch means what you asked

A patch can be valid and still wrong. The usual mistake is a flipped condition: you ask for "disabled while busy" and the patch says "disabled while not busy". Compare the old and new screens with `weft explain --against` ([the `weft` command](cli.md)):

```console
$ cat > weft-tour/inverted.json <<'EOF'
[{ "op": "set", "id": "submit", "prop": "disabled", "value": { "bind": "$.busy", "not": true } }]
EOF
$ node docs/examples/mcp-call.mjs weft_patch markup=@weft-tour/login.weft patches=@weft-tour/inverted.json > weft-tour/inverted.weft
$ weft explain weft-tour/inverted.weft --against weft-tour/login.weft --catalog packages/catalog/catalog.json
button#submit disabled changed: was true while $.email is falsy (NOT $.email); now true while $.busy is falsy (NOT $.busy)
```

The line ends with `is falsy (NOT $.busy)`, which is the opposite of "disabled while busy". Dropping `"not": true` gives the right condition. The agent guide ([AGENT-SPEC.md](../AGENT-SPEC.md)) asks agents to read back their changes this way before they answer.

## From code

An application uses the same operation through the library, `applyPatches(document, patches, { catalog, mode, tokens, actions })`. It treats the patch list as untrusted input, never throws and never changes the document it is given. From a file inside a workspace package (see the note in the [index](README.md)):

```console
$ cat > plugins/shared/scratch/patch.ts <<'EOF'
import { readFileSync } from "node:fs";
import { coreCatalog } from "@weft/catalog";
import { applyPatches, parse, serialize } from "@weft/core";

const { document } = parse(readFileSync("weft-tour/login.weft", "utf8"), { catalog: coreCatalog });
const patches = [{ op: "set", id: "submit", prop: "variant", value: "danger" }];

const result = applyPatches(document, patches, { catalog: coreCatalog, mode: "strict" });
if (result.document === undefined) {
  console.log(result.diagnostics);
} else {
  console.log(serialize(result.document).split("\n")[7]);
}
EOF
$ node plugins/shared/scratch/patch.ts
    <button id="submit" disabled="{!$.email}" submit="true" variant="danger">Sign in</button>
```

## Limits

- Text that shares a list with elements, such as an `item` holding text beside a button, cannot be addressed by a patch. Remove the element and insert it again with the same id.
- There is no patch for the root's `weft` version, and no way to rename an id.
- A patch is not a merge. Two people's patches against the same screen are applied one after the other, and the second fails if it names something the first removed.
