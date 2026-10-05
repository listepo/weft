import { LIMITS, type Limits } from "./context.ts";

/**
 * What a model must know to write Weft; component details are not repeated here because the
 * catalog is the source of truth and `weft_catalog` serves it on demand.
 */
export const primer = (
  limits: Limits,
): string => `Weft describes a UI screen as strict XML-subset markup. Rules:
- One root <screen id="…" weft="0.1" label="…">. Every element has an id, unique in the document: a letter, then letters, digits, "_" or "-".
- Element and attribute names are lowercase with hyphens. Attribute values are always in double quotes. Booleans are true or false. An element without content is self-closing.
- Only components of the catalog exist. Do not invent elements or attributes; call weft_catalog to see them.
- A value is a literal ("Email"), a binding value="{$.user.email}", a negated binding disabled="{!$.busy}", or a design token gap="{token.space.md}". A literal that starts with "{" is written "{{". Never mix text and a binding in one value.
- Inside <each in="{$.items}" as="item"> the loop variable is bound as {$item.field}; ids inside it are templates.
- Events are on-<event>="action.name", for example on-press="auth.submit". Actions take no arguments and no code.
- Named slots: <slot name="footer"> as a direct child of a component that declares that slot. Text is content of text-bearing components.
- Comments are allowed and dropped by weft_format.

Tools:
- weft_capabilities: what the host accepts: the format version, the catalogs and, when the host checks them, the design token paths and action names. Write only what it lists.
- weft_catalog: no arguments lists every component; pass kind for one component in full (props, slots, states, events).
- weft_validate: check markup. Diagnostics carry a code, a path, what was expected and often the fix in "hint".
- weft_format: canonical markup, the one form that diffs and hashes.
- weft_patch: edit existing markup without rewriting it. Send the current markup and a list of patches; the result is the new canonical markup, or diagnostics and nothing applied.
- weft_render: see the screen as assistive technology or a browsing agent would: the accessibility tree (roles, names, states) for the markup and optional sample data.
- project: every tool except weft_primer takes the project's weft.json with each file name replaced by that file's JSON content ({"tokens":[…],"catalog":{…},"actions":[…],"data":{JSON Schema}}). Pass the same project to every call: it brings the project's own components, tokens, actions and data model. Its problems have paths that start at #/project; fix the project, not the screen.

Patches (addressed by id, applied in order, all-or-nothing):
- {"op":"set","id":"go","prop":"variant","value":"primary"}: value is typed JSON: "text", 7, true, {"bind":"$.busy"}, {"bind":"$.busy","not":true}, {"token":"space.md"}. null removes the prop. A prop named on-<event> sets the action name (null unbinds). An id cannot be set.
- {"op":"insert","parent":"main","slot":"footer","index":0,"markup":"<button id=\\"b\\">Go</button>"}: markup is one or more elements with new ids. Without slot it goes into the default content; index defaults to the end and counts text too.
- {"op":"remove","id":"go"}: removes the element and everything in it.
- {"op":"move","id":"go","parent":"f","slot":"footer","index":0}: index counts the target list after the element left it.
- {"op":"set","id":"go","prop":"text","value":"Save"}: changes an element's text. Text written as content stays content; a binding value moves it into the text attribute.
The root cannot be removed or moved.

Workflow: write or edit, call weft_validate (or let weft_patch validate), and fix every error using its hint before you answer. Limits: markup at most ${limits.markupChars} characters, at most ${limits.patches} patches per call.`;

export const PRIMER = primer(LIMITS);
